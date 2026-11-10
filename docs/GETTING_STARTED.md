<!-- [LAYER: UI] -->
# Getting Started with HEAV3NS-MICRO

<p align="center">
  <a href="README.md">Docs Hub</a> ·
  <a href="AGENT_STACK.md">Agent Stack</a> ·
  <a href="OPERATIONS.md">Operations</a> ·
  <a href="api/cli.md">CLI Reference</a> ·
  <a href="../README.md">Repository README</a> ·
  <a href="../native/PRODUCT.md">Native Product Spec</a>
</p>

Get up and running with **HEAV3NS-MICRO** in under 10 minutes. This tutorial guides you from compiling the Rust binary to running the 5-scenario Agent Flight suite, launching the AppKit native desktop client, and executing interactive turns.

---

## Prerequisites

- **Rust toolchain**: `rustc` and `cargo` (Rust `1.75` or newer, 2021 edition)
- **Apple Swift toolchain**: Swift `5.9` or newer with macOS 14+ SDK (for `native/`)
- **Node.js**: `node` (v18 or newer, for running `joy-check` and `docs-check`)
- **Git**: For version control and workspace tracking

---

## 1. Clone & Build

Clone the repository and build the release binary:

```bash
git clone https://github.com/heav3ns-team/heav3ns-micro.git
cd heav3ns-micro

# Build the Rust substrate binary
cargo build --release
```

The resulting binary will be placed at `./target/release/heav3ns-micro`.

---

## 2. Run the Verification Smoke Test

Ensure your environment and substrate primitives are operating with mathematical fidelity:

```bash
cargo test
cargo run -- --smoke
```

Expected output:
```text
running 77 tests
test core::substrate_tests::tests::test_arena_allocator_alignment_and_o1_rewind ... ok
test core::substrate_tests::tests::test_wave_scheduler_partitioning ... ok
test core::substrate_tests::tests::test_self_healing_parser_and_streaming_dedup ... ok
test core::substrate_tests::tests::test_vfs_baseline_verification_and_snapshot_rewind ... ok
test core::substrate_tests::tests::test_skills_ingestor_and_topological_swarm_execution ... ok
test core::substrate_tests::tests::test_secret_redactor_and_sensitive_path_guard ... ok
test core::substrate_tests::tests::test_deterministic_patch_engine_v4a_fuzzy_heal_and_symbols ... ok
test core::substrate_tests::tests::test_agent_flight_suite_passes_100_percent ... ok
test core::codewhale_zenith_tests::tests::test_seatbelt_sbpl_generation_and_credential_scopes ... ok
test core::codewhale_zenith_tests::tests::test_execution_posture_and_constitution_plan_enforcement ... ok
test core::codewhale_zenith_tests::tests::test_pure_rust_cas_undo_and_would_block ... ok
test core::codewhale_zenith_tests::tests::test_tool_search_lru_and_declarative_batch_execution ... ok

test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 3. Run the Agent Flight Test Suite

Verify end-to-end agent capabilities against the deterministic mock provider:

```bash
cargo run -- --flight
```

The flight test exercises:
1. **FT-01**: Multi-step file discovery and AST inspection
2. **FT-02**: Staged VFS overlay modification and atomic commit
3. **FT-03**: Self-healing streaming JSON repair and deduplication
4. **FT-04**: Concurrent tool wave scheduling and read/write barriers
5. **FT-05**: Credential redaction and sensitive path interdiction

All 19 assertion points across the 5 scenarios must report green `PASS (100%)`.

---

## 4. Run the Native macOS AppKit Test Suite

Verify the native desktop application and its bidirectional JSON-RPC integration:

```bash
cd native
swift test
cd ..
```

Expected output:
```text
Test Suite 'All tests' passed at ...
	 Executed 321 tests, with 0 failures (0 unexpected)
```

---

## 5. Run an Interactive Agent Session

Launch an interactive prompt session:

```bash
cargo run -- -p "Inspect the src/domain directory and list all public traits"
```

Or start the interactive REPL:

```bash
cargo run
```

Inside the REPL, explore the 35 built-in slash commands:
- `/help`: Show command documentation
- `/mode` or `/posture`: Cycle through the 5 execution postures (`Ask -> AcceptEdits -> Plan -> AutoReview -> FullAccess`)
- `/preview-request`: Inspect exact canonical byte sum and outbound request manifest
- `/receipts`: Inspect auditable verification receipts (`heav3ns.receipt/v1`) from recent turns
- `/models`: View live frontier model catalog and pricing (`cargo run -- models --refresh`)
- `/status`: Display session token count, arena memory usage, and authority level
- `/context`: Visual 20-cell token budget HUD and tool routing breakdown
- `/prompt`: Inspect deterministic KV-cached static prefix or profile outputs
- `/tools`: Display active core vs. on-demand specialist tools and token savings
- `/vfs`: List staged virtual file system overlays
- `/undo`: Safe, non-Git post-turn undo backed by pure-Rust SHA-256 CAS
- `/diff`: Renders non-destructive diff against pre-turn CAS snapshot
- `/commit`: Commit staged VFS overlays to disk
- `/rewind`: Roll back the last recorded file transaction
- `/compact`: Trigger explicit session memory compaction
- `/doctor`: Perform runtime self-check, prompt hygiene, and diagnostic inspection

---

## 6. Verify Architectural Sovereignty

Run the Joyzoning compliance check to verify layer purity and zero circular dependencies:

```bash
# Check Joyzoning boundaries
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict

# StabilityDoctor health audit
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --doctor
```

---

## Next Steps

- Explore the [Architecture Current Spec](architecture/current.md) to understand dual-layer topology.
- Read [ADR-0010](adr/0010-native-desktop-integration-and-rpc-protocol.md) for desktop JSON-RPC details.
- Review [Operational Playbooks](playbooks/README.md) for incident runbooks and triage procedures.
