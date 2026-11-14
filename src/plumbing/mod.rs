// [LAYER: PLUMBING]
pub mod display_telemetry;
pub mod environmental_diff;
pub mod fuzzy_patch;
pub mod hash_and_math;
pub mod json_repair;
pub mod oauth_pkce;
pub mod schema_and_sse;
pub mod seatbelt_profile;
pub mod secret_redactor;
pub mod shell_guard;
pub mod spatial_geometry;
pub mod text_tokens;
pub mod vision_and_dom;

pub use crate::plumbing::display_telemetry::{apply_annotation_mutation, compute_action_ledger_health, compute_preview_delta, extract_dom_ui_tuples, query_action_ledger_dsl};
pub use crate::plumbing::environmental_diff::{analyze_spider_structural_relationships, calibrate_confidence_score, classify_element_affordance, compute_element_deltas, detect_modal_or_dialog, diagnose_failure_cause};
pub use crate::plumbing::fuzzy_patch::{heal_fuzzy_patch, DeterministicPatchEngine, PatchHunkLine, PatchOperation};
pub use crate::plumbing::hash_and_math::{fnv1a_32, levenshtein, now_ms};
pub use crate::plumbing::json_repair::{accumulate_streaming_tool_name, compute_streaming_arguments_delta, extract_json_roots, normalize_completed_tool_call_arguments, repair_json_string};
pub use crate::plumbing::oauth_pkce::{build_openai_authorize_url, decode_base64url, encode_base64url, encode_hex_lower, extract_jwt_expiry_ms, extract_openai_account_id, extract_openai_email, extract_openai_plan_type, generate_pkce_challenge, parse_jwt_claims, parse_query_params, url_decode_component, url_encode_component};
pub use crate::plumbing::schema_and_sse::{parse_sse_events, redact_endpoint_for_display, sanitize_tool_schema, sanitize_tool_schema_for_responses, SseEventFrame, MAX_SSE_EVENT_DATA_BYTES};
pub use crate::plumbing::seatbelt_profile::{build_macos_seatbelt_invocation, inspect_command_security, CommandSecurityVerdict, SeatbeltInvocation, SeatbeltSandboxMode};
pub use crate::plumbing::secret_redactor::{CredentialDenyScope, DeterministicSecretRedactor, SafetyPolicy};
pub use crate::plumbing::shell_guard::{split_top_level_statements, ShellCommandGuard};
pub use crate::plumbing::spatial_geometry::{calculate_rectangles_union_area, normalize_coordinate_in_space, normalize_coordinate_value, validate_trajectory, SpatialPoint, SpatialRect, ViewportTransform};
pub use crate::plumbing::text_tokens::{canonical_tool_name, count_diff_line_stats, encode_base64, estimate_text_tokens, mask_secret_preview, render_unified_diff, safe_utf8_prefix, safe_utf8_slice, truncate_text_to_token_budget};
pub use crate::plumbing::vision_and_dom::{calculate_aspect_ratio, decode_image_metadata, generate_synthetic_bmp, generate_synthetic_svg, redact_cdp_url, sanitize_trojan_unicode, snapshot_html_dom, validate_navigation_url};
