// [LAYER: CORE]
#[cfg(test)]
mod tests {
    use crate::core::engine::AgentEngine;
    use crate::domain::interfaces::ICiGateRunner;
    use crate::domain::profiles::{
        AgentProfile, CiGatePolicy, CiGateReceipt, DEFAULT_AGENT_PROFILE_ID,
        DEFAULT_JOY_METHODOLOGY_PATH,
    };
    use crate::domain::prompt_spec::{SystemPromptSpec, WorkspacePromptFacts};
    use crate::domain::types::runtime::{
        AgentConfig, EngineTickInput, EngineTickOutcome, ExecutionAuthorityLevel, ExecutionPosture,
    };
    use crate::infrastructure::joy_ci_gate::JoyCiGateRunner;
    use crate::infrastructure::scripted_provider::{ProviderStep, ScriptedProvider};
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!(
                "heav3ns-profile-ci-{}-{}-{}",
                prefix,
                std::process::id(),
                nanos
            ));
            let _ = fs::create_dir_all(&path);
            Self { path }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct SequenceCiGateRunner {
        calls: AtomicUsize,
        fail_first: bool,
    }

    impl ICiGateRunner for SequenceCiGateRunner {
        fn run_ci_gate(&self, _workspace_root: &Path, policy: &CiGatePolicy) -> CiGateReceipt {
            let idx = self.calls.fetch_add(1, Ordering::SeqCst);
            let passed = !(self.fail_first && idx == 0);
            CiGateReceipt {
                passed,
                gate: "heav3ns-joy".to_string(),
                theme: policy.theme.clone(),
                phase: policy.phase.clone(),
                exit_code: if passed { 0 } else { 1 },
                duration_ms: 5,
                blocking_count: if passed { 0 } else { 1 },
                advisory_count: 0,
                summary: if passed {
                    "[spider:pass] files=1 errors=0 warnings=0 stability=100%".to_string()
                } else {
                    "[spider:fail] src/domain/bad.rs:1:1: error SPI-206 Missing [LAYER: DOMAIN] header"
                        .to_string()
                },
            }
        }
    }

    #[test]
    fn test_agent_profile_resolution_and_prompt_methodology_injection() {
        let def = AgentProfile::default_profile();
        assert_eq!(def.id, DEFAULT_AGENT_PROFILE_ID);
        assert_eq!(def.methodology, "heav3ns-joy");
        assert_eq!(def.methodology_path, DEFAULT_JOY_METHODOLOGY_PATH);
        assert!(def.ci_gate.enabled);
        assert!(def.ci_gate.trigger_on_dev_tasks);
        assert!(def.ci_gate.fail_closed);

        assert_eq!(
            AgentProfile::resolve("heav3ns-joy").map(|p| p.id),
            Some("joyzoning-architect".to_string())
        );
        assert_eq!(
            AgentProfile::resolve("strict").map(|p| p.id),
            Some("strict-ci".to_string())
        );
        assert_eq!(
            AgentProfile::resolve("research-readonly").and_then(|p| p.default_posture),
            Some(ExecutionPosture::Plan)
        );
        assert!(AgentProfile::resolve("nonexistent-profile").is_none());

        let cfg = AgentConfig::default();
        let spec = SystemPromptSpec::for_turn_with_profile(
            &cfg,
            ExecutionAuthorityLevel::Autonomous,
            None,
            Some(def),
        );
        let facts = WorkspacePromptFacts {
            joyzoning_detected: true,
            joy_methodology_path: Some(DEFAULT_JOY_METHODOLOGY_PATH.to_string()),
            ..Default::default()
        };
        let rendered = spec.render(&cfg, Path::new("/tmp/ws"), &facts, &[]);
        assert!(rendered.contains("HEAV3NS-JOY"));
        assert!(rendered.contains(DEFAULT_JOY_METHODOLOGY_PATH));
        assert!(rendered.contains("Triad Audit"));
        assert!(rendered.contains("Producer-Before-Consumer"));
        assert!(rendered.contains("CI Gate (Dev Tasks)"));
    }

    #[test]
    fn test_engine_profile_switching_and_slash_commands() {
        let ws = TempDirGuard::new("slash");
        let provider = ScriptedProvider::new(vec![]);
        let engine = AgentEngine::new(&ws.path, AgentConfig::default(), provider);

        assert_eq!(engine.active_profile().id, "joyzoning-architect");
        let snap = engine.inspect_context_snapshot("");
        assert_eq!(snap.agent_profile, "joyzoning-architect");
        assert_eq!(snap.methodology, "heav3ns-joy");
        assert!(snap.ci_gate_enabled);

        let switch_out = engine
            .handle_slash_command("/profile research-readonly")
            .expect("slash output");
        assert!(switch_out.contains("research-readonly"));
        assert_eq!(engine.active_profile().id, "research-readonly");
        assert_eq!(engine.current_posture(), ExecutionPosture::Plan);

        assert!(engine.set_profile("strict-ci").is_ok());
        assert_eq!(engine.current_posture(), ExecutionPosture::Ask);
        assert!(engine.set_profile("missing").is_err());
    }

    #[tokio::test]
    async fn test_ci_gate_fail_closed_remediates_dev_task_and_records_receipt() {
        let ws = TempDirGuard::new("ci-gate-loop");
        let gate_runner = Arc::new(SequenceCiGateRunner {
            calls: AtomicUsize::new(0),
            fail_first: true,
        });
        let provider = ScriptedProvider::new(vec![
            ProviderStep::tool_call(
                "tc_1",
                "write_file",
                json!({
                    "path": "src/domain/sample.rs",
                    "content": "pub struct Sample;\n"
                }),
                Some("Creating domain module".into()),
            ),
            ProviderStep::text("Done creating sample."),
            ProviderStep::tool_call(
                "tc_2",
                "write_file",
                json!({
                    "path": "src/domain/sample.rs",
                    "content": "// [LAYER: DOMAIN]\npub struct Sample;\n"
                }),
                Some("Adding missing layer header.".into()),
            ),
            ProviderStep::text("Verified with HEAV3NS-JOY CI gate."),
        ]);

        let engine = AgentEngine::new(&ws.path, AgentConfig::default(), provider)
            .with_ci_gate_runner(gate_runner.clone());
        let res = engine.tick(EngineTickInput::new("Add sample domain module")).await;

        assert_eq!(res.outcome, EngineTickOutcome::Completed);
        assert_eq!(res.response, "Verified with HEAV3NS-JOY CI gate.");
        assert_eq!(gate_runner.calls.load(Ordering::SeqCst), 2);
        let receipt = res.receipt.expect("turn receipt");
        let ci = receipt.ci_gate.expect("ci_gate receipt");
        assert!(ci.passed);
        assert_eq!(ci.gate, "heav3ns-joy");
        assert_eq!(ci.blocking_count, 0);
    }

    #[tokio::test]
    async fn test_ci_gate_skipped_for_readonly_turn_without_mutations() {
        let ws = TempDirGuard::new("ci-gate-skip");
        let gate_runner = Arc::new(SequenceCiGateRunner {
            calls: AtomicUsize::new(0),
            fail_first: false,
        });
        let provider = ScriptedProvider::new(vec![ProviderStep::text("Read-only answer.")]);

        let engine = AgentEngine::new(&ws.path, AgentConfig::default(), provider)
            .with_ci_gate_runner(gate_runner.clone());
        let res = engine.tick(EngineTickInput::new("Explain architecture")).await;
        assert_eq!(res.outcome, EngineTickOutcome::Completed);
        assert_eq!(gate_runner.calls.load(Ordering::SeqCst), 0);
        assert!(res.receipt.and_then(|r| r.ci_gate).is_none());
    }

    #[test]
    fn test_live_joy_ci_gate_runner_detects_missing_layer_and_passes_valid_zone() {
        if !Path::new(crate::domain::profiles::DEFAULT_JOY_CLI_PATH).is_file() {
            return;
        }
        let ws = TempDirGuard::new("live-joy");
        let domain_dir = ws.path.join("src/domain");
        fs::create_dir_all(&domain_dir).expect("mkdir");
        let target = domain_dir.join("entity.rs");

        // Missing [LAYER: DOMAIN] -> should fail strict CI gate
        fs::write(&target, "pub struct Entity;\n").expect("write invalid");
        let runner = JoyCiGateRunner;
        let fail_receipt = runner.run_ci_gate(&ws.path, &CiGatePolicy::default());
        assert!(!fail_receipt.passed);
        assert!(fail_receipt.blocking_count >= 1);

        // Add [LAYER: DOMAIN] -> should pass strict CI gate
        fs::write(&target, "// [LAYER: DOMAIN]\npub struct Entity;\n").expect("write valid");
        let pass_receipt = runner.run_ci_gate(&ws.path, &CiGatePolicy::default());
        assert!(pass_receipt.passed, "expected pass: {}", pass_receipt.summary);
        assert_eq!(pass_receipt.blocking_count, 0);
    }

    #[test]
    fn test_ci_gate_rejects_untrusted_cli_path() {
        let ws = TempDirGuard::new("untrusted-cli");
        let runner = JoyCiGateRunner;
        let policy = CiGatePolicy {
            joy_cli_path: "/tmp/malicious-joy-check.mjs".to_string(),
            ..Default::default()
        };
        let receipt = runner.run_ci_gate(&ws.path, &policy);
        assert!(!receipt.passed);
        assert_eq!(receipt.exit_code, 2);
        assert!(receipt.summary.contains("UNTRUSTED_CI_GATE_PATH"));
    }

    #[tokio::test]
    async fn test_ci_gate_fail_closed_exhausted_rounds_marks_turn_failed() {
        let ws = TempDirGuard::new("ci-gate-exhausted");
        let gate_runner = Arc::new(SequenceCiGateRunner {
            calls: AtomicUsize::new(0),
            fail_first: true,
        });
        let mut cfg = AgentConfig::default();
        cfg.max_tool_rounds = Some(2);
        let provider = ScriptedProvider::new(vec![
            ProviderStep::tool_call(
                "tc_1",
                "write_file",
                json!({ "path": "src/domain/bad.rs", "content": "pub struct Bad;\n" }),
                None,
            ),
            ProviderStep::text("Trying to finish without fixing."),
        ]);
        let engine = AgentEngine::new(&ws.path, cfg, provider)
            .with_ci_gate_runner(gate_runner.clone());
        let res = engine.tick(EngineTickInput::new("Create domain file")).await;

        assert_eq!(res.outcome, EngineTickOutcome::Failed);
        assert!(res.response.contains("HEAV3NS-JOY CI GATE FAILED"));
        let ci = res.receipt.and_then(|r| r.ci_gate).expect("ci_gate receipt");
        assert!(!ci.passed);
        assert_eq!(ci.blocking_count, 1);
    }

    #[tokio::test]
    async fn test_fast_patch_advisory_ci_gate_allows_completion_with_failed_receipt() {
        let ws = TempDirGuard::new("ci-gate-advisory");
        let gate_runner = Arc::new(SequenceCiGateRunner {
            calls: AtomicUsize::new(0),
            fail_first: true,
        });
        let provider = ScriptedProvider::new(vec![
            ProviderStep::tool_call(
                "tc_1",
                "write_file",
                json!({ "path": "src/domain/advisory.rs", "content": "pub struct Adv;\n" }),
                None,
            ),
            ProviderStep::text("Patched quickly."),
        ]);
        let engine = AgentEngine::new(&ws.path, AgentConfig::default(), provider)
            .with_ci_gate_runner(gate_runner.clone());
        engine.set_profile("fast-patch").expect("set fast-patch");
        let res = engine.tick(EngineTickInput::new("Quick patch")).await;

        assert_eq!(res.outcome, EngineTickOutcome::Completed);
        assert_eq!(res.response, "Patched quickly.");
        let ci = res.receipt.and_then(|r| r.ci_gate).expect("ci_gate receipt");
        assert!(!ci.passed);
    }

    #[tokio::test]
    async fn test_swarm_inherits_parent_profile_and_ci_gate_runner() {
        use crate::domain::swarm::SwarmAssignment;
        let ws = TempDirGuard::new("swarm-profile");
        let gate_runner = Arc::new(SequenceCiGateRunner {
            calls: AtomicUsize::new(0),
            fail_first: false,
        });
        let provider = ScriptedProvider::new(vec![
            ProviderStep::tool_call(
                "tc_s1",
                "write_file",
                json!({ "path": "src/domain/s.rs", "content": "// [LAYER: DOMAIN]\npub struct S;\n" }),
                None,
            ),
            ProviderStep::text("Specialist finished."),
        ]);
        let engine = AgentEngine::new(&ws.path, AgentConfig::default(), provider)
            .with_ci_gate_runner(gate_runner.clone());
        engine.set_profile("strict-ci").expect("set strict-ci");
        engine.set_posture(ExecutionPosture::AcceptEdits);

        let formation = engine
            .run_swarm(
                "Create domain S",
                vec![SwarmAssignment {
                    id: "spec-1".into(),
                    role: "Domain Author".into(),
                    objective: "Create src/domain/s.rs".into(),
                    depends_on: vec![],
                    assigned_files: vec!["src/domain/s.rs".into()],
                }],
                None,
                None,
            )
            .await;

        assert_eq!(formation.outcome, EngineTickOutcome::Completed);
        assert_eq!(gate_runner.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_workspace_profiles_strict_schema_and_trust_boundary() {
        let ws = TempDirGuard::new("ws-profiles");
        let heav3ns_dir = ws.path.join(".heav3ns");
        fs::create_dir_all(&heav3ns_dir).expect("mkdir .heav3ns");

        // 1. Valid custom workspace profile loads and activates
        let valid_json = json!([
            {
                "id": "custom-joy-strict",
                "name": "Custom Joy Strict",
                "description": "Repo-pinned strict Joyzoning profile",
                "methodology": "heav3ns-joy",
                "methodologyPath": "",
                "styleDirectives": ["Enforce strict Rust domain purity."],
                "defaultPosture": "accept_edits",
                "ciGate": {
                    "enabled": true,
                    "triggerOnDevTasks": true,
                    "joyCliPath": "",
                    "theme": "strict",
                    "phase": "ci",
                    "failClosed": true,
                    "timeoutMs": 15000
                }
            }
        ])
        .to_string();
        fs::write(heav3ns_dir.join("profiles.json"), valid_json).expect("write profiles.json");

        let engine = AgentEngine::new(&ws.path, AgentConfig::default(), ScriptedProvider::new(vec![]));
        let all = engine.list_profiles();
        assert!(all.iter().any(|p| p.id == "custom-joy-strict"));
        let switched = engine.set_profile("custom-joy-strict").expect("set custom");
        assert_eq!(switched.id, "custom-joy-strict");
        assert_eq!(
            switched.ci_gate.joy_cli_path,
            crate::domain::profiles::DEFAULT_JOY_CLI_PATH
        );

        // 2. Malicious joyCliPath in profiles.json is rejected by strict parser
        let malicious_raw = json!([
            {
                "id": "evil-profile",
                "name": "Evil",
                "description": "Untrusted path",
                "methodology": "heav3ns-joy",
                "methodologyPath": "/tmp/evil",
                "styleDirectives": [],
                "ciGate": {
                    "enabled": true,
                    "triggerOnDevTasks": true,
                    "joyCliPath": "/tmp/evil.mjs",
                    "theme": "strict",
                    "phase": "ci",
                    "failClosed": true
                }
            }
        ])
        .to_string();
        let err = AgentProfile::parse_workspace_profiles_strict(&malicious_raw)
            .expect_err("untrusted joyCliPath must be rejected");
        assert!(err.contains("untrusted joyCliPath"));
    }
}
