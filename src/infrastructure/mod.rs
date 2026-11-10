// [LAYER: INFRASTRUCTURE]
pub mod behavioral_store;
pub mod fs_tools;
pub mod http_provider;
pub mod pricing_updater;
pub mod scripted_provider;
pub mod search_tools;
pub mod shell_runner;
pub mod transaction_journal;
pub mod vfs_store;
pub mod workspace_cas;

pub use crate::infrastructure::behavioral_store::BehavioralKnowledgeStore;
pub use crate::infrastructure::fs_tools::dispatch_fs_tool;
pub use crate::infrastructure::http_provider::{parse_sse_stream, probe_local_ollama_model, HttpProvider};
pub use crate::infrastructure::pricing_updater::{global_pricing_registry, ModelPricingRegistry, DEFAULT_CATALOG_TTL_MS, DEFAULT_OPENROUTER_MODELS_URL};
pub use crate::infrastructure::scripted_provider::{ProviderStep, ScriptedProvider};
pub use crate::infrastructure::search_tools::{atomic_write_file, batch_replace_regex_files, build_directory_tree, collect_source_files, execute_grep_search, extract_workspace_symbols, is_ignored_dir, resolve_workspace_path, suggest_nearby_files, verify_model_path_access, walk_find_files};
pub use crate::infrastructure::shell_runner::{execute_shell_command, execute_shell_command_sandboxed};
pub use crate::infrastructure::transaction_journal::TransactionJournal;
pub use crate::infrastructure::vfs_store::{workspace_revision, FileSessionPersistence, SessionVfs, SkillsIngestor, WorkspacePromptProbe};
pub use crate::infrastructure::workspace_cas::{CasFileEntry, CasSnapshotManifest, WorkspaceSnapshotCas};
