// [LAYER: DOMAIN]
use crate::domain::types::progress::{EngineProgressEvent, ToolExecutionRecord};
use crate::domain::types::runtime::{CancellationToken, ExecutionAuthorityLevel};
use crate::domain::types::skill_types::SkillManifest;
use crate::domain::types::tick_result::{ProviderError, ProviderRequest, ProviderResponse};
use crate::domain::types::tool_schedule::{RollbackResult, ScheduledToolCall};
use crate::domain::types::vfs_types::VfsFileOverlay;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type ProgressCallback = Arc<dyn Fn(EngineProgressEvent) + Send + Sync>;
pub type DeferralPredicate = Arc<dyn Fn() -> bool + Send + Sync>;

/// Contractual sovereignty marker for [`HttpProvider`](crate::infrastructure::http_provider::HttpProvider).
pub trait IHttpProvider: ILlmProvider {}

/// Contractual sovereignty marker for [`ScriptedProvider`](crate::infrastructure::scripted_provider::ScriptedProvider).
pub trait IScriptedProvider: ILlmProvider {
    fn request_count(&self) -> usize;
}

/// Contractual sovereignty interface for session JSONL persistence.
pub trait ISessionPersistence: Send + Sync {
    fn save_jsonl(&self, path: &Path, payload: &str) -> Result<bool, String>;
    fn load_jsonl(&self, path: &Path) -> Result<String, String>;
}

/// Contractual sovereignty marker for [`FileSessionPersistence`](crate::infrastructure::vfs_store::FileSessionPersistence).
pub trait IFileSessionPersistence: ISessionPersistence {}

/// Contractual sovereignty interface for staged Virtual File System overlays.
pub trait ISessionVfs: Send + Sync {
    fn export_staged_overlays(&self) -> Vec<VfsFileOverlay>;
    fn commit_staged(&mut self) -> Result<Vec<String>, String>;
}

/// Contractual sovereignty interface for workspace `SKILL.md` discovery.
pub trait ISkillsIngestor: Send + Sync {
    fn discover_manifests(workspace_root: &Path) -> Vec<SkillManifest>;
}

/// Contractual sovereignty interface for atomic file mutation journaling and rollback.
pub trait ITransactionJournal: Send + Sync {
    fn record_file_mutation(&mut self, tool_name: &str, target_path: &Path) -> String;
    fn rollback_last_mutation(&mut self) -> RollbackResult;
}

/// Contractual sovereignty interface for batch and single tool execution.
pub trait IToolExecutor: Send + Sync {
    fn execute_batch_contract<'a>(
        &'a self,
        calls: &'a [ScheduledToolCall],
        cwd: &'a Path,
        authority: ExecutionAuthorityLevel,
        cancel_token: Option<&'a CancellationToken>,
    ) -> BoxFuture<'a, Vec<ToolExecutionRecord>>;
}

/// Contractual sovereignty interface for LLM completion providers (`I<Name>` convention).
pub trait ILlmProvider: Send + Sync {
    fn provider_name(&self) -> &str;
    fn complete<'a>(
        &'a self,
        req: ProviderRequest,
        cancel: Option<CancellationToken>,
    ) -> BoxFuture<'a, Result<ProviderResponse, ProviderError>>;
}

pub type StaticProviderId = &'static str;
pub use ILlmProvider as LlmProvider;
