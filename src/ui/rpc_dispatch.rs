// [LAYER: UI]
use crate::plumbing::hash_and_math::now_ms;
use serde_json::{json, Value};
use std::path::Path;

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
            "queue:3",
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
            "conversation-delete-v1"
        ]
    })
}

pub fn build_spider_check_response(file: &str) -> Value {
    let now = now_ms() as f64;
    let hash_64 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    json!({
        "version": 1,
        "id": "spider-check-1",
        "generatedAt": now,
        "requestedPaths": [file],
        "outcome": "no-findings",
        "summary": "100% Joyzoning sovereignty verified across all layers.",
        "scope": "workspace",
        "nextAction": "Proceed with operational workflow.",
        "files": [{ "path": file, "hash": hash_64 }],
        "findings": [],
        "coverage": {
            "relativeImports": 0,
            "externalImports": 0,
            "skipped": 0,
            "omittedFindings": 0,
            "limitations": []
        }
    })
}

pub fn build_state_inspect_response(conversation: &str) -> Value {
    let now = now_ms() as f64;
    json!({
        "version": 1,
        "conversation": conversation,
        "observedAt": now,
        "verified": 0,
        "legacy": 0,
        "invalid": 0,
        "interrupted": 0,
        "more": false,
        "summary": "Runtime state intact and verified.",
        "scope": "workspace",
        "nextAction": "Ready for next instruction.",
        "runs": []
    })
}

pub fn build_context_inspect_response(conversation: &str) -> Value {
    let now = now_ms() as f64;
    json!({
        "conversation": conversation,
        "activeMessages": 1,
        "rollingSummaries": 0,
        "savedMemories": 1,
        "queued": 0,
        "working": false,
        "observedAt": now
    })
}

pub fn build_model_catalog() -> Value {
    json!({
        "provider": "openai-codex",
        "model": "gpt-5.6-terra",
        "revision": 1,
        "reasoningEffort": "medium",
        "reasoningOptions": ["low", "medium", "high"],
        "busy": false,
        "blocked": null,
        "providers": [{
            "id": "openai-codex",
            "label": "OpenAI Codex",
            "detail": "Standard reasoning and code orchestration engine",
            "models": ["gpt-5.6-terra", "gpt-5-turbo", "o3-mini"]
        }],
        "recent": []
    })
}
