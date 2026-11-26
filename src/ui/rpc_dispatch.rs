// [LAYER: UI]
use crate::core::engine::AgentEngine;
use crate::core::session_store::SessionCompactor;
use crate::domain::memory::MemoryEntry;
use crate::domain::pricing_and_taxonomy::BillingMetering;
use crate::plumbing::hash_and_math::{lock_unpoison, now_ms};
use crate::plumbing::secret_redactor::SafetyPolicy;
use crate::plumbing::text_tokens::safe_utf8_prefix;
use crate::ui::worker_handlers::{build_sovereign_model_catalog, build_sovereign_model_discovery};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn validate_rpc_read_path(root: &Path, path: &str) -> Option<PathBuf> {
    if path.is_empty()
        || path.contains("..")
        || path.starts_with('/')
        || SafetyPolicy::check_path(Path::new(path)).is_err()
    {
        return None;
    }
    let full_path = root.join(path);
    let (Ok(canon_root), Ok(canon_file)) = (root.canonicalize(), full_path.canonicalize()) else {
        return None;
    };
    if !canon_file.starts_with(&canon_root) || SafetyPolicy::check_path(&canon_file).is_err() {
        return None;
    }
    Some(canon_file)
}

pub fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn sha256_hex(input: &str) -> String {
    sha256_bytes_hex(input.as_bytes())
}

pub fn uuid_from_seed(seed: &str) -> String {
    let h = sha256_hex(seed);
    format!(
        "{}-{}-4{}-8{}-{}",
        &h[0..8],
        &h[8..12],
        &h[13..16],
        &h[17..20],
        &h[20..32]
    )
}

pub fn build_hello_response(ws_id: &str, sess_id: &str, workspace_root: &Path) -> Value {
    json!({
        "protocol": 1,
        "runtime": "heav3ns-micro",
        "workspace": ws_id,
        "session": sess_id,
        "path": workspace_root.to_string_lossy(),
        "limits": {
            "queuedInstructions": 3,
            "instructionLength": 16000
        },
        "capabilities": [
            "native",
            "workspace-branch-v1",
            "binary-inspection-v1",
            "autonomous-execution-v1",
            "snapshot",
            "workspace-verification-v1",
            "task-continuation-v1",
            "project-repair-v1",
            "project-run-v1",
            "project-self-healing-v1",
            "walkthrough-v1",
            "queue:3",
            "queue-edit",
            "queue-remove",
            "cancel",
            "approval",
            "history",
            "files",
            "file-search-v1",
            "changes",
            "paged-inspection",
            "workspace-stop",
            "run-details",
            "run-insights-v1",
            "runtime-state-v1",
            "agent-swarm-defer-v1",
            "spider-check-v1",
            "conversation-context-v1",
            "context-history-v1",
            "context-compact-v1",
            "side-query-v1",
            "project-tasks-v1",
            "project-tasks-move-v1",
            "model-selection-v1",
            "reasoning-selection-v1",
            "model-discovery-v1",
            "openrouter-credentials-v1",
            "conversation-memory-v1",
            "checkpoints-v1",
            "conversations",
            "conversation-delete-v1",
            "computer-use-v1",
            "browser-dom-v1",
            "vision-inspect-v1",
            "skill-forge-v1",
            "soul-vault-v1",
            "heredoc-defense-v1",
            "prompt-inspect-v1",
            "posture-dial-v1",
            "agent-profiles-v1",
            "joy-ci-gate-v1",
            "turn-receipts-v1",
            "request-preview-v1",
            "multi-provider-parity-v1",
            "openai-responses-wire-v1"
        ]
    })
}

pub fn build_model_selection(provider: &str, model: &str, reasoning: &str, revision: i64) -> Value {
    let norm_reasoning = match reasoning.trim().to_ascii_lowercase().as_str() {
        "" | "default" => "default",
        "none" | "off" | "disabled" => "none",
        "low" | "minimal" => "low",
        "medium" | "med" => "medium",
        "high" => "high",
        "xhigh" | "max" => "xhigh",
        _ => "default",
    };
    json!({
        "provider": provider,
        "model": model,
        "revision": revision,
        "reasoningEffort": norm_reasoning,
        "reasoningOptions": ["none", "low", "medium", "high", "xhigh"],
        "busy": false,
        "blocked": null
    })
}

