// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Abstract contract for querying live or cached model pricing & context-window metadata.
pub trait IModelCatalog {
    fn lookup_entry(&self, provider: &str, model: &str) -> Option<&ModelCatalogEntry>;
    fn model_count(&self) -> usize;
}

/// Abstract contract for dynamic fast/strong model tier recommendation per provider.
pub trait IModelTierRecommender {
    fn recommend_fast_and_strong(&self, provider: &str) -> (String, String);
}

/// Normalized per-million-token pricing and capability entry for a single LLM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCatalogEntry {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "contextLength")]
    pub context_length: usize,
    #[serde(rename = "inputPerMillion")]
    pub input_per_million: f64,
    #[serde(rename = "outputPerMillion")]
    pub output_per_million: f64,
    #[serde(rename = "cachedPerMillion")]
    pub cached_per_million: f64,
    #[serde(rename = "createdUnix")]
    pub created_unix: u64,
}

/// Pure, zero-I/O model pricing and capability catalog (`DOMAIN`).
/// Seeded with November 2026 frontier models and hot-upgraded in memory from live
/// `https://openrouter.ai/api/v1/models` JSON payloads (`460+` models) via `INFRASTRUCTURE`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricingCatalog {
    pub schema: String,
    #[serde(rename = "fetchedAtMs")]
    pub fetched_at_ms: u64,
    pub source: String,
    pub entries: BTreeMap<String, ModelCatalogEntry>,
    #[serde(rename = "bareIndex")]
    pub bare_index: BTreeMap<String, String>,
}

impl Default for ModelPricingCatalog {
    fn default() -> Self {
        Self::embedded_baseline()
    }
}

impl IModelCatalog for ModelPricingCatalog {
    fn lookup_entry(&self, provider: &str, model: &str) -> Option<&ModelCatalogEntry> {
        self.lookup(provider, model)
    }

    fn model_count(&self) -> usize {
        self.entries.len()
    }
}

impl IModelTierRecommender for ModelPricingCatalog {
    fn recommend_fast_and_strong(&self, provider: &str) -> (String, String) {
        self.recommend_tiers(provider)
    }
}

