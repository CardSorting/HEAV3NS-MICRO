// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::IModelPricingRegistry;
use crate::domain::types::model_catalog::ModelPricingCatalog;
use crate::plumbing::hash_and_math::now_ms;
use reqwest::Client;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

pub const DEFAULT_OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
pub const DEFAULT_CATALOG_TTL_MS: u64 = 12 * 60 * 60 * 1_000; // 12 hours

/// Auto-refreshing live model pricing & capability registry implementing [`IModelPricingRegistry`].
/// Boots instantaneously from `~/.heav3ns/pricing_catalog.json` (merged over `ModelPricingCatalog::embedded_baseline()`)
/// and refreshes asynchronously in the background from `https://openrouter.ai/api/v1/models` whenever stale.
pub struct ModelPricingRegistry {
    catalog: RwLock<ModelPricingCatalog>,
    cache_path: PathBuf,
    ttl_ms: u64,
    refresh_in_flight: AtomicBool,
}

impl ModelPricingRegistry {
    pub fn load_or_default() -> Self {
        let cache_path = default_pricing_cache_path();
        let ttl_ms = std::env::var("HEAV3NS_PRICING_TTL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(|secs| secs.saturating_mul(1_000))
            .unwrap_or(DEFAULT_CATALOG_TTL_MS);

        let initial = load_cached_catalog(&cache_path).unwrap_or_else(ModelPricingCatalog::embedded_baseline);
        Self {
            catalog: RwLock::new(initial),
            cache_path,
            ttl_ms,
            refresh_in_flight: AtomicBool::new(false),
        }
    }

    pub fn update_in_memory(&self, next: ModelPricingCatalog) {
        if let Ok(mut guard) = self.catalog.write() {
            *guard = next;
        }
    }

    /// Non-blocking auto-refresh hook called on live provider startup / turn orchestration.
    /// Never blocks the caller, deduplicates concurrent refreshes with `AtomicBool`, and honors `HEAV3NS_OFFLINE_PRICING=1`.
    pub fn ensure_fresh_background(&'static self) {
        if std::env::var("HEAV3NS_OFFLINE_PRICING")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            return;
        }
        if !self.is_stale(now_ms()) {
            return;
        }
        if self
            .refresh_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = self.refresh_now().await;
                self.refresh_in_flight.store(false, Ordering::Release);
            });
        } else {
            self.refresh_in_flight.store(false, Ordering::Release);
        }
    }

    /// Explicitly fetches the latest live model pricing catalog from `HEAV3NS_MODEL_CATALOG_URL`
    /// (default `https://openrouter.ai/api/v1/models`), persists `~/.heav3ns/pricing_catalog.json` atomically,
    /// and hot-swaps the in-memory catalog.
    pub async fn refresh_now(&self) -> Result<ModelPricingCatalog, String> {
        let url = std::env::var("HEAV3NS_MODEL_CATALOG_URL")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_OPENROUTER_MODELS_URL.to_string());

        let client = Client::builder()
            .connect_timeout(Duration::from_millis(3_500))
            .timeout(Duration::from_millis(6_000))
            .build()
            .map_err(|e| format!("Failed building HTTP client for model catalog refresh: {e}"))?;

        let resp = client
            .get(&url)
            .header("Accept", "application/json")
            .header("User-Agent", "heav3ns-micro/0.1.0")
            .send()
            .await
            .map_err(|e| format!("Failed fetching model catalog from {url}: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Model catalog endpoint {url} returned HTTP {}", resp.status().as_u16()));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed reading model catalog payload: {e}"))?;

        let refreshed = ModelPricingCatalog::parse_openrouter_models_json(&body, now_ms())?;
        let _ = persist_catalog_atomic(&self.cache_path, &refreshed);
        self.update_in_memory(refreshed.clone());
        Ok(refreshed)
    }
}

impl IModelPricingRegistry for ModelPricingRegistry {
    fn snapshot_catalog(&self) -> ModelPricingCatalog {
        self.catalog
            .read()
            .map(|g| g.clone())
            .unwrap_or_else(|_| ModelPricingCatalog::embedded_baseline())
    }

    fn is_stale(&self, current_ms: u64) -> bool {
        let Ok(guard) = self.catalog.read() else {
            return true;
        };
        if guard.fetched_at_ms == 0 {
            return true;
        }
        current_ms.saturating_sub(guard.fetched_at_ms) >= self.ttl_ms
    }
}

static GLOBAL_PRICING_REGISTRY: OnceLock<ModelPricingRegistry> = OnceLock::new();

pub fn global_pricing_registry() -> &'static ModelPricingRegistry {
    GLOBAL_PRICING_REGISTRY.get_or_init(ModelPricingRegistry::load_or_default)
}

fn default_pricing_cache_path() -> PathBuf {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    home.join(".heav3ns").join("pricing_catalog.json")
}

fn load_cached_catalog(path: &Path) -> Option<ModelPricingCatalog> {
    let raw = fs::read_to_string(path).ok()?;
    let disk: ModelPricingCatalog = serde_json::from_str(&raw).ok()?;
    if disk.entries.is_empty() {
        return None;
    }
    let mut merged = ModelPricingCatalog::embedded_baseline();
    merged.fetched_at_ms = disk.fetched_at_ms;
    merged.source = if disk.source.is_empty() {
        "disk_cache".to_string()
    } else {
        format!("{}_cached", disk.source)
    };
    for entry in disk.entries.into_values() {
        merged.upsert_entry(entry);
    }
    Some(merged)
}

fn persist_catalog_atomic(path: &Path, catalog: &ModelPricingCatalog) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed creating cache dir: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }
    let serialized = serde_json::to_string(catalog).map_err(|e| format!("Serialize error: {e}"))?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, serialized).map_err(|e| format!("Write temp catalog failed: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp_path, path).map_err(|e| format!("Atomic rename catalog failed: {e}"))
}
