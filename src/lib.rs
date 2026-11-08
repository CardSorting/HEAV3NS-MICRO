// [LAYER: UI]
pub mod core;
pub mod domain;
pub mod infrastructure;
pub mod plumbing;
pub mod ui;

pub use crate::domain::interfaces::ProgressCallback;
pub use crate::domain::types::messages::{Role, SessionMessage, ToolCall, ToolCallFunction};
pub use crate::domain::types::progress::{EngineProgressEvent, EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus, ToolExecutionRecord};
pub use crate::domain::types::runtime::{AgentConfig, CancellationToken, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel};
pub use crate::domain::types::tick_result::EngineTickResult;
pub use crate::plumbing::hash_and_math::now_ms;

/// Backward-compatible public module facade for [`AgentEngine`] and swarm orchestration.
pub mod engine {
    pub use crate::core::engine::AgentEngine;
    pub use crate::core::request_queue::{PromptComposer, QueuePermit, QueueState, RuntimeRequestQueue};
    pub use crate::domain::swarm::{SwarmAssignment, SwarmFormationResult, SwarmItemReport};
}

/// Backward-compatible public module facade for the Agent Flight Test suite and micro-benchmarks.
pub mod flight {
    pub use crate::core::flight_runner::{format_flight_test_report, run_agent_flight_test, run_micro_benchmarks, FlightTestCheck, FlightTestReport, FlightTestScenarioResult, FlightTestSummary};
}

/// Backward-compatible public module facade for self-healing tool argument parsing and secret redaction.
pub mod parser {
    pub use crate::domain::tool_parser::ToolCallArgParser;
    pub use crate::domain::types::messages::extract_text_tool_calls;
    pub use crate::domain::types::tool_types::{ParamSchema, ParamType, SchemaValidationResult};
    pub use crate::plumbing::hash_and_math::levenshtein;
    pub use crate::plumbing::json_repair::{accumulate_streaming_tool_name, compute_streaming_arguments_delta, extract_json_roots, normalize_completed_tool_call_arguments, repair_json_string};
    pub use crate::plumbing::secret_redactor::DeterministicSecretRedactor;
    pub use crate::plumbing::text_tokens::canonical_tool_name;
}

/// Backward-compatible public module facade for LLM completion providers and wire protocols.
pub mod provider {
    pub use crate::domain::interfaces::{BoxFuture, ILlmProvider, LlmProvider};
    pub use crate::domain::types::tick_result::{finalize_provider_response, is_retryable_http_status, ProviderError, ProviderRequest, ProviderResponse};
    pub use crate::infrastructure::http_provider::{parse_sse_stream, HttpProvider};
    pub use crate::infrastructure::scripted_provider::{ProviderStep, ScriptedProvider};
}

/// Backward-compatible public module facade for Arena slab allocation, VFS, memory, and compaction.
pub mod session {
    pub use crate::core::session_store::{SessionCompactor, SessionStore};
    pub use crate::domain::arena::{ArenaAllocator, SlabBufferSnapshot, DEFAULT_ARENA_CAPACITY_BYTES};
    pub use crate::domain::compaction::{CompactionPolicy, CompactionReport, ContextBudgetInfo, CONTEXT_CHECKPOINT_PREFIX};
    pub use crate::domain::memory::{GameStateSnapshot, MemoryEntry, MemoryStore, MEMORY_ENVELOPE_PREFIX};
    pub use crate::domain::types::messages::estimate_messages_tokens;
    pub use crate::domain::types::skill_types::SkillManifest;
    pub use crate::domain::types::vfs_types::{VfsDiskBaseline, VfsFileOverlay};
    pub use crate::infrastructure::vfs_store::{workspace_revision, FileSessionPersistence, SessionVfs, SkillsIngestor};
    pub use crate::plumbing::hash_and_math::fnv1a_32;
    pub use crate::plumbing::text_tokens::{estimate_text_tokens, render_unified_diff, truncate_text_to_token_budget};
}

/// Backward-compatible public module facade for the 23 built-in tools, wave scheduler, and patch engine.
pub mod tools {
    pub use crate::core::tool_governor::{ToolExecutionCache, ToolLoopBreaker, ToolOutputGovernor};
    pub use crate::core::tool_registry::ToolRegistry;
    pub use crate::domain::types::skill_types::GovernedOutput;
    pub use crate::domain::types::tool_schedule::{MutationTransaction, RollbackResult, ScheduledToolCall};
    pub use crate::domain::types::tool_types::ToolSpec;
    pub use crate::infrastructure::fs_tools::dispatch_fs_tool;
    pub use crate::infrastructure::search_tools::{execute_grep_search, extract_workspace_symbols, resolve_workspace_path, suggest_nearby_files};
    pub use crate::infrastructure::shell_runner::execute_shell_command;
    pub use crate::infrastructure::transaction_journal::TransactionJournal;
    pub use crate::plumbing::fuzzy_patch::{heal_fuzzy_patch, DeterministicPatchEngine, PatchHunkLine, PatchOperation};
    pub use crate::plumbing::secret_redactor::SafetyPolicy;
}
