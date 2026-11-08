// [LAYER: PLUMBING]
pub mod fuzzy_patch;
pub mod hash_and_math;
pub mod json_repair;
pub mod secret_redactor;
pub mod text_tokens;

pub use crate::plumbing::fuzzy_patch::{heal_fuzzy_patch, DeterministicPatchEngine, PatchHunkLine, PatchOperation};
pub use crate::plumbing::hash_and_math::{fnv1a_32, levenshtein, now_ms};
pub use crate::plumbing::json_repair::{accumulate_streaming_tool_name, compute_streaming_arguments_delta, extract_json_roots, normalize_completed_tool_call_arguments, repair_json_string};
pub use crate::plumbing::secret_redactor::{DeterministicSecretRedactor, SafetyPolicy};
pub use crate::plumbing::text_tokens::{canonical_tool_name, estimate_text_tokens, render_unified_diff, truncate_text_to_token_budget};
