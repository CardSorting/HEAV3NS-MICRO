<!-- [LAYER: UI] -->
# Papers & Theoretical Foundations

<p align="center">
  <a href="../README.md">Docs Hub</a> ·
  <a href="companion-brief.md">Companion Brief</a> ·
  <a href="philosophy.md">Philosophy</a> ·
  <a href="whitepaper.md">Whitepaper</a> ·
  <a href="../wiki/README.md">Wiki</a>
</p>

This section contains the theoretical and empirical foundations of **HEAV3NS-MICRO**, providing academic, philosophical, and executive perspectives on hyper-compressed agent runtime engineering.

---

## Papers Index

| Paper | Scope | Key Topics |
| :--- | :--- | :--- |
| **[Companion Brief](companion-brief.md)** | Executive & SRE Overview | Live workspace-verified metrics, benchmark throughput, and operational comparison. |
| **[Philosophy](philosophy.md)** | Engineering Values & Ethics | Spatial sovereignty, zero-allocation agent loops, and human-in-the-loop auditability. |
| **[Technical Whitepaper](whitepaper.md)** | Formal Substrate Specification | Arena bump allocation, self-healing streaming parsing, and topological wave scheduling. |

---

## Reading Order

1. **New Visitors & Executives**: Start with the [Companion Brief](companion-brief.md) for a concise, verified summary of performance characteristics and architectural pillars.
2. **Software Architects**: Read [Philosophy](philosophy.md) to understand why Joyzoning spatial sovereignty and zero-allocation memory models matter for autonomous systems.
3. **Core Substrate Engineers**: Study the [Technical Whitepaper](whitepaper.md) for detailed mathematical models, formal algorithms, and empirical verification data.

---

## Verification

All quantitative claims made across these papers are verified directly against the live repository:

```bash
# Verify flight benchmarks and test suite
cargo test
cargo run -- --flight

# Verify documentation metrics and links
npm run docs:check
```
