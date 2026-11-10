// [LAYER: CORE]
pub mod benchmark_runner;
pub mod computer_use_orchestrator;
pub mod context_inspector;
pub mod engine;
pub mod flight_runner;
pub mod request_queue;
pub mod session_store;
pub mod slash_and_swarm;
pub mod spatial_engine;
#[cfg(test)]
pub mod computer_use_tests;
#[cfg(test)]
pub mod spatial_tests;
pub mod substrate_tests;
#[cfg(test)]
pub mod zenith_tests;
pub mod tool_governor;
pub mod tool_registry;
pub mod tool_specs;

pub use crate::core::benchmark_runner::{run_full_benchmark_suite, run_stress_benchmarks, run_workspace_benchmarks, LatencyMetrics, WorkspaceTaskBenchmarkResult};
pub use crate::core::computer_use_orchestrator::ComputerUseOrchestrator;
pub use crate::core::context_inspector::ContextInspectionSnapshot;
pub use crate::core::engine::{compute_workspace_revision, AgentEngine};
pub use crate::core::flight_runner::{format_flight_test_report, run_agent_flight_test, run_micro_benchmarks, FlightTestCheck, FlightTestReport, FlightTestScenarioResult, FlightTestSummary};
pub use crate::core::request_queue::{PromptComposer, QueuePermit, QueueState, RuntimeRequestQueue};
pub use crate::core::session_store::{SessionCompactor, SessionStore};
pub use crate::core::slash_and_swarm::SwarmSlashOrchestrator;
pub use crate::core::spatial_engine::SpatialInteractionEngine;
pub use crate::core::tool_governor::{ToolExecutionCache, ToolLoopBreaker, ToolOutputGovernor};
pub use crate::core::tool_registry::ToolRegistry;
pub use crate::core::tool_specs::builtin_tool_specs;