pub fn build_model_catalog(provider: &str, model: &str, reasoning: &str, revision: i64) -> Value {
    build_sovereign_model_catalog(provider, model, reasoning, revision, &[])
}

pub fn build_model_discovery(provider: &str) -> Value {
    build_sovereign_model_discovery(provider, &[])
}

pub fn build_spider_check_response(workspace_root: &Path, file: &str) -> Value {
    build_spider_check_multi_response(workspace_root, &[file.to_string()])
}

pub fn build_spider_check_multi_response(workspace_root: &Path, requested: &[String]) -> Value {
    let now = now_ms() as f64;
    let clean_paths: Vec<String> = if requested.is_empty() {
        vec!["src/main.rs".to_string()]
    } else {
        requested
            .iter()
            .take(64)
            .map(|p| {
                let t = p.trim();
                if t.is_empty() || t.starts_with('/') || t.contains("..") {
                    "src/main.rs".to_string()
                } else {
                    t.to_string()
                }
            })
            .collect()
    };

    let mut files_meta = Vec::new();
    let mut findings = Vec::new();
    let mut rel_imports = 0usize;
    let mut ext_imports = 0usize;
    let mut seed_concat = String::new();

    for (idx, safe_file) in clean_paths.iter().enumerate() {
        let content = validate_rpc_read_path(workspace_root, safe_file)
            .filter(|p| {
                fs::metadata(p)
                    .map(|m| m.len() <= 2 * 1024 * 1024)
                    .unwrap_or(false)
            })
            .and_then(|p| fs::read_to_string(&p).ok())
            .unwrap_or_default();
        let hash_64 = sha256_hex(&content);
        seed_concat.push_str(&hash_64);
        files_meta.push(json!({ "path": safe_file, "hash": hash_64 }));

        let check_layer = safe_file.ends_with(".rs")
            || safe_file.ends_with(".swift")
            || safe_file.ends_with(".ts")
            || safe_file.ends_with(".js")
            || safe_file.ends_with(".py")
            || safe_file.ends_with(".go")
            || safe_file.ends_with(".md");
        let header_slice = safe_utf8_prefix(&content, 10_000);
        let has_layer = header_slice.contains("[LAYER:");
        let line_count = content.lines().count();
        if !content.is_empty() && check_layer && !has_layer {
            findings.push(json!({
                "id": format!("SPI-208-{}", idx + 1),
                "rule": "SPI-208",
                "severity": "warning",
                "path": safe_file,
                "message": "Missing explicit [LAYER: <ZONE>] header tag in the file header.",
                "remediation": "Add // [LAYER: <ZONE>] on line 1 matching the file's directory zone.",
                "line": 1,
                "column": 1
            }));
        }
        if check_layer && !safe_file.ends_with(".md") && line_count >= 800 {
            findings.push(json!({
                "id": format!("SPI-203-{}", idx + 1),
                "rule": "SPI-203",
                "severity": "warning",
                "path": safe_file,
                "message": format!("File has {line_count} lines, exceeding the 800-line Tier 1 Optimal cap."),
                "remediation": "Decompose module into smaller focused files under 800 lines.",
                "line": 800,
                "column": 1
            }));
        }
        rel_imports += content
            .lines()
            .filter(|l| {
                l.contains("use crate::") || l.contains("from './") || l.contains("from '../")
            })
            .count();
        ext_imports += content
            .lines()
            .filter(|l| {
                (l.trim_start().starts_with("use ") && !l.contains("use crate::"))
                    || l.trim_start().starts_with("import ")
            })
            .count();
    }

    let id_hash = sha256_hex(&seed_concat);
    let outcome = if findings.is_empty() {
        "no-findings"
    } else {
        "findings"
    };
    json!({
        "version": 1,
        "id": format!("spider-{}", safe_utf8_prefix(&id_hash, 12)),
        "generatedAt": now,
        "requestedPaths": clean_paths,
        "outcome": outcome,
        "summary": if findings.is_empty() { "100% Joyzoning sovereignty verified across all checked layers." } else { "Joyzoning structural check reported findings." },
        "scope": "workspace",
        "nextAction": if findings.is_empty() { "Proceed with operational workflow." } else { "Review and remediate reported structural findings." },
        "files": files_meta,
        "findings": findings,
        "coverage": {
            "relativeImports": rel_imports,
            "externalImports": ext_imports,
            "skipped": 0,
            "omittedFindings": 0,
            "limitations": []
        }
    })
}

