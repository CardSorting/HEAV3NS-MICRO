// [LAYER: DOMAIN]
use crate::domain::types::model_catalog::ModelPricingCatalog;
use crate::plumbing::hash_and_math::fnv1a_32;
use serde::{Deserialize, Serialize};

/// Abstract contract for honest provider token billing & error envelope classification.
pub trait IBillingMeter {
    fn honest_cost_summary(&self) -> String;
}

/// Abstract contract for structured HTTP/provider error recoverability classification.
pub trait IErrorClassifier {
    fn is_recoverable_error(&self) -> bool;
}

/// Exact or estimated token usage returned from an LLM completion call (`Pricing & Receipts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderTokenUsage {
    #[serde(rename = "promptTokens")]
    pub prompt_tokens: usize,
    #[serde(rename = "completionTokens")]
    pub completion_tokens: usize,
    #[serde(rename = "cachedPromptTokens")]
    pub cached_prompt_tokens: usize,
    #[serde(rename = "isExact")]
    pub is_exact: bool,
}

impl ProviderTokenUsage {
    pub fn accumulate(&mut self, other: &Self) {
        self.prompt_tokens = self.prompt_tokens.saturating_add(other.prompt_tokens);
        self.completion_tokens = self.completion_tokens.saturating_add(other.completion_tokens);
        self.cached_prompt_tokens = self.cached_prompt_tokens.saturating_add(other.cached_prompt_tokens);
        self.is_exact = self.is_exact && other.is_exact;
    }
}

/// Codewhale Honest Pricing invariant (`pricing.rs`): an unrecognized model is NEVER shown as `$0.00`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingMetering {
    Metered {
        usd_cost: f64,
        input_per_million: f64,
        output_per_million: f64,
        cached_per_million: f64,
    },
    LocalNoBill,
    Subscription,
    Unknown,
}

impl BillingMetering {
    /// Evaluates token billing against the embedded 2026 frontier baseline (pure zero-I/O entrypoint).
    pub fn evaluate(provider: &str, model: &str, endpoint: &str, usage: &ProviderTokenUsage) -> Self {
        let baseline = ModelPricingCatalog::embedded_baseline();
        Self::evaluate_with_catalog(provider, model, endpoint, usage, &baseline)
    }

    /// Evaluates token billing against a live/cached [`ModelPricingCatalog`] (`https://openrouter.ai/api/v1/models`).
    /// Unrecognized models strictly return [`BillingMetering::Unknown`] (`"unknown"`), never `$0.0000`.
    pub fn evaluate_with_catalog(
        provider: &str,
        model: &str,
        endpoint: &str,
        usage: &ProviderTokenUsage,
        catalog: &ModelPricingCatalog,
    ) -> Self {
        let p = provider.trim().to_ascii_lowercase();
        let m = model.trim().to_ascii_lowercase();
        let ep = endpoint.trim().to_ascii_lowercase();

        if p == "ollama"
            || p.contains("apple")
            || m.starts_with("apple-")
            || m.ends_with(":local")
            || ep.contains("127.0.0.1:11434")
            || ep.contains("localhost:11434")
        {
            return Self::LocalNoBill;
        }
        if p == "openai-codex" || p.contains("subscription") || p == "scripted" {
            return Self::Subscription;
        }

        let Some(entry) = catalog.lookup(provider, model) else {
            return Self::Unknown;
        };

        let in_rate = entry.input_per_million;
        let out_rate = entry.output_per_million;
        let cache_rate = entry.cached_per_million;

        let uncached_in = usage.prompt_tokens.saturating_sub(usage.cached_prompt_tokens);
        let raw_usd = (uncached_in as f64 * in_rate / 1_000_000.0)
            + (usage.cached_prompt_tokens as f64 * cache_rate / 1_000_000.0)
            + (usage.completion_tokens as f64 * out_rate / 1_000_000.0);
        let usd_cost = if raw_usd.is_finite() && raw_usd >= 0.0 { raw_usd } else { 0.0 };

        Self::Metered {
            usd_cost,
            input_per_million: in_rate,
            output_per_million: out_rate,
            cached_per_million: cache_rate,
        }
    }

    pub fn format_honest(&self) -> String {
        match self {
            Self::Metered { usd_cost, .. } => format!("${usd_cost:.4}"),
            Self::LocalNoBill => "$0.0000 (local)".to_string(),
            Self::Subscription => "subscription".to_string(),
            Self::Unknown => "unknown".to_string(),
        }
    }

