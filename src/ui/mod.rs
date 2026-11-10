// [LAYER: UI]
pub mod cli_presenter;
pub mod onboarding;
pub mod rpc_dispatch;
pub mod stdio_worker;

pub use crate::core::flight_runner::format_flight_test_report;
pub use crate::ui::cli_presenter::{parse_args, print_help, CliOptions, Heav3nsUserConfig, RunMode};
pub use crate::ui::onboarding::{create_cli_progress_listener, format_auth_error_card, format_turn_summary, print_onboarding_notice, print_tool_catalog, print_welcome_banner, render_color_diff, run_doctor_cli, run_guided_demo, run_init_wizard, suggest_command};
pub use crate::ui::stdio_worker::run_stdio_worker;
