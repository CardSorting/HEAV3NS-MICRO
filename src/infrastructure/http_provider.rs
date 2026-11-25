// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{BoxFuture, IHttpProvider, ILlmProvider, ProgressCallback};
use crate::domain::types::progress::EngineProgressPhase;
use crate::domain::types::provider_registry::{
    canonicalize_provider_id, infer_wire_dialect_from_endpoint, lookup_provider_descriptor,
    opencode_go_model_id, resolve_model_aware_wire_and_model, retired_provider_remediation,
    WireDialect,
};
use crate::domain::types::runtime::CancellationToken;
use crate::domain::types::tick_result::{
    is_retryable_http_status, ProviderError, ProviderRequest,
    ProviderResponse,
};
use crate::infrastructure::codex_oauth::{
    build_codex_oauth_post, resolve_codex_base_url, CodexAccountStore,
};
use crate::infrastructure::sse_and_response_parsers::parse_responses_json;
use crate::infrastructure::wire_dialects::{
    build_anthropic_messages_payload, build_chat_completions_payload_with_endpoint,
    build_google_gemini_payload, build_openai_responses_payload,
};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

fn shared_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .pool_max_idle_per_host(16)
            .tcp_nodelay(true)
            .build()
            .unwrap_or_default()
    })
}

/// Hard cap (`16 MiB`) on buffered HTTP/SSE response bodies to prevent OOM from runaway upstream streams.
pub const MAX_HTTP_RESPONSE_BODY_BYTES: usize = 16 * 1024 * 1024;
pub use crate::infrastructure::http_provider_helpers::{
    parse_sse_stream, probe_local_ollama_model, probe_local_ollama_models, probe_openai_compatible_models,
};
use crate::infrastructure::http_provider_helpers::{
    emit_stream_progress, is_event_stream_content_type, is_non_loopback_cleartext_http, looks_like_sse,
    parse_anthropic_response, parse_gemini_response, parse_openai_response, parse_retry_after_header,
    read_bounded_response_text, stream_incremental_sse_response,
};

/// Sovereign 4-dialect HTTP LLM adapter supporting OpenAI Chat Completions, OpenAI `/v1/responses`,
/// Anthropic `/v1/messages`, and Google Gemini `:generateContent` across 65+ providers.
pub struct HttpProvider {
    name: String,
    endpoint: String,
    api_key: Option<String>,
    timeout_ms: u64,
    wire_dialect: WireDialect,
    explicit_wire_override: bool,
    client: reqwest::Client,
}

