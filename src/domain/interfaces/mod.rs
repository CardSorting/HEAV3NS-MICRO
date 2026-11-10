// [LAYER: DOMAIN]
use crate::domain::turn_receipt::FileChangeReceipt;
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
pub type MemoryPersistenceCallback = Arc<dyn Fn(&str, &str, &str) + Send + Sync>;

/// Contractual sovereignty marker for HTTP-backed LLM completion providers.
pub trait IHttpProvider: ILlmProvider {}

/// Contractual sovereignty marker for deterministic scripted LLM providers.
pub trait IScriptedProvider: ILlmProvider {
    fn request_count(&self) -> usize;
}

/// Contractual sovereignty interface for session JSONL persistence.
pub trait ISessionPersistence: Send + Sync {
    fn save_jsonl(&self, path: &Path, payload: &str) -> Result<bool, String>;
    fn load_jsonl(&self, path: &Path) -> Result<String, String>;
}

/// Contractual sovereignty marker for file-backed session JSONL persistence.
pub trait IFileSessionPersistence: ISessionPersistence {}

/// Contractual sovereignty interface for staged Virtual File System overlays.
pub trait ISessionVfs: Send + Sync {
    fn export_staged_overlays(&self) -> Vec<VfsFileOverlay>;
    fn commit_staged(&mut self) -> Result<Vec<String>, String>;
}

/// Contractual sovereignty interface for pure-Rust Content-Addressable Workspace Snapshots (`CAS`).
pub trait IWorkspaceSnapshotCas: Send + Sync {
    fn capture_snapshot(&mut self, label: &str, turn_number: u64) -> Result<String, String>;
    fn diff_between(&self, base_id: &str, target_id: Option<&str>) -> Result<(Vec<FileChangeReceipt>, String), String>;
    fn undo_turn_changes(&mut self, pre_turn_id: &str, post_turn_id: &str, force: bool) -> Result<Vec<String>, String>;
    fn restore_full_snapshot(&mut self, snapshot_id: &str) -> Result<Vec<String>, String>;
}

/// Contractual sovereignty interface for workspace `SKILL.md` discovery.
pub trait ISkillsIngestor: Send + Sync {
    fn discover_manifests(workspace_root: &Path) -> Vec<SkillManifest>;
}

/// Contractual sovereignty interface for cached workspace environment and governance probing.
pub trait IWorkspacePromptProbe: Send + Sync {
    fn probe_workspace(&self, workspace_root: &Path) -> crate::domain::types::skill_types::WorkspacePromptFacts;
}

/// Contractual sovereignty interface for atomic file mutation journaling and rollback.
pub trait ITransactionJournal: Send + Sync {
    fn record_file_mutation(&mut self, tool_name: &str, target_path: &Path) -> String;
    fn rollback_last_mutation(&mut self) -> RollbackResult;
}

/// Contractual sovereignty interface for spatial action mediation and target grounding.
pub trait ISpatialMediator: Send + Sync {
    fn ground_contract(&self, contract: &crate::domain::types::spatial_types::SpatialActionContract) -> crate::domain::spatial_action::SpatialGroundingResult;
}

/// Contractual sovereignty interface for Wrapper A (Spatial Grounding Gate).
pub trait ISpatialGroundingGate: Send + Sync {
    fn ground(&self, contract: &crate::domain::types::spatial_types::SpatialActionContract) -> crate::domain::spatial_action::SpatialGroundingResult;
}

/// Contractual sovereignty interface for Wrapper B (Execution Assurance Gate).
pub trait IExecutionAssuranceGate: Send + Sync {
    fn assure_pre(&self, grounded: &crate::domain::spatial_action::GroundedSpatialTarget, contract: &crate::domain::types::spatial_types::SpatialActionContract) -> Result<(), crate::domain::types::spatial_types::SpatialError>;
    fn verify_post(&mut self, action: &str, grounded: Option<&crate::domain::spatial_action::GroundedSpatialTarget>, before: &crate::domain::types::spatial_types::SpatialScene, after: &crate::domain::types::spatial_types::SpatialScene, duration_ms: u64) -> crate::domain::spatial_action::SpatialExecutionReceipt;
}

/// Contractual sovereignty interface for environmental observation and modal trap detection.
pub trait IEnvironmentalObserver: Send + Sync {
    fn observe_environment(&self) -> crate::domain::types::environmental_types::EnvironmentalObservation;
    fn detect_modal_trap(&self, obs: &crate::domain::types::environmental_types::EnvironmentalObservation) -> Option<String>;
}

/// Contractual sovereignty interface for durable behavioral knowledge, interaction patterns, and environmental reconciliation.
pub trait IBehavioralKnowledgeStore: Send + Sync {
    fn record_outcome(&mut self, pattern_id: &str, app: &str, success: bool, epoch: u64);
    fn lookup_pattern(&self, app: &str, trigger: &str) -> Option<crate::domain::types::learned_knowledge_types::LearnedInteractionPattern>;
    fn reconcile_state(&self, recorded_epoch: u64, current_epoch: u64, hash_drift: bool) -> crate::domain::types::learned_knowledge_types::EnvironmentalReconciliationReport;
}

/// Contractual sovereignty marker for file-backed behavioral knowledge stores.
pub trait IBehavioralStore: IBehavioralKnowledgeStore {}

/// Contractual sovereignty interface for consulting the Synthetic Swarm on environmental interpretation and recovery.
pub trait ISwarmIntelligenceConsultant: Send + Sync {
    fn consult(&self, req: &crate::domain::types::swarm_intelligence_types::SwarmConsultationRequest) -> crate::domain::types::swarm_intelligence_types::SwarmConsultationResponse;
}

/// Contractual sovereignty interface for structural environmental element diffing.
pub trait IEnvironmentalDiffer: Send + Sync {
    fn compute_deltas(&self, before: &[serde_json::Value], after: &[serde_json::Value]) -> serde_json::Value;
}

/// Contractual sovereignty interface for spatial geometry operations and union calculation.
pub trait ISpatialGeometry: Send + Sync {
    fn compute_union_area(&self, rects: &[crate::plumbing::spatial_geometry::SpatialRect]) -> f64;
}

/// Contractual sovereignty interface for closed-loop Computer Use orchestration.
pub trait IComputerUseOrchestrator: Send + Sync {
    fn execute_closed_loop(&self, goal: &str, contract: &crate::domain::types::spatial_types::SpatialActionContract) -> Result<serde_json::Value, String>;
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