impl ModelPricingCatalog {
    /// Embedded November 2026 frontier baseline used offline before the first live auto-refresh completes.
    pub fn embedded_baseline() -> Self {
        let mut catalog = Self {
            schema: "heav3ns.model_catalog/v1".to_string(),
            fetched_at_ms: 0,
            source: "embedded_2026_11".to_string(),
            entries: BTreeMap::new(),
            bare_index: BTreeMap::new(),
        };

        // Seed rows: (canonical_id, display_name, ctx_len, in_$/M, out_$/M, cache_$/M, created_unix)
        let seeds: &[(&str, &str, usize, f64, f64, f64, u64)] = &[
            // OpenAI GPT-6.1 / GPT-6 / GPT-5.6 / GPT-5.4 frontier series
            ("openai/gpt-6.1-terra-pro", "GPT-6.1 Terra Pro", 1_050_000, 1.00, 5.00, 0.05, 1_793_038_556),
            ("openai/gpt-6.1-terra", "GPT-6.1 Terra", 1_050_000, 1.00, 5.00, 0.05, 1_793_038_409),
            ("openai/gpt-6.1-sol-pro", "GPT-6.1 Sol Pro", 1_050_000, 2.00, 10.00, 0.10, 1_792_900_000),
            ("openai/gpt-6.1-sol", "GPT-6.1 Sol", 1_050_000, 2.00, 10.00, 0.10, 1_792_890_000),
            ("openai/gpt-6-luna-pro", "GPT-6 Luna Pro", 1_050_000, 0.10, 0.50, 0.01, 1_791_500_000),
            ("openai/gpt-6-luna", "GPT-6 Luna", 1_050_000, 0.10, 0.50, 0.01, 1_791_490_000),
            ("openai/gpt-6-sol", "GPT-6 Sol", 1_050_000, 2.00, 10.00, 0.20, 1_790_800_000),
            ("openai/gpt-6-astra-pro", "GPT-6 Astra Pro", 1_050_000, 10.00, 50.00, 1.00, 1_790_500_000),
            ("openai/gpt-6-astra", "GPT-6 Astra", 1_050_000, 10.00, 50.00, 1.00, 1_790_490_000),
            ("openai/gpt-5.6-terra-pro", "GPT-5.6 Terra Pro", 1_050_000, 2.00, 12.00, 0.20, 1_788_000_000),
            ("openai/gpt-5.6-terra", "GPT-5.6 Terra", 1_050_000, 2.00, 12.00, 0.20, 1_787_990_000),
            ("openai/gpt-5.6-luna", "GPT-5.6 Luna", 1_050_000, 0.20, 1.20, 0.02, 1_787_980_000),
            ("openai/gpt-5.6-sol", "GPT-5.6 Sol", 1_050_000, 4.00, 20.00, 0.40, 1_787_970_000),
            ("openai/gpt-5.5", "GPT-5.5", 1_050_000, 5.00, 30.00, 0.50, 1_784_000_000),
            ("openai/gpt-5.4", "GPT-5.4", 1_050_000, 2.50, 15.00, 0.25, 1_780_000_000),
            ("openai/gpt-5.4-mini", "GPT-5.4 Mini", 400_000, 0.75, 4.50, 0.075, 1_779_990_000),
            ("openai/gpt-5.4-nano", "GPT-5.4 Nano", 400_000, 0.20, 1.25, 0.02, 1_779_980_000),
            ("openai/gpt-5.3-codex", "GPT-5.3 Codex", 400_000, 1.75, 14.00, 0.175, 1_776_000_000),
            ("openai/gpt-5", "GPT-5", 400_000, 1.25, 10.00, 0.125, 1_755_000_000),
            ("openai/gpt-4.1", "GPT-4.1", 1_047_576, 2.00, 8.00, 0.50, 1_744_600_000),
            ("openai/gpt-4.1-mini", "GPT-4.1 Mini", 1_047_576, 0.40, 1.60, 0.10, 1_744_590_000),
            ("openai/o4-mini", "OpenAI o4-mini", 200_000, 1.10, 4.40, 0.275, 1_744_800_000),
            ("openai/o3", "OpenAI o3", 200_000, 2.00, 8.00, 0.50, 1_744_790_000),
            // Anthropic Claude 5.5 / Fable 5.2 / Claude 5 / Claude 4.8-4.6 series
            ("anthropic/claude-fable-5.2", "Claude Fable 5.2", 1_000_000, 8.00, 40.00, 0.20, 1_794_253_781),
            ("anthropic/claude-haiku-5.5", "Claude Haiku 5.5", 1_000_000, 0.10, 0.50, 0.01, 1_793_500_000),
            ("anthropic/claude-sonnet-5.5", "Claude Sonnet 5.5", 1_000_000, 2.00, 10.00, 0.10, 1_793_490_000),
            ("anthropic/claude-opus-5.5", "Claude Opus 5.5", 1_000_000, 4.00, 20.00, 0.20, 1_793_480_000),
            ("anthropic/claude-fable-5.1", "Claude Fable 5.1", 1_000_000, 10.00, 50.00, 0.25, 1_791_000_000),
            ("anthropic/claude-opus-5", "Claude Opus 5", 1_000_000, 5.00, 25.00, 0.50, 1_789_000_000),
            ("anthropic/claude-sonnet-5", "Claude Sonnet 5", 1_000_000, 2.00, 10.00, 0.20, 1_788_990_000),
            ("anthropic/claude-opus-4.8", "Claude Opus 4.8", 1_000_000, 5.00, 25.00, 0.50, 1_782_000_000),
            ("anthropic/claude-opus-4.7", "Claude Opus 4.7", 1_000_000, 5.00, 25.00, 0.50, 1_779_000_000),
            ("anthropic/claude-opus-4.6", "Claude Opus 4.6", 1_000_000, 5.00, 25.00, 0.50, 1_772_000_000),
            ("anthropic/claude-sonnet-4.6", "Claude Sonnet 4.6", 1_000_000, 3.00, 15.00, 0.30, 1_771_900_000),
            ("anthropic/claude-haiku-4.5", "Claude Haiku 4.5", 200_000, 1.00, 5.00, 0.10, 1_760_000_000),
            ("anthropic/claude-4-sonnet", "Claude 4 Sonnet", 200_000, 3.00, 15.00, 0.30, 1_747_900_000),
            ("anthropic/claude-sonnet-4", "Claude Sonnet 4", 200_000, 3.00, 15.00, 0.30, 1_747_890_000),
            ("anthropic/claude-4-opus", "Claude 4 Opus", 200_000, 15.00, 75.00, 1.50, 1_747_880_000),
            // Google Gemini 4 / Gemini 3.8-3.5 / Gemini 2.5 series
            ("google/gemini-4-carbon", "Gemini 4 Carbon", 1_048_576, 2.00, 10.00, 0.10, 1_794_326_871),
            ("google/gemini-4-argon", "Gemini 4 Argon", 1_048_576, 2.00, 10.00, 0.10, 1_793_700_000),
            ("google/gemini-omni-1.2-flash", "Gemini Omni 1.2 Flash", 131_072, 1.50, 9.00, 0.15, 1_793_898_961),
            ("google/gemini-3.8-flash", "Gemini 3.8 Flash", 1_048_576, 0.825, 4.125, 0.0825, 1_791_200_000),
            ("google/gemini-3.7-flash", "Gemini 3.7 Flash", 1_048_576, 0.75, 3.75, 0.075, 1_789_500_000),
            ("google/gemini-3.6-flash", "Gemini 3.6 Flash", 1_048_576, 0.75, 3.75, 0.075, 1_787_000_000),
            ("google/gemini-3.5-flash-lite", "Gemini 3.5 Flash-Lite", 1_048_576, 0.30, 2.50, 0.03, 1_785_000_000),
            ("google/gemini-3.1-pro", "Gemini 3.1 Pro", 1_048_576, 2.00, 12.00, 0.20, 1_775_000_000),
            ("google/gemini-2.5-pro", "Gemini 2.5 Pro", 1_048_576, 1.25, 10.00, 0.31, 1_745_000_000),
            ("google/gemini-2.5-flash", "Gemini 2.5 Flash", 1_048_576, 0.15, 0.60, 0.0375, 1_744_900_000),
            // DeepSeek V4.1 / V4 / V3.2
            ("deepseek/deepseek-v4.1-flash", "DeepSeek V4.1 Flash", 1_048_576, 0.30, 1.20, 0.006, 1_792_225_624),
            ("deepseek/deepseek-v4-pro", "DeepSeek V4 Pro", 1_048_576, 0.889, 1.778, 0.074, 1_789_000_000),
            ("deepseek/deepseek-v4-flash", "DeepSeek V4 Flash", 1_048_576, 0.091, 0.182, 0.0182, 1_788_500_000),
            ("deepseek/deepseek-v3.2", "DeepSeek V3.2", 163_840, 0.26, 0.38, 0.13, 1_770_000_000),
            ("deepseek/deepseek-reasoner", "DeepSeek Reasoner", 163_840, 0.55, 2.19, 0.14, 1_765_000_000),
            ("deepseek/deepseek-chat", "DeepSeek Chat", 163_840, 0.27, 1.10, 0.07, 1_764_900_000),
            // xAI Grok 4.7 / 4.6 / Build
            ("x-ai/grok-4.7", "Grok 4.7", 500_000, 2.00, 6.00, 0.50, 1_793_100_000),
            ("x-ai/grok-4.6", "Grok 4.6", 500_000, 2.00, 6.00, 0.50, 1_791_000_000),
            ("x-ai/grok-4.5", "Grok 4.5", 500_000, 2.00, 6.00, 0.30, 1_788_000_000),
            ("x-ai/grok-build-0.1", "Grok Build 0.1", 256_000, 1.00, 2.00, 0.20, 1_787_500_000),
            ("x-ai/grok-4.3", "Grok 4.3", 1_000_000, 1.25, 2.50, 0.20, 1_785_000_000),
            // Qwen 3.8 / Moonshot Kimi K3.1 / Z.ai GLM 5.3 / Meta Muse
            ("qwen/qwen3.8-max-prime", "Qwen 3.8 Max Prime", 1_000_000, 4.00, 12.00, 0.50, 1_793_000_000),
            ("qwen/qwen3.8-max-0902", "Qwen 3.8 Max", 1_000_000, 2.00, 6.00, 0.25, 1_790_000_000),
            ("qwen/qwen3.8-flash", "Qwen 3.8 Flash", 1_000_000, 0.15, 0.47, 0.016, 1_789_900_000),
            ("moonshotai/kimi-k3.1-pro", "Kimi K3.1 Pro", 1_048_576, 2.00, 10.00, 0.20, 1_792_778_188),
            ("moonshotai/kimi-k2.7-code", "Kimi K2.7 Code", 262_144, 0.60, 3.50, 0.16, 1_788_200_000),
            ("z-ai/glm-5.3-flashx-legal", "GLM 5.3 FlashX", 1_048_576, 0.37, 1.25, 0.075, 1_793_626_154),
            ("meta/muse-spark-open", "Meta Muse Spark", 1_048_576, 1.25, 4.25, 0.15, 1_792_087_069),
        ];

        for &(id, display_name, ctx, inp, outp, cache, created) in seeds {
            catalog.upsert_entry(ModelCatalogEntry {
                id: id.to_string(),
                display_name: display_name.to_string(),
                context_length: ctx,
                input_per_million: inp,
                output_per_million: outp,
                cached_per_million: cache,
                created_unix: created,
            });
        }
        catalog
    }