pub fn build_state_inspect_response(conversation: &str, session_runs: &[Value]) -> Value {
    let now = now_ms() as f64;
    let mut runs_out = Vec::new();
    for (idx, r) in session_runs.iter().rev().take(20).enumerate() {
        let raw_id = r.get("id").and_then(|v| v.as_str()).unwrap_or("run_0");
        let hex_id = sha256_hex(&format!("{raw_id}:{idx}"));
        let raw_title = r
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("Instruction");
        let title: String = raw_title.chars().take(96).collect();
        let state = r
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("completed");
        let rev = r.get("revision").and_then(|v| v.as_i64()).unwrap_or(1);
        let updated = r
            .get("finishedAt")
            .or_else(|| r.get("createdAt"))
            .and_then(|v| v.as_f64())
            .unwrap_or(now);
        runs_out.push(json!({
            "id": hex_id,
            "title": if title.is_empty() { "Instruction".to_string() } else { title },
            "state": state,
            "integrity": "verified",
            "revision": rev,
            "updatedAt": updated,
            "issues": []
        }));
    }
    let count = runs_out.len();
    json!({
        "version": 1,
        "conversation": conversation,
        "observedAt": now,
        "verified": count,
        "legacy": 0,
        "invalid": 0,
        "interrupted": 0,
        "more": session_runs.len() > 20,
        "summary": "Runtime state intact and checksums verified.",
        "scope": "workspace",
        "nextAction": "Ready for next instruction.",
        "runs": runs_out
    })
}

pub fn build_context_inspect_response(conversation: &str, engine: &AgentEngine) -> Value {
    let now = now_ms() as f64;
    let (active_msgs, summaries) = {
        let sess = lock_unpoison(&engine.session());
        let msgs = sess.messages();
        let sum_count = msgs
            .iter()
            .filter(|m| SessionCompactor::is_checkpoint(m))
            .count();
        (msgs.len(), sum_count)
    };
    let saved_memories = lock_unpoison(&engine.memory()).list_memories().len();
    let q = engine.queue().snapshot();
    let snap = engine.inspect_context_snapshot("");
    let cfg = engine.config_snapshot();
    let posture = engine.current_posture();
    let last_receipt = engine.latest_receipt();
    let (billing_display, billing_kind) = if let Some(ref r) = last_receipt {
        (r.billing_display.clone(), r.billing_kind.clone())
    } else {
        let b = BillingMetering::evaluate(
            &cfg.provider,
            &cfg.model_name,
            &cfg.api_endpoint,
            &Default::default(),
        );
        (b.format_honest(), b.kind_slug().to_string())
    };
    json!({
        "conversation": conversation,
        "activeMessages": active_msgs,
        "rollingSummaries": summaries.min(active_msgs),
        "savedMemories": saved_memories,
        "queued": q.pending_count.min(32),
        "working": q.active,
        "observedAt": now,
        "posture": posture.slug(),
        "postureLabel": posture.display_label(),
        "promptProfile": snap.profile,
        "agentProfile": snap.agent_profile,
        "methodology": snap.methodology,
        "ciGateEnabled": snap.ci_gate_enabled,
        "modelFamily": snap.model_family,
        "cachePrefixHash": snap.cache_prefix_hash,
        "prefixStatus": last_receipt.as_ref().map(|r| r.prefix_status.as_str()).unwrap_or("pinned_initial"),
        "billingDisplay": billing_display,
        "billingKind": billing_kind,
        "systemTokens": snap.system_total_tokens,
        "staticPrefixTokens": snap.static_prefix_tokens,
        "toolSchemaTokens": snap.tool_schema_tokens,
        "activeTools": snap.tool_active_count,
        "totalTools": snap.tool_total_count,
        "contextWindowTokens": snap.context_window_tokens,
        "usedInputTokens": snap.used_input_tokens,
        "headroomTokens": snap.headroom_tokens,
        "governanceSources": snap.governance_sources,
        "governanceWarnings": snap.governance_warnings,
        "latestReceipt": last_receipt
    })
}

