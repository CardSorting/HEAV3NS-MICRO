// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{BoxFuture, ILlmProvider};
use crate::domain::types::messages::{Role, SessionMessage, ToolCall, ToolCallFunction};
use crate::domain::types::runtime::CancellationToken;
use crate::domain::types::tick_result::{finalize_provider_response, is_retryable_http_status, ProviderError, ProviderRequest, ProviderResponse};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Debug, Clone)]
pub enum ProviderStep {
    Completion {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    HttpError {
        status: u16,
        message: String,
    },
    WaitForAbort,
}

impl ProviderStep {
    pub fn text(content: impl Into<String>) -> Self {
        Self::Completion {
            content: Some(content.into()),
            tool_calls: Vec::new(),
        }
    }

    pub fn tool_call(
        call_id: impl Into<String>,
        name: impl Into<String>,
        args: Value,
        content: Option<String>,
    ) -> Self {
        Self::Completion {
            content,
            tool_calls: vec![ToolCall {
                id: call_id.into(),
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: name.into(),
                    arguments: args.to_string(),
                },
            }],
        }
    }

    pub fn http_error(status: u16, message: impl Into<String>) -> Self {
        Self::HttpError {
            status,
            message: message.into(),
        }
    }
}

/// Deterministic scripted provider for Flight Tests (`FT-01`..`FT-05`) and harness evaluations.
pub struct ScriptedProvider {
    steps: Mutex<VecDeque<ProviderStep>>,
    recorded_requests: Mutex<Vec<ProviderRequest>>,
    request_notify: Notify,
}

impl ScriptedProvider {
    pub extern "Rust" fn new(steps: Vec<ProviderStep>) -> Arc<Self> {
        Arc::new(Self {
            steps: Mutex::new(VecDeque::from(steps)),
            recorded_requests: Mutex::new(Vec::new()),
            request_notify: Notify::new(),
        })
    }

    pub fn push_step(&self, step: ProviderStep) {
        self.steps.lock().unwrap().push_back(step);
    }

    pub fn request_count(&self) -> usize {
        self.recorded_requests.lock().unwrap().len()
    }

    pub fn requests(&self) -> Vec<ProviderRequest> {
        self.recorded_requests.lock().unwrap().clone()
    }

    pub fn tool_reply_count_by_request(&self) -> Vec<usize> {
        self.recorded_requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.messages.iter().filter(|m| m.role == Role::Tool).count())
            .collect()
    }

    pub fn tool_messages_by_request(&self) -> Vec<Vec<SessionMessage>> {
        self.recorded_requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.messages.iter().filter(|m| m.role == Role::Tool).cloned().collect())
            .collect()
    }

    pub async fn wait_for_request_count(&self, target: usize, timeout_ms: u64) -> bool {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            let notified = self.request_notify.notified();
            if self.request_count() >= target {
                return true;
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return self.request_count() >= target;
            }
        }
    }
}

impl ILlmProvider for ScriptedProvider {
    fn provider_name(&self) -> &str {
        "scripted"
    }

    fn complete<'a>(
        &'a self,
        req: ProviderRequest,
        cancel: Option<CancellationToken>,
    ) -> BoxFuture<'a, Result<ProviderResponse, ProviderError>> {
        Box::pin(async move {
            self.recorded_requests.lock().unwrap().push(req);
            self.request_notify.notify_waiters();
            if cancel.as_ref().map(|t| t.is_cancelled()).unwrap_or(false) {
                return Err(ProviderError::Cancelled);
            }
            let step = self
                .steps
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| ProviderStep::text("Scripted provider queue exhausted."));
            match step {
                ProviderStep::Completion { content, tool_calls } => {
                    Ok(finalize_provider_response(content, tool_calls, "scripted"))
                }
                ProviderStep::HttpError { status, message } => Err(ProviderError::HttpStatus {
                    status,
                    body: message,
                    retryable: is_retryable_http_status(status),
                }),
                ProviderStep::WaitForAbort => {
                    if let Some(token) = cancel {
                        token.cancelled().await;
                        Err(ProviderError::Cancelled)
                    } else {
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        Err(ProviderError::Timeout)
                    }
                }
            }
        })
    }
}

