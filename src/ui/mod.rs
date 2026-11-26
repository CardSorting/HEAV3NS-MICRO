// [LAYER: UI]
pub mod cli_presenter;
pub mod files_rpc;
pub mod git_remote_rpc;
pub mod git_rpc;
pub mod onboarding;
pub mod onboarding_cli;
pub mod provider_rpc;
pub mod rpc_dispatch;
pub mod stdio_worker;
pub mod stdio_worker_extended;
pub mod worker_handlers;
pub mod worker_submit;

pub use crate::core::flight_runner::format_flight_test_report;
pub use crate::ui::cli_presenter::{
    parse_args, print_help, CliOptions, Heav3nsUserConfig, RunMode,
};
pub use crate::ui::onboarding::{
    create_cli_progress_listener, format_auth_error_card, format_turn_summary,
    print_onboarding_notice, print_welcome_banner, render_color_diff, suggest_command,
};
pub use crate::ui::onboarding_cli::{
    print_tool_catalog, run_doctor_cli, run_guided_demo, run_init_wizard,
};
pub use crate::ui::stdio_worker::run_stdio_worker;
pub use crate::ui::worker_handlers::WorkerSharedState;