    pub fn upsert_entry(&mut self, entry: ModelCatalogEntry) {
        let canonical = entry.id.trim().to_ascii_lowercase();
        if canonical.is_empty() || canonical.ends_with(":batch") || canonical.ends_with(":free") {
            return;
        }
        let bare = canonical
            .split_once('/')
            .map(|(_, slug)| slug.to_string())
            .unwrap_or_else(|| canonical.clone());

        let replace_bare = match self
            .bare_index
            .get(&bare)
            .and_then(|existing_id| self.entries.get(existing_id))
        {
            Some(existing) => entry.created_unix >= existing.created_unix,
            None => true,
        };
        if replace_bare {
            self.bare_index.insert(bare, canonical.clone());
        }
        self.entries.insert(canonical, entry);
    }

    /// Parses live `https://openrouter.ai/api/v1/models` JSON (`{"data": [...]}`), merging all valid
    /// metered models over `embedded_baseline()` so newly released frontier models are recognized automatically.
    pub fn parse_openrouter_models_json(raw_json: &str, fetched_at_ms: u64) -> Result<Self, String> {
        let parsed: Value = serde_json::from_str(raw_json)
            .map_err(|e| format!("Invalid model catalog JSON: {e}"))?;
        let data = parsed
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| "Model catalog JSON missing 'data' array".to_string())?;