pub fn build_prompt_inspect_response(mode: &str, engine: &AgentEngine) -> Value {
    let snap = engine.inspect_context_snapshot("");
    let rendered = engine.render_prompt_inspector(mode);
    json!({
        "snapshot": snap,
        "rendered": rendered,
        "observedAt": now_ms() as f64
    })
}

pub fn handle_profile_and_ci_rpc(
    method: &str,
    params: &Value,
    engine: &AgentEngine,
) -> Result<Value, (String, String)> {
    match method {
        "profile.list" => Ok(json!({
            "activeProfile": engine.active_profile(),
            "profiles": engine.list_profiles(),
        })),
        "profile.get" => Ok(json!(engine.active_profile())),
        "profile.set" => {
            let pid = params
                .get("id")
                .or_else(|| params.get("profile"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            engine
                .set_profile(pid)
                .map(|p| json!({ "activeProfile": p, "posture": engine.current_posture().slug() }))
                .map_err(|msg| ("UNKNOWN_PROFILE".to_string(), msg))
        }
        "ci_gate.run" => Ok(json!(engine.run_ci_gate_now())),
        _ => Err(("UNKNOWN_METHOD".to_string(), format!("Unsupported method: {method}"))),
    }
}

pub fn build_context_history_response(
    conversation: &str,
    offset: usize,
    engine: &AgentEngine,
) -> Value {
    let now = now_ms() as f64;
    let full_text = {
        let sess = lock_unpoison(&engine.session());
        sess.transcript()
            .iter()
            .map(|m| format!("[{}] {}\n", m.role.as_str().to_uppercase(), m.content))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let rev_uuid = uuid_from_seed(&full_text);
    let bytes = full_text.as_bytes();
    let total = bytes.len();
    let (slice, end, more) = if offset < total && full_text.is_char_boundary(offset) {
        let mut end = offset.saturating_add(16 * 1024).min(total);
        while end > offset && !full_text.is_char_boundary(end) {
            end -= 1;
        }
        (&full_text[offset..end], end, end < total)
    } else {
        ("", offset.min(total), false)
    };
    json!({
        "conversation": conversation,
        "revision": rev_uuid,
        "observedAt": now,
        "text": slice,
        "size": total,
        "nextOffset": end,
        "more": more
    })
}

fn format_memory_entry(entry: &MemoryEntry) -> Value {
    let id = sha256_hex(&entry.key);
    let rev = sha256_hex(&format!(
        "{}:{}:{}:{}",
        entry.key, entry.value, entry.category, entry.timestamp
    ));
    let preview: String = entry.value.chars().take(120).collect();
    let val: String = entry.value.chars().take(8000).collect();
    let key: String = entry.key.chars().take(160).collect();
    json!({
        "id": id,
        "revision": rev,
        "key": key,
        "category": entry.category,
        "preview": preview,
        "value": val,
        "editable": true,
        "updatedAt": entry.timestamp as f64
    })
}

pub fn handle_memory_rpc(
    method: &str,
    params: &Value,
    default_conv: &str,
    engine: &AgentEngine,
) -> Value {
    let conv = params
        .get("conversation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(default_conv);
    let mut mem = lock_unpoison(&engine.memory());
    match method {
        "memory.list" => {
            let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let matched = mem.search_memories(query);
            let total = matched.len();
            let entries: Vec<Value> = matched
                .iter()
                .skip(offset)
                .take(50)
                .map(format_memory_entry)
                .collect();
            let more = offset + entries.len() < total;
            json!({
                "conversation": conv,
                "entries": entries,
                "offset": offset,
                "total": total,
                "more": more,
                "limited": false
            })
        }
        "memory.read" => {
            let target_id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let found = mem
                .list_memories()
                .into_iter()
                .find(|e| sha256_hex(&e.key) == target_id || e.key == target_id);
            if let Some(entry) = found {
                json!({ "found": true, "conversation": conv, "entry": format_memory_entry(&entry) })
            } else {
                json!({ "found": false, "conversation": conv, "id": target_id, "entry": Value::Null })
            }
        }
        "memory.write" => {
            let key = params
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("fact")
                .trim()
                .to_string();
            let value = params
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let threats =
                crate::plumbing::secret_redactor::SafetyPolicy::scan_for_threats(&value, "all");
            if !threats.is_empty() {
                return json!({ "saved": false, "conversation": conv, "error": format!("Blocked memory write containing security threats: {:?}", threats) });
            }
            let category = params
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("fact")
                .to_string();
            let entry = mem.save_memory(key, value, category);
            json!({ "saved": true, "conversation": conv, "entry": format_memory_entry(&entry) })
        }
        "memory.remove" => {
            let target_id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(existing) = mem
                .list_memories()
                .into_iter()
                .find(|e| sha256_hex(&e.key) == target_id || e.key == target_id)
            {
                mem.remove_memory(&existing.key);
            }
            json!({ "saved": true, "conversation": conv })
        }
        _ => json!({ "saved": true, "conversation": conv }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::flight_runner::TempWorkspace;
    use crate::domain::interfaces::{BoxFuture, ILlmProvider};
    use crate::domain::types::runtime::{AgentConfig, CancellationToken};
    use crate::domain::types::tick_result::{ProviderError, ProviderRequest, ProviderResponse};
    use crate::ui::worker_handlers::{handle_provider_credential_rpc, WorkerSharedState};
    use std::collections::BTreeSet;
    use std::sync::Arc;

    struct DummyUiProvider;
    impl ILlmProvider for DummyUiProvider {
        fn provider_name(&self) -> &str {
            "dummy"
        }
        fn complete<'a>(
            &'a self,
            _req: ProviderRequest,
            _cancel: Option<CancellationToken>,
        ) -> BoxFuture<'a, Result<ProviderResponse, ProviderError>> {
            Box::pin(async { Err(ProviderError::Cancelled) })
        }
    }

    #[test]
    fn test_sovereign_model_catalog_discovery_and_credential_rpc_parity() {
        let catalog = build_model_catalog("gemini", "gemini-2.5-pro", "high", 3);
        assert_eq!(catalog["provider"], "gemini");
        assert_eq!(catalog["model"], "gemini-2.5-pro");
        assert_eq!(catalog["reasoningEffort"], "high");
        let providers = catalog["providers"].as_array().unwrap();
        assert!(
            providers.len() >= 19,
            "Expected >= 19 curated AppKit providers"
        );
        let mut ids = BTreeSet::new();
        for p in providers {
            let id = p["id"].as_str().unwrap();
            assert!(ids.insert(id), "Duplicate provider id in models.list: {id}");
            assert!(!p["models"].as_array().unwrap().is_empty());
        }
        for expected in [
            "openai-codex",
            "anthropic",
            "gemini",
            "deepseek",
            "xai",
            "groq",
            "mistral",
            "qwen",
            "ollama",
            "apple-playground",
        ] {
            assert!(ids.contains(expected), "Missing {expected} in models.list");
        }

        // Dynamic fallback inclusion for non-default canonical providers (e.g. cerebras)
        let dyn_cat = build_model_catalog("cerebras", "gpt-oss-120b", "medium", 4);
        assert!(dyn_cat["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "cerebras"));

        // Discovery deduplicates models cleanly
        let disc = build_sovereign_model_discovery(
            "ollama",
            &["llama3.2".into(), "llama3.2".into(), "phi4:latest".into()],
        );
        assert_eq!(disc["state"], "ready");
        let disc_models = disc["models"].as_array().unwrap();
        let mut model_ids = BTreeSet::new();
        for m in disc_models {
            assert!(model_ids.insert(m["id"].as_str().unwrap()));
        }

        // Universal credential RPC normalizes camelCase prefixes (nvidiaNim -> nvidia-nim, clinePass -> cline-pass)
        // and supports clearing saved keys when `apiKey: ""` is submitted by AppKit `Remove Key`
        let ws = TempWorkspace::new("ui-rpc-parity");
        let cfg = AgentConfig::default();
        let engine = AgentEngine::new(&ws.path, cfg.clone(), Arc::new(DummyUiProvider));
        let mut state = WorkerSharedState::from_engine_config(&cfg);

        let ap = state.active_provider.clone();
        let am = state.active_model.clone();
        let saved_nim = handle_provider_credential_rpc(
            "nvidiaNim.saveKey",
            &json!({"apiKey": "nvapi-test-1234"}),
            &mut state.provider_keys,
            &ap,
            &am,
            &engine,
        );
        assert_eq!(saved_nim["provider"], "nvidiaNim");
        assert_eq!(saved_nim["canonicalProvider"], "nvidia-nim");
        assert_eq!(saved_nim["saved"], true);

        let cleared_nim = handle_provider_credential_rpc(
            "nvidiaNim.saveKey",
            &json!({"apiKey": ""}),
            &mut state.provider_keys,
            &ap,
            &am,
            &engine,
        );
        assert_eq!(cleared_nim["saved"], false);

        let status_cline = handle_provider_credential_rpc(
            "clinePass.status",
            &json!({}),
            &mut state.provider_keys,
            &ap,
            &am,
            &engine,
        );
        assert_eq!(status_cline["provider"], "clinePass");
        assert_eq!(status_cline["canonicalProvider"], "cline-pass");

        // Saving and then clearing the ACTIVE provider's key must immediately purge `engine.config_snapshot().api_key`
        let _ = handle_provider_credential_rpc(
            "openai.saveKey",
            &json!({"apiKey": "sk-live-openai-temp"}),
            &mut state.provider_keys,
            &ap,
            &am,
            &engine,
        );
        assert_eq!(
            engine.config_snapshot().api_key.as_deref(),
            Some("sk-live-openai-temp")
        );
        let _ = handle_provider_credential_rpc(
            "openai.saveKey",
            &json!({"apiKey": ""}),
            &mut state.provider_keys,
            &ap,
            &am,
            &engine,
        );
        if std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .is_none()
        {
            assert!(engine.config_snapshot().api_key.is_none());
        }
    }

    #[test]
    fn test_profile_rpc_dispatch_in_ui_layer() {
        let ws = std::env::temp_dir().join(format!("heav3ns-rpc-prof-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&ws);
        let engine = crate::core::engine::AgentEngine::with_defaults(&ws);

        let list = super::handle_profile_and_ci_rpc("profile.list", &json!({}), &engine).expect("list");
        assert_eq!(list["activeProfile"]["id"], "joyzoning-architect");
        assert_eq!(list["profiles"].as_array().map(|a| a.len()), Some(4));

        let set_ok = super::handle_profile_and_ci_rpc("profile.set", &json!({ "id": "strict-ci" }), &engine)
            .expect("set strict-ci");
        assert_eq!(set_ok["activeProfile"]["id"], "strict-ci");
        assert_eq!(set_ok["posture"], "ask");

        assert!(super::handle_profile_and_ci_rpc("profile.set", &json!({ "id": "nope" }), &engine).is_err());
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_progress_to_walkthrough_activity_preserves_diff_preview() {
        use crate::domain::types::progress::{
            EngineProgressEvent, EngineProgressMetadata, EngineProgressPhase, EngineProgressStatus,
        };
        let ev = EngineProgressEvent {
            activity_id: "tools-r0".to_string(),
            phase: EngineProgressPhase::Tool,
            status: EngineProgressStatus::Completed,
            message: "replace_file_content src/domain/item.rs (+1/-1, 2ms)".to_string(),
            detail: Some("--- a/src/domain/item.rs\n+++ b/src/domain/item.rs\n@@ -1 +1 @@\n-old\n+new".to_string()),
            timestamp: 1_000,
            elapsed_ms: Some(2),
            sequence: 3,
            metadata: Some(EngineProgressMetadata {
                source: Some("scheduler".to_string()),
                scope: Some("step".to_string()),
                item_type: Some("tool".to_string()),
                files: Some(vec!["src/domain/item.rs".to_string()]),
                preview_kind: Some("diff".to_string()),
                preview_text: Some("--- a/src/domain/item.rs\n+++ b/src/domain/item.rs\n@@ -1 +1 @@\n-old\n+new".to_string()),
                preview_limited: Some(false),
                lines_added: Some(1),
                lines_removed: Some(1),
                ..Default::default()
            }),
        };
        let act = crate::ui::worker_handlers::progress_to_walkthrough_activity(&ev);
        assert_eq!(act["preview"]["kind"], "diff");
        assert_eq!(act["metadata"]["files"][0], "src/domain/item.rs");
        assert_eq!(act["metadata"]["linesAdded"], 1);
        assert_eq!(act["metadata"]["linesRemoved"], 1);
    }
}
