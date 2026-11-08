<!-- [LAYER: UI] -->
# Getting Started with HEAV3NS-MICRO

<p align="center">
  <a href="README.md">Docs Hub</a> ·
  <a href="AGENT_STACK.md">Agent Stack</a> ·
  <a href="OPERATIONS.md">Operations</a> ·
  <a href="api/cli.md">CLI Reference</a> ·
  <a href="../README.md">Repository README</a>
</p>

Get up and running with **HEAV3NS-MICRO** in under 10 minutes. This tutorial guides you from compiling the Rust binary to running the 5-scenario Agent Flight suite and executing interactive turns in the CLI REPL.

---

## Prerequisites

- **Rust toolchain**: `rustc` and `cargo` (Rust `1.75` or newer, 2021 edition)
- **Node.js**: `node` (v18 or newer, for running `joy-check` and `docs-check`)
- **Git**: For version control and workspace tracking

---

## 1. Clone & Build

Clone the repository and build the release binary:

```bash
git clone https://github.com/heav3ns-team/heav3ns-micro.git
cd heav3ns-micro

# Build the binary
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
running 8 tests
test core::substrate_tests::tests::test_arena_allocator_alignment_and_o1_rewind ... ok
test core::substrate_tests::tests::test_wave_scheduler_partitioning ... ok
test core::substrate_tests::tests::test_self_healing_parser_and_streaming_dedup ... ok
test core::substrate_tests::tests::test_vfs_baseline_verification_and_snapshot_rewind ... ok
test core::substrate_tests::tests::test_skills_ingestor_and_topological_swarm_execution ... ok
test core::substrate_tests::tests::test_secret_redactor_and_sensitive_path_guard ... ok
test core::substrate_tests::tests::test_deterministic_patch_engine_v4a_fuzzy_heal_and_symbols ... ok
test core::substrate_tests::tests::test_agent_flight_suite_passes_100_percent ... ok

test result: ok. 8 passed; 0 failed
```

---

## 3. Execute the 5-Scenario Agent Flight Benchmark

HEAV3NS-MICRO features an embedded multi-turn benchmark suite ([`FlightRunner`](../src/core/flight_runner.rs)) verifying end-to-end agent capabilities without external network dependencies:

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

## 4. Run an Interactive Agent Session

Launch an interactive prompt session:

```bash
cargo run -- -p "Inspect the src/domain directory and list all public traits"
```

Or start the interactive REPL:

```bash
cargo run
```

Inside the REPL, you can explore the 10 built-in slash commands:
- `/help`: Show command documentation
- `/status`: Display session token count, arena memory usage, and authority level
- `/vfs`: List staged virtual file system overlays
- `/commit`: Commit staged VFS overlays to disk
- `/rewind`: Roll back the last recorded file transaction
- `/compact`: Trigger explicit session memory compaction
- `/doctor`: Perform runtime self-check and diagnostic inspection

---

## 5. Verify Architectural Sovereignty

Run the Joyzoning compliance check to verify layer purity and zero circular dependencies:

```bash
# Check Joyzoning boundaries
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict

# Check documentation links and structural taxonomies
npm run docs:check
```

---

## Next Steps

- Explore the [Agent Stack](AGENT_STACK.md) to understand the three operational surfaces.
- Read [Substrate Primitives](PORTABLE_TRINITY.md) for details on the memory arena and VFS.
- Review [Architecture Decision Records](adr/README.md) to understand core engineering choices.
