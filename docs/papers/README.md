<!-- [LAYER: UI] -->
# Papers & Theoretical Foundations

<p align="center">
  <a href="../README.md">Docs Hub</a> ·
  <a href="../../doc.md">Foundational Charter (doc.md)</a> ·
  <a href="ascendant-intelligence.md">Ascendant Intelligence</a> ·
  <a href="companion-brief.md">Companion Brief</a> ·
  <a href="empirical-benchmarking.md">Empirical Benchmarking</a> ·
  <a href="philosophy.md">Philosophy</a> ·
  <a href="whitepaper.md">Whitepaper</a> ·
  <a href="../wiki/README.md">Wiki</a>
</p>

This section contains the theoretical, mathematical, and empirical foundations of **HEAV3NS — The Sovereign Intelligence Architecture** and its hyper-compressed runtime **HEAV3NS-MICRO**, providing foundational doctrine, philosophical principles, formal specifications, and executive metrics.

---

## Papers Index

| Paper | Scope | Key Topics |
| :--- | :--- | :--- |
| **[The Ascendant Computational Intelligence](ascendant-intelligence.md)** | Foundational Doctrine & Cybernetic Theory | The Seven Sovereign Pillars (`HEAV3NS-MICRO`, `SPIDER`, `REWIND`, `BROCCOLI`, `SYNTHETIC SWARM`, `COMPUTER USE`, `THE HARNESS`), emergent intelligence formalism, and the 5 Grand Challenges. |
| **[Companion Brief](companion-brief.md)** | Executive & SRE Overview | Live workspace-verified metrics (`70` Rust modules, `39` `I<Name>` traits, `29` tools, `66` Rust tests, `315` AppKit tests, `E_S = 0.0041`), benchmark throughput, and operational comparison. |
| **[Empirical Benchmarking](empirical-benchmarking.md)** | Empirical Evaluation & Stress Dynamics | Live micro-benchmarks ($260\text{k}$ allocs/sec, $6.3\text{k}$ parser repairs/sec, $17\text{k}$ spatial groundings/sec) and real-world workspace task verification across 4 testbeds in `/Users/bozoegg/Desktop/tests`. |
| **[Philosophy](philosophy.md)** | Engineering Values & Ethics | Spatial sovereignty, embodied agency beyond passive token prediction, bounded memory, and reversible reality parity. |
| **[Technical Whitepaper](whitepaper.md)** | Formal Substrate & Spatial Specification | Arena bump allocation, self-healing streaming parsing, two-phase VFS, Double-Wrapped Spatial Action Mediation (`Wrapper A` + `Wrapper B`), 9-stage Closed-Loop Computer Use, and AppKit JSON-RPC IPC. |

---

## Reading Order

1. **Visionaries & System Leaders**: Begin with [`doc.md`](../../doc.md) and **[The Ascendant Computational Intelligence](ascendant-intelligence.md)** to understand the seven cooperating pillars of the HEAV3NS computational civilization.
2. **New Visitors & Executives**: Read the **[Companion Brief](companion-brief.md)** for a concise, workspace-verified summary of metrics, capabilities, and architectural guarantees.
3. **Performance Engineers & SREs**: Read **[Empirical Benchmarking](empirical-benchmarking.md)** for quantitative throughput profiling, stress dynamics, and real-world multi-language workspace evaluation.
4. **Software Architects**: Read **[Philosophy](philosophy.md)** to understand why Joyzoning spatial sovereignty, embodied agency, and zero-allocation memory models matter for autonomous systems.
5. **Core Substrate Engineers**: Study the **[Technical Whitepaper](whitepaper.md)** for mathematical models, formal algorithms, and empirical verification data.

---

## Verification

All quantitative claims made across these papers are verified directly against the live repository:

```bash
# Verify Rust test suite (66 tests) and Agent Flight benchmarks (FT-01..FT-05)
cargo test
cargo run -- --flight

# Verify Native macOS AppKit test suite (315 tests)
cd native && swift test && cd ..

# Verify 100% Joyzoning Stability Index and documentation guardrails
node /Users/bozoegg/Desktop/HEAV3NS-JOY/bin/joy-check.mjs . --theme strict
node bin/docs-check.mjs
```
