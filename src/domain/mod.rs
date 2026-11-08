// [LAYER: DOMAIN]
pub mod arena;
pub mod compaction;
pub mod interfaces;
pub mod memory;
pub mod swarm;
pub mod tool_parser;
pub mod types;

pub use crate::domain::arena::{ArenaAllocator, SlabBufferSnapshot, DEFAULT_ARENA_CAPACITY_BYTES};
pub use crate::domain::compaction::{CompactionPolicy, CompactionReport, ContextBudgetInfo, CONTEXT_CHECKPOINT_PREFIX};
pub use crate::domain::interfaces::{BoxFuture, DeferralPredicate, ILlmProvider, ISessionPersistence, IToolExecutor, LlmProvider, ProgressCallback};
pub use crate::domain::memory::{GameStateSnapshot, MemoryEntry, MemoryStore, MEMORY_ENVELOPE_PREFIX};
pub use crate::domain::swarm::{SwarmAssignment, SwarmFormationResult, SwarmItemReport};
pub use crate::domain::tool_parser::ToolCallArgParser;
pub use crate::domain::types::{estimate_messages_tokens, extract_text_tool_calls, finalize_provider_response, is_retryable_http_status, AgentConfig, CancellationToken, EngineProgressEvent, EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus, EngineTickInput, EngineTickOutcome, EngineTickResult, ExecutionAuthorityLevel, GovernedOutput, MutationTransaction, ParamSchema, ParamType, ProviderError, ProviderRequest, ProviderResponse, Role, RollbackResult, ScheduledToolCall, SchemaValidationResult, SessionMessage, SkillManifest, ToolCall, ToolCallFunction, ToolExecutionRecord, ToolSpec, VfsDiskBaseline, VfsFileOverlay};