        let mut catalog = Self::embedded_baseline();
        catalog.fetched_at_ms = fetched_at_ms;
        catalog.source = "live_openrouter_v1".to_string();

        let mut ingested = 0usize;
        for item in data {
            let Some(raw_id) = item.get("id").and_then(Value::as_str) else {
                continue;
            };
            let norm_id = raw_id.trim().to_ascii_lowercase();
            if norm_id.ends_with(":batch") || norm_id.ends_with(":free") || norm_id.starts_with("openrouter/auto") {
                continue;
            }
            let Some(pricing) = item.get("pricing").and_then(Value::as_object) else {
                continue;
            };
            let Some(prompt_per_tok) = parse_decimal_field(pricing.get("prompt")) else {
                continue;
            };
            let Some(comp_per_tok) = parse_decimal_field(pricing.get("completion")) else {
                continue;
            };
            if prompt_per_tok < 0.0 || comp_per_tok < 0.0 || (prompt_per_tok == 0.0 && comp_per_tok == 0.0) {
                continue;
            }
            let cache_per_tok = parse_decimal_field(
                pricing
                    .get("input_cache_read")
                    .or_else(|| pricing.get("inputCacheRead")),
            )
            .filter(|v| *v >= 0.0)
            .unwrap_or(prompt_per_tok);

            let display_name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(raw_id)
                .to_string();
            let context_length = item
                .get("context_length")
                .and_then(Value::as_u64)
                .unwrap_or(128_000) as usize;
            let created_unix = item.get("created").and_then(Value::as_u64).unwrap_or(0);

            catalog.upsert_entry(ModelCatalogEntry {
                id: norm_id,
                display_name,
                context_length,
                input_per_million: prompt_per_tok * 1_000_000.0,
                output_per_million: comp_per_tok * 1_000_000.0,
                cached_per_million: cache_per_tok * 1_000_000.0,
                created_unix,
            });
            ingested += 1;
        }