    pub fn kind_slug(&self) -> &'static str {
        match self {
            Self::Metered { .. } => "metered",
            Self::LocalNoBill => "local_no_bill",
            Self::Subscription => "subscription",
            Self::Unknown => "unknown",
        }
    }
}

/// Structured error taxonomy distilled from Codewhale (`crates/error-taxonomy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Authentication,
    Authorization,
    Budget,
    RateLimit,
    Timeout,
    Network,
    ContextOverflow,
    InvalidInput,
    ServerTransient,
    Cancelled,
}

pub const CREDENTIAL_REJECTED_UNSENT_CODE: &str = "llm_auth_rejected_unsent";

/// Actionable error envelope distinguishing non-recoverable 402/spent-balance from retryable 429 rate limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub category: ErrorCategory,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    pub remediation: String,
}

impl ErrorEnvelope {
    pub fn classify_http(status: u16, body: &str, had_side_effects: bool) -> Self {
        let lower = body.to_ascii_lowercase();
        let is_spent_balance = status == 402
            || lower.contains("insufficient_quota")
            || lower.contains("credit balance is too low")
            || lower.contains("billing_hard_limit_reached")
            || lower.contains("exceeded your current quota");

        if is_spent_balance {
            return Self {
                category: ErrorCategory::Budget,
                recoverable: false,
                code: Some("spent_balance_or_quota".to_string()),
                message: format!("HTTP {status}: Account quota or credit balance exhausted."),
                remediation: "Top up provider billing credits or switch provider/model via `/model` (`F3`).".to_string(),
            };
        }

        match status {
            401 => Self {
                category: ErrorCategory::Authentication,
                recoverable: false,
                code: if !had_side_effects {
                    Some(CREDENTIAL_REJECTED_UNSENT_CODE.to_string())
                } else {
                    Some("auth_invalid_key".to_string())
                },
                message: "HTTP 401: Missing or rejected API key.".to_string(),
                remediation: "Configure a valid key via `heav3ns-micro init`, environment variable, or Model Picker (`F3`).".to_string(),
            },
            403 => Self {
                category: ErrorCategory::Authorization,
                recoverable: false,
                code: if !had_side_effects {
                    Some(CREDENTIAL_REJECTED_UNSENT_CODE.to_string())
                } else {
                    Some("auth_forbidden".to_string())
                },
                message: "HTTP 403: Provider refused access to the requested model or endpoint.".to_string(),
                remediation: "Verify model access permissions or switch models with `/model`.".to_string(),
            },
            429 => Self {
                category: ErrorCategory::RateLimit,
                recoverable: true,
                code: Some("rate_limited".to_string()),
                message: "HTTP 429: Provider rate limit reached; applying jittered backoff.".to_string(),
                remediation: "Automatic exponential backoff active; switch to a secondary model if sustained.".to_string(),
            },
            400 | 413 if lower.contains("context_length") || lower.contains("too many tokens") || lower.contains("maximum context") => Self {
                category: ErrorCategory::ContextOverflow,
                recoverable: false,
                code: Some("context_window_exceeded".to_string()),
                message: format!("HTTP {status}: Request exceeded model context window."),
                remediation: "Run `/compact` to compress session history or switch to a larger-context model.".to_string(),
            },
            408 | 504 => Self {
                category: ErrorCategory::Timeout,
                recoverable: true,
                code: Some("upstream_timeout".to_string()),
                message: format!("HTTP {status}: Upstream gateway timeout."),
                remediation: "Retrying with bounded backoff.".to_string(),
            },
            500 | 502 | 503 | 529 => Self {
                category: ErrorCategory::ServerTransient,
                recoverable: true,
                code: Some("upstream_transient".to_string()),
                message: format!("HTTP {status}: Transient provider capacity/server error."),
                remediation: "Retrying with jittered backoff.".to_string(),
            },
            _ => Self {
                category: ErrorCategory::InvalidInput,
                recoverable: false,
                code: Some(format!("http_{status}")),
                message: format!("HTTP {status}: {body}"),
                remediation: "Inspect model name (`heav3ns-micro doctor`) and outbound payload (`/preview-request`).".to_string(),
            },
        }
    }
}