impl HttpProvider {
    pub extern "Rust" fn new(
        name: impl Into<String>,
        endpoint: impl Into<String>,
        api_key: Option<String>,
        timeout_ms: u64,
    ) -> Self {
        let name_str = name.into();
        let endpoint_str = endpoint.into();
        let wire_dialect = lookup_provider_descriptor(&name_str)
            .map(|d| d.wire_dialect)
            .unwrap_or_else(|| infer_wire_dialect_from_endpoint(&endpoint_str));
        let bounded_timeout = timeout_ms.max(1_000);
        let client = if bounded_timeout >= 15_000 {
            shared_http_client().clone()
        } else {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_millis(bounded_timeout))
                .pool_max_idle_per_host(16)
                .tcp_nodelay(true)
                .build()
                .unwrap_or_default()
        };
        Self {
            name: name_str,
            endpoint: endpoint_str,
            api_key,
            timeout_ms: bounded_timeout,
            wire_dialect,
            explicit_wire_override: false,
            client,
        }
    }

    pub fn with_wire_dialect(mut self, wire_dialect: WireDialect) -> Self {
        self.wire_dialect = wire_dialect;
        self
    }

    pub fn with_explicit_wire_override(mut self, explicit: bool) -> Self {
        self.explicit_wire_override = explicit;
        self
    }

    #[inline]
    pub const fn wire_dialect(&self) -> WireDialect {
        self.wire_dialect
    }

    pub(crate) fn resolve_request_route(&self, raw_model: &str) -> (WireDialect, String) {
        let (model_wire, normalized_model) = resolve_model_aware_wire_and_model(
            &self.name,
            raw_model,
            &self.endpoint,
            self.wire_dialect,
        );
        if self.explicit_wire_override {
            (self.wire_dialect, normalized_model)
        } else {
            (model_wire, normalized_model)
        }
    }

    #[allow(dead_code)]
    pub(crate) fn resolve_url(&self, model: &str) -> String {
        let (dialect, wire_model) = self.resolve_request_route(model);
        self.resolve_url_for(dialect, &wire_model)
    }

    pub(crate) fn resolve_url_for(&self, dialect: WireDialect, model: &str) -> String {
        let trimmed = self.endpoint.trim_end_matches('/');
        match dialect {
            WireDialect::AnthropicMessages => {
                if trimmed.ends_with("/messages") {
                    trimmed.to_string()
                } else if trimmed.ends_with("/v1") {
                    format!("{trimmed}/messages")
                } else {
                    format!("{trimmed}/v1/messages")
                }
            }
            WireDialect::GoogleGemini => {
                if trimmed.contains(":generateContent") {
                    trimmed.to_string()
                } else if let Some((base, q)) = trimmed.split_once('?') {
                    format!(
                        "{}/models/{model}:generateContent?{q}",
                        base.trim_end_matches('/')
                    )
                } else {
                    format!("{trimmed}/models/{model}:generateContent")
                }
            }
            WireDialect::OpenAiResponses => {
                if trimmed.ends_with("/responses") {
                    trimmed.to_string()
                } else if canonicalize_provider_id(&self.name) == "deepseek"
                    && matches!(
                        trimmed.to_ascii_lowercase().as_str(),
                        "https://api.deepseek.com"
                            | "https://api.deepseek.com/v1"
                            | "https://api.deepseek.com/beta"
                            | "https://api.deepseeki.com"
                            | "https://api.deepseeki.com/v1"
                            | "https://api.deepseeki.com/beta"
                    )
                {
                    let base = trimmed
                        .strip_suffix("/v1")
                        .or_else(|| trimmed.strip_suffix("/beta"))
                        .unwrap_or(trimmed);
                    format!("{base}/responses")
                } else if trimmed.ends_with("/v1") {
                    format!("{trimmed}/responses")
                } else {
                    format!("{trimmed}/v1/responses")
                }
            }
            WireDialect::ChatCompletions => {
                if trimmed.ends_with("/chat/completions") {
                    trimmed.to_string()
                } else if trimmed.ends_with("/v1") {
                    format!("{trimmed}/chat/completions")
                } else {
                    format!("{trimmed}/v1/chat/completions")
                }
            }
        }
    }

    pub(crate) fn build_openai_payload(&self, req: &ProviderRequest) -> Value {
        build_chat_completions_payload_with_endpoint(&self.name, &self.endpoint, req)
    }

    pub(crate) fn build_responses_payload(&self, req: &ProviderRequest) -> Value {
        build_openai_responses_payload(&self.name, req)
    }

    pub(crate) fn build_anthropic_payload(&self, req: &ProviderRequest) -> Value {
        build_anthropic_messages_payload(&self.name, &self.endpoint, req)
    }

    pub(crate) fn build_gemini_payload(&self, req: &ProviderRequest) -> Value {
        build_google_gemini_payload(req)
    }

    fn apply_auth_headers(
        &self,
        builder: reqwest::RequestBuilder,
        dialect: WireDialect,
    ) -> reqwest::RequestBuilder {
        let mut builder = builder;
        let canon = canonicalize_provider_id(&self.name);
        if canon == "openrouter" {
            builder = builder
                .header("HTTP-Referer", "https://github.com/heav3ns/heav3ns-micro")
                .header("X-Title", "HEAV3NS-MICRO");
        }
        let Some(key) = self
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
        else {
            return builder;
        };
        match dialect {
            WireDialect::AnthropicMessages => {
                builder = builder.header("anthropic-version", "2023-06-01");
                if canon == "codewhale" {
                    return builder.bearer_auth(key);
                }
                let is_oauth = key.starts_with("sk-ant-oat")
                    || (canon.contains("subscription") && !key.starts_with("sk-ant-api"));
                if is_oauth {
                    builder = builder
                        .bearer_auth(key)
                        .header("anthropic-beta", "oauth-2025-04-20");
                } else {
                    builder = builder.header("x-api-key", key);
                    if !self
                        .endpoint
                        .to_ascii_lowercase()
                        .contains("api.anthropic.com")
                    {
                        builder = builder.bearer_auth(key);
                    }
                }
                builder
            }
            WireDialect::GoogleGemini => builder.header("x-goog-api-key", key),
            WireDialect::OpenAiResponses | WireDialect::ChatCompletions => builder.bearer_auth(key),
        }
    }
}