        if ingested == 0 {
            return Err("Model catalog JSON contained 0 valid priced models".to_string());
        }
        Ok(catalog)
    }

    pub fn canonical_vendor_prefix(provider: &str) -> &'static str {
        let p = provider.trim().to_ascii_lowercase();
        if p.contains("anthropic") || p.contains("claude") {
            "anthropic"
        } else if p.contains("gemini") || p == "google" {
            "google"
        } else if p == "deepseek" {
            "deepseek"
        } else if p == "xai" || p == "x-ai" || p.contains("grok") {
            "x-ai"
        } else if p == "qwen" {
            "qwen"
        } else if p == "moonshot" || p == "moonshotai" || p == "kimi" {
            "moonshotai"
        } else {
            "openai"
        }
    }

    pub fn lookup(&self, provider: &str, model: &str) -> Option<&ModelCatalogEntry> {
        let norm_model = model
            .trim()
            .to_ascii_lowercase()
            .trim_end_matches(":latest")
            .to_string();
        if norm_model.is_empty() {
            return None;
        }
        let vendor = Self::canonical_vendor_prefix(provider);

        // 1. Direct canonical match (`vendor/slug`) or provider-prefixed match
        if let Some(hit) = self.entries.get(&norm_model) {
            return Some(hit);
        }
        let with_vendor = format!("{vendor}/{norm_model}");
        if let Some(hit) = self.entries.get(&with_vendor) {
            return Some(hit);
        }
        if let Some(canon) = self.bare_index.get(&norm_model) {
            if let Some(hit) = self.entries.get(canon) {
                return Some(hit);
            }
        }

        // 2. Strip trailing snapshot date (`-20261015`, `-20250514`, `-1017`, `-latest`, `-preview`)
        let stripped = strip_model_snapshot_suffix(&norm_model);
        if stripped != norm_model {
            let vendor_stripped = format!("{vendor}/{stripped}");
            if let Some(hit) = self.entries.get(&vendor_stripped) {
                return Some(hit);
            }
            if let Some(canon) = self.bare_index.get(&stripped) {
                if let Some(hit) = self.entries.get(canon) {
                    return Some(hit);
                }
            }
        }
        None
    }

    /// Dynamically selects the newest fast-tier and strong-tier models for `provider` from the catalog.
    pub fn recommend_tiers(&self, provider: &str) -> (String, String) {
        let p = provider.trim().to_ascii_lowercase();
        if p == "ollama" {
            return ("qwen2.5-coder:14b".to_string(), "qwen2.5-coder:14b".to_string());
        }
        let vendor_prefix = format!("{}/", Self::canonical_vendor_prefix(provider));
        let use_full_id = p == "openrouter";

        let mut newest_fast: Option<&ModelCatalogEntry> = None;
        let mut newest_strong: Option<&ModelCatalogEntry> = None;

        for entry in self.entries.values() {
            if !entry.id.starts_with(&vendor_prefix) || entry.context_length < 128_000 {
                continue;
            }
            let slug = entry.id.split_once('/').map(|(_, s)| s).unwrap_or(&entry.id);
            if slug.contains("image") || slug.contains("audio") || slug.contains("embed") || slug.contains("vision-exp") {
                continue;
            }
            if entry.input_per_million > 0.0 && entry.input_per_million <= 0.90 {
                if newest_fast.map_or(true, |cur| entry.created_unix > cur.created_unix) {
                    newest_fast = Some(entry);
                }
            } else if entry.input_per_million > 0.90 && entry.input_per_million <= 10.0 {
                if newest_strong.map_or(true, |cur| entry.created_unix > cur.created_unix) {
                    newest_strong = Some(entry);
                }
            }
        }

        let format_id = |e: &ModelCatalogEntry| -> String {
            if use_full_id {
                e.id.clone()
            } else {
                e.id.split_once('/').map(|(_, s)| s.to_string()).unwrap_or_else(|| e.id.clone())
            }
        };

        let (default_fast, default_strong) = match Self::canonical_vendor_prefix(provider) {
            "anthropic" => ("claude-haiku-5.5", "claude-sonnet-5.5"),
            "google" => ("gemini-3.8-flash", "gemini-4-carbon"),
            "deepseek" => ("deepseek-v4.1-flash", "deepseek-v4-pro"),
            "x-ai" => ("grok-build-0.1", "grok-4.7"),
            "qwen" => ("qwen3.8-flash", "qwen3.8-max-prime"),
            "moonshotai" => ("kimi-k2.7-code", "kimi-k3.1-pro"),
            _ => ("gpt-6-luna", "gpt-6.1-terra"),
        };

        let fast = newest_fast.map(format_id).unwrap_or_else(|| default_fast.to_string());
        let strong = newest_strong.map(format_id).unwrap_or_else(|| default_strong.to_string());
        (fast, strong)
    }

    /// Returns the newest `limit` models for a provider family, sorted newest-first by `created_unix`.
    pub fn newest_models_for_provider(&self, provider: &str, limit: usize) -> Vec<&ModelCatalogEntry> {
        let p = provider.trim().to_ascii_lowercase();
        let mut matches: Vec<&ModelCatalogEntry> = if p == "openrouter" {
            self.entries.values().collect()
        } else {
            let prefix = format!("{}/", Self::canonical_vendor_prefix(provider));
            self.entries.values().filter(|e| e.id.starts_with(&prefix)).collect()
        };
        matches.sort_by(|a, b| b.created_unix.cmp(&a.created_unix).then_with(|| a.id.cmp(&b.id)));
        matches.truncate(limit);
        matches
    }
}

fn parse_decimal_field(val: Option<&Value>) -> Option<f64> {
    match val? {
        Value::String(s) => s.trim().parse::<f64>().ok().filter(|v| v.is_finite()),
        Value::Number(n) => n.as_f64().filter(|v| v.is_finite()),
        _ => None,
    }
}

fn strip_model_snapshot_suffix(slug: &str) -> String {
    let mut s = slug
        .trim_end_matches("-latest")
        .trim_end_matches("-preview")
        .to_string();
    if let Some((base, tail)) = s.rsplit_once('-') {
        let all_digits = !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit());
        if all_digits && (tail.len() == 8 || tail.len() == 4) {
            s = base.to_string();
        }
    }
    s
}