/// Computes bounded exponential backoff with deterministic `50%..100%` jitter, honoring `Retry-After` headers.
pub fn compute_retry_delay_ms(
    attempt: u32,
    category: ErrorCategory,
    retry_after_ms: Option<u64>,
    is_scripted_provider: bool,
) -> u64 {
    if is_scripted_provider {
        return 1;
    }
    if let Some(ra) = retry_after_ms {
        return ra.clamp(50, 30_000);
    }
    let base_ms: u64 = match category {
        ErrorCategory::RateLimit => 600u64.saturating_mul(1u64 << attempt.min(5)),
        _ => 250u64.saturating_mul(1u64 << attempt.min(4)),
    };
    let capped = base_ms.min(15_000);
    let jitter_pct = 50 + (fnv1a_32(&attempt.to_le_bytes()) as u64 % 51);
    (capped * jitter_pct) / 100
}

/// Same-provider model routing receipt (`/model auto`) — NEVER cross-routes incompatible provider model IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoRouteDecision {
    pub selected_model: String,
    pub tier: String,
    pub reason: String,
}

/// Resolves a safe fallback model strictly compatible with the active provider using the embedded baseline.
pub fn resolve_compatible_fallback_model(
    provider: &str,
    primary_model: &str,
    configured_fallback: Option<&str>,
) -> Option<String> {
    let baseline = ModelPricingCatalog::embedded_baseline();
    resolve_compatible_fallback_model_with_catalog(provider, primary_model, configured_fallback, &baseline)
}

/// Resolves a safe fallback model strictly compatible with the active provider using the live/cached [`ModelPricingCatalog`].
pub fn resolve_compatible_fallback_model_with_catalog(
    provider: &str,
    primary_model: &str,
    configured_fallback: Option<&str>,
    catalog: &ModelPricingCatalog,
) -> Option<String> {
    let p = provider.trim().to_ascii_lowercase();
    if p == "scripted" || p == "ollama" {
        return configured_fallback
            .filter(|f| *f != primary_model && p == "scripted")
            .map(String::from);
    }
    if let Some(fb) = configured_fallback {
        let fb_low = fb.to_ascii_lowercase();
        let cross_provider_mismatch = (p.contains("anthropic") && !fb_low.contains("claude"))
            || ((p.contains("gemini") || p == "google") && !fb_low.contains("gemini"))
            || (p == "deepseek" && !fb_low.contains("deepseek"))
            || ((p == "xai" || p == "x-ai") && !fb_low.contains("grok"));
        if !cross_provider_mismatch && fb != primary_model {
            return Some(fb.to_string());
        }
    }
    let (fast, strong) = catalog.recommend_tiers(provider);
    if fast != primary_model {
        Some(fast)
    } else if strong != primary_model {
        Some(strong)
    } else {
        None
    }
}

/// Evaluates `/model auto` routing within the active provider family using the embedded baseline.
pub fn select_auto_routed_model(provider: &str, configured_model: &str, prompt: &str) -> AutoRouteDecision {
    let baseline = ModelPricingCatalog::embedded_baseline();
    select_auto_routed_model_with_catalog(provider, configured_model, prompt, &baseline)
}

/// Evaluates `/model auto` routing dynamically against the live/refreshed [`ModelPricingCatalog`],
/// automatically selecting the newest fast or strong frontier model for the active provider.
pub fn select_auto_routed_model_with_catalog(
    provider: &str,
    configured_model: &str,
    prompt: &str,
    catalog: &ModelPricingCatalog,
) -> AutoRouteDecision {
    if !configured_model.eq_ignore_ascii_case("auto") {
        return AutoRouteDecision {
            selected_model: configured_model.to_string(),
            tier: "declared".to_string(),
            reason: "Explicitly configured model".to_string(),
        };
    }
    let lower = prompt.to_ascii_lowercase();
    let needs_deep = prompt.len() > 600
        || lower.contains("architect")
        || lower.contains("refactor")
        || lower.contains("audit")
        || lower.contains("bug")
        || lower.contains("security")
        || lower.contains("implement");

    let (fast, strong) = catalog.recommend_tiers(provider);

    if needs_deep {
        AutoRouteDecision {
            selected_model: strong,
            tier: "strong".to_string(),
            reason: format!("Multi-step engineering/audit complexity routed via {}", catalog.source),
        }
    } else {
        AutoRouteDecision {
            selected_model: fast,
            tier: "fast".to_string(),
            reason: format!("Direct lightweight query routed to fast tier via {}", catalog.source),
        }
    }
}