impl IHttpProvider for HttpProvider {}

impl ILlmProvider for HttpProvider {
    fn provider_name(&self) -> &str {
        &self.name
    }

    fn complete<'a>(
        &'a self,
        req: ProviderRequest,
        cancel: Option<CancellationToken>,
    ) -> BoxFuture<'a, Result<ProviderResponse, ProviderError>> {
        self.complete_with_progress(req, cancel, None)
    }

    fn complete_with_progress<'a>(
        &'a self,
        req: ProviderRequest,
        cancel: Option<CancellationToken>,
        on_progress: Option<ProgressCallback>,
    ) -> BoxFuture<'a, Result<ProviderResponse, ProviderError>> {
        Box::pin(async move {
            if cancel.as_ref().map(|t| t.is_cancelled()).unwrap_or(false) {
                return Err(ProviderError::Cancelled);
            }
            if let Some(retired_msg) = retired_provider_remediation(&self.name) {
                return Err(ProviderError::HttpStatus {
                    status: 400,
                    body: retired_msg.to_string(),
                    retryable: false,
                    retry_after_ms: None,
                });
            }
            let canon_provider = canonicalize_provider_id(&self.name);
            if canon_provider == "opencode-go" && opencode_go_model_id(&req.model).is_none() {
                return Err(ProviderError::HttpStatus {
                    status: 400,
                    body: format!(
                        "Model '{}' is not in OpenCode Go's supported roster; check `heav3ns-micro providers` or select a documented OpenCode Go model.",
                        req.model
                    ),
                    retryable: false,
                    retry_after_ms: None,
                });
            }
            let has_key = self
                .api_key
                .as_deref()
                .is_some_and(|k| !k.trim().is_empty());
            if has_key && is_non_loopback_cleartext_http(&self.endpoint) {
                return Err(ProviderError::HttpStatus {
                    status: 400,
                    body: format!("Refusing to transmit API key for '{}' over non-loopback cleartext HTTP; use HTTPS.", self.name),
                    retryable: false,
                    retry_after_ms: None,
                });
            }

            let (effective_wire, wire_model) = self.resolve_request_route(&req.model);
            let mut effective_req = req;
            effective_req.model = wire_model;
            let mut url = self.resolve_url_for(effective_wire, &effective_req.model);
            let mut payload = match effective_wire {
                WireDialect::AnthropicMessages => self.build_anthropic_payload(&effective_req),
                WireDialect::GoogleGemini => self.build_gemini_payload(&effective_req),
                WireDialect::OpenAiResponses => self.build_responses_payload(&effective_req),
                WireDialect::ChatCompletions => self.build_openai_payload(&effective_req),
            };

            let has_explicit_sk = self
                .api_key
                .as_deref()
                .is_some_and(|k| k.trim().starts_with("sk-"));
            let has_any_api_key = self
                .api_key
                .as_deref()
                .is_some_and(|k| !k.trim().is_empty());
            let use_codex_oauth = self.endpoint.trim_end_matches('/')
                == "https://api.openai.com/v1"
                && ((canon_provider == "openai-codex" && !has_explicit_sk)
                    || (canon_provider == "openai" && !has_any_api_key));

            let http_fut = async {
                let codex_oauth = if use_codex_oauth {
                    CodexAccountStore::default_store()
                        .get_valid_active_credentials()
                        .await
                        .map_err(ProviderError::Transport)?
                } else {
                    None
                };
                let sse_provider_name: &str = if codex_oauth.is_some() {
                    "openai-codex"
                } else {
                    &self.name
                };
                let builder = if let Some((ref oauth_token, ref account_id)) = codex_oauth {
                    url = format!("{}/responses", resolve_codex_base_url());
                    if effective_wire != WireDialect::OpenAiResponses || canon_provider != "openai-codex" {
                        payload = build_openai_responses_payload("openai-codex", &effective_req);
                    }
                    if let Some(obj) = payload.as_object_mut() {
                        obj.remove("temperature");
                        obj.insert("stream".to_string(), Value::Bool(true));
                    }
                    build_codex_oauth_post(&self.client, &url, oauth_token, account_id.as_deref())
                } else {
                    if matches!(
                        effective_wire,
                        WireDialect::ChatCompletions
                            | WireDialect::OpenAiResponses
                            | WireDialect::AnthropicMessages
                    ) {
                        if let Some(obj) = payload.as_object_mut() {
                            obj.entry("stream".to_string())
                                .or_insert(Value::Bool(true));
                        }
                    }
                    self.apply_auth_headers(
                        self.client
                            .post(&url)
                            .header("content-type", "application/json"),
                        effective_wire,
                    )
                };
                emit_stream_progress(
                    on_progress.as_ref(),
                    EngineProgressPhase::Connecting,
                    "Waiting for model response…".to_string(),
                    None,
                    0,
                );
                let resp = builder
                    .json(&payload)
                    .send()
                    .await
                    .map_err(|e| ProviderError::Transport(e.without_url().to_string()))?;
                let status = resp.status().as_u16();
                let retry_after_ms = parse_retry_after_header(resp.headers());
                let content_type = resp
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .map(String::from);

                if (200..300).contains(&status)
                    && is_event_stream_content_type(content_type.as_deref())
                {
                    return stream_incremental_sse_response(
                        resp,
                        sse_provider_name,
                        MAX_HTTP_RESPONSE_BODY_BYTES,
                        on_progress.as_ref(),
                    )
                    .await;
                }

                let text = read_bounded_response_text(resp, MAX_HTTP_RESPONSE_BODY_BYTES).await?;

                if !(200..300).contains(&status) {
                    if effective_wire == WireDialect::ChatCompletions && status == 400 {
                        let mut needs_retry = false;
                        let mut healed_payload = payload.clone();
                        if text.contains("max_completion_tokens")
                            && healed_payload.get("max_tokens").is_some()
                        {
                            if let Some(tokens) = healed_payload
                                .as_object_mut()
                                .and_then(|o| o.remove("max_tokens"))
                            {
                                healed_payload["max_completion_tokens"] = tokens;
                                needs_retry = true;
                            }
                        }
                        if text.contains("reasoning_effort")
                            && text.contains("set reasoning_effort to 'none'")
                        {
                            healed_payload["reasoning_effort"] = Value::String("none".to_string());
                            needs_retry = true;
                        }
                        if text.contains("Unsupported parameter: 'temperature'") {
                            if let Some(obj) = healed_payload.as_object_mut() {
                                obj.remove("temperature");
                                needs_retry = true;
                            }
                        }
                        if needs_retry {
                            let retry_builder = self.apply_auth_headers(
                                self.client
                                    .post(&url)
                                    .header("content-type", "application/json"),
                                effective_wire,
                            );
                            let retry_resp = retry_builder
                                .json(&healed_payload)
                                .send()
                                .await
                                .map_err(|e| {
                                    ProviderError::Transport(e.without_url().to_string())
                                })?;
                            let retry_status = retry_resp.status().as_u16();
                            let retry_ra = parse_retry_after_header(retry_resp.headers());
                            let retry_ct = retry_resp
                                .headers()
                                .get("content-type")
                                .and_then(|v| v.to_str().ok())
                                .map(String::from);
                            if (200..300).contains(&retry_status)
                                && is_event_stream_content_type(retry_ct.as_deref())
                            {
                                return stream_incremental_sse_response(
                                    retry_resp,
                                    "openai-codex",
                                    MAX_HTTP_RESPONSE_BODY_BYTES,
                                    on_progress.as_ref(),
                                )
                                .await;
                            }
                            let retry_text = read_bounded_response_text(
                                retry_resp,
                                MAX_HTTP_RESPONSE_BODY_BYTES,
                            )
                            .await?;
                            if (200..300).contains(&retry_status) {
                                if looks_like_sse(&retry_text, retry_ct.as_deref()) {
                                    return parse_sse_stream(&retry_text, &self.name);
                                }
                                let json_body: Value =
                                    serde_json::from_str(&retry_text).map_err(|e| {
                                        ProviderError::Transport(format!(
                                            "Invalid JSON from provider: {e}"
                                        ))
                                    })?;
                                return parse_openai_response(&json_body);
                            }
                            return Err(ProviderError::HttpStatus {
                                status: retry_status,
                                body: retry_text,
                                retryable: is_retryable_http_status(retry_status),
                                retry_after_ms: retry_ra,
                            });
                        }
                    }

                    if codex_oauth.is_some() && status == 429 {
                        CodexAccountStore::default_store().record_rate_limit(retry_after_ms);
                    }
                    if status == 401
                        && self.endpoint.trim_end_matches('/') == "https://api.openai.com/v1"
                        && matches!(canon_provider.as_str(), "openai" | "openai-codex")
                    {
                        let retry_creds: Option<(String, Option<String>)> = if let Some((ref rejected_tok, _)) = codex_oauth {
                            Some(
                                CodexAccountStore::default_store()
                                    .force_refresh_active(rejected_tok)
                                    .await
                                    .map_err(|e| ProviderError::HttpStatus {
                                        status: 401,
                                        body: e,
                                        retryable: false,
                                        retry_after_ms: None,
                                    })?,
                            )
                        } else {
                            CodexAccountStore::default_store()
                                .get_valid_active_credentials()
                                .await
                                .ok()
                                .flatten()
                        };
                        if let Some((new_token, new_account_id)) = retry_creds {
                            url = format!("{}/responses", resolve_codex_base_url());
                            if effective_wire != WireDialect::OpenAiResponses || canon_provider != "openai-codex" {
                                payload = build_openai_responses_payload("openai-codex", &effective_req);
                            }
                            if let Some(obj) = payload.as_object_mut() {
                                obj.remove("temperature");
                                obj.insert("stream".to_string(), Value::Bool(true));
                            }
                            let retry_b = build_codex_oauth_post(
                                &self.client,
                                &url,
                                &new_token,
                                new_account_id.as_deref(),
                            );
                            let retry_resp = retry_b.json(&payload).send().await.map_err(|e| {
                                ProviderError::Transport(e.without_url().to_string())
                            })?;
                            let retry_status = retry_resp.status().as_u16();
                            let retry_ra = parse_retry_after_header(retry_resp.headers());
                            let retry_ct = retry_resp
                                .headers()
                                .get("content-type")
                                .and_then(|v| v.to_str().ok())
                                .map(String::from);
                            if (200..300).contains(&retry_status)
                                && is_event_stream_content_type(retry_ct.as_deref())
                            {
                                return stream_incremental_sse_response(
                                    retry_resp,
                                    "openai-codex",
                                    MAX_HTTP_RESPONSE_BODY_BYTES,
                                    on_progress.as_ref(),
                                )
                                .await;
                            }
                            let retry_text = read_bounded_response_text(
                                retry_resp,
                                MAX_HTTP_RESPONSE_BODY_BYTES,
                            )
                            .await?;
                            if (200..300).contains(&retry_status) {
                                if looks_like_sse(&retry_text, retry_ct.as_deref()) {
                                    return parse_sse_stream(&retry_text, "openai-codex");
                                }
                                let json_body: Value =
                                    serde_json::from_str(&retry_text).map_err(|e| {
                                        ProviderError::Transport(format!(
                                            "Invalid JSON from provider: {e}"
                                        ))
                                    })?;
                                return parse_responses_json(&json_body, "openai-codex");
                            }
                            if retry_status == 429 {
                                CodexAccountStore::default_store().record_rate_limit(retry_ra);
                            }
                            return Err(ProviderError::HttpStatus {
                                status: retry_status,
                                body: retry_text,
                                retryable: is_retryable_http_status(retry_status),
                                retry_after_ms: retry_ra,
                            });
                        }
                    }

                    return Err(ProviderError::HttpStatus {
                        status,
                        body: text,
                        retryable: is_retryable_http_status(status),
                        retry_after_ms,
                    });
                }

                if looks_like_sse(&text, content_type.as_deref()) {
                    return parse_sse_stream(&text, &self.name);
                }
                let json_body: Value = serde_json::from_str(&text).map_err(|e| {
                    ProviderError::Transport(format!("Invalid JSON from provider: {e}"))
                })?;
                match effective_wire {
                    WireDialect::AnthropicMessages => parse_anthropic_response(&json_body),
                    WireDialect::GoogleGemini => parse_gemini_response(&json_body),
                    WireDialect::OpenAiResponses => parse_responses_json(&json_body, &self.name),
                    WireDialect::ChatCompletions => parse_openai_response(&json_body),
                }
            };

            let timeout_dur = Duration::from_millis(self.timeout_ms);
            if let Some(token) = cancel {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => Err(ProviderError::Cancelled),
                    res = tokio::time::timeout(timeout_dur, http_fut) => res.unwrap_or(Err(ProviderError::Timeout)),
                }
            } else {
                tokio::time::timeout(timeout_dur, http_fut)
                    .await
                    .unwrap_or(Err(ProviderError::Timeout))
            }
        })
    }
}

