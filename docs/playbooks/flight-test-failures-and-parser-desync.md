<!-- [LAYER: UI] -->
# Playbook PB-03: Flight Test Failures & Parser Desync

<p align="center">
  <a href="README.md">Playbooks Hub</a> ·
  <a href="../README.md">Docs Hub</a> ·
  <a href="../adr/0003-self-healing-tool-call-parser.md">ADR-0003</a> ·
  <a href="../adr/0006-stdio-jsonl-and-flight-test-harness.md">ADR-0006</a>
</p>

## Triage at a glance

| Field | Detail |
| :--- | :--- |
| **Playbook ID** | `PB-03` |
| **Severity** | `SEV-2` (CI flight benchmark failure / parse error) |
| **Trigger Codes** | Flight test exit `1`, `JsonParseError`, `UnbalancedBraces` |
| **Time to Mitigate** | `2–8 minutes` |
| **First Command** | `cargo run -- --flight` |
| **Rollback Safety** | Flight suite runs against an isolated mock provider |

---

## Remediation steps

### Case 1: Scenario FT-03 Fails (Streaming JSON Repair Desync)
1. **Identify the assertion point**: Run `cargo run -- --flight` and note which scenario assertion failed.
2. **Inspect the raw stream delta**:
   - Check if the upstream provider emitted an unescaped character within a string literal.
   - Verify `src/plumbing/json_repair.rs` correctly counts brace depth across escaped quotes (`\"`).
3. **Run targeted regression test**:
   ```bash
   cargo test test_self_healing_parser_and_streaming_dedup
   ```

### Case 2: Markdown Delimiter Stripping Failure
1. If the model wraps its tool call payload in multiple nested code fences (e.g. ` ```json\n``` `):
   - Ensure `extract_json_roots` strips outermost markdown fences while preserving interior code snippets inside patch hunks.

### Case 3: Streaming Tool Name Fragment Desync
1. If tool names are received in fragmented chunks (e.g. `fs_` followed by `write`):
   - Check `accumulate_streaming_tool_name()` in `src/plumbing/json_repair.rs` to verify token accumulation.

---

## Verification

```bash
cargo run -- --flight
cargo test test_agent_flight_suite_passes_100_percent
```

Confirm that all 5 scenarios pass with 19/19 checks green.
