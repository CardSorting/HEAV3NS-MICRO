<!-- [LAYER: UI] -->
# HEAV3NS-MICRO Technical Whitepaper

**A Zero-Bloat, High-Throughput Rust Micro Agent Runtime**  
*HEAV3NS Architecture Working Group*

<p align="center">
  <a href="README.md">Papers Hub</a> ·
  <a href="companion-brief.md">Companion Brief</a> ·
  <a href="philosophy.md">Philosophy</a> ·
  <a href="../architecture/current.md">Architecture</a>
</p>

---

## Abstract

We present **HEAV3NS-MICRO**, a hyper-compressed, deterministic agent execution runtime written in Rust. By organizing system modules across five sovereign architectural zones and enforcing mathematical dependency constraints, HEAV3NS-MICRO eliminates runtime heap fragmentation, parser desynchronization, and speculative disk corruption. Central to the runtime is a contiguous 16-MiB arena allocator enabling constant-time $O(1)$ scratch memory reclamation, an incremental self-healing JSON parser resilient to streaming markdown block fences, a two-phase virtual file system with baseline hash verification, and a topological wave scheduler for concurrent tool execution. We demonstrate 100% architectural sovereignty, zero blocking violations, and complete passing verification across the 5-scenario Agent Flight benchmark.

---

## 1. Introduction & Motivation

Autonomous software development workflows require executing LLM-generated operations against real file systems, terminal processes, and code repositories. Conventional runtime environments exhibit three systemic vulnerabilities:
1. **Dynamic Memory Sprawl**: Recursive parsing and unconstrained string concatenation incur thousands of small heap allocations, triggering frequent memory reallocations and unpredictable latency.
2. **Streaming Parser Fragility**: LLMs emit fragmented or subtly malformed JSON outputs (e.g., unmatched quotes, trailing commas, mixed markdown fences) that fail standard strict JSON deserializers.
3. **Speculative Filesystem Corruption**: Tool execution without transactional boundaries risks modifying files on disk based on outdated repository state, clobbering concurrent developer edits.

HEAV3NS-MICRO resolves these challenges through a unified, zero-bloat Rust architecture.

---

## 2. Memory Arena & $O(1)$ Rewind Formalism

HEAV3NS-MICRO allocates a fixed, contiguous memory slab of size $M = 16 \times 1024 \times 1024$ bytes (16 MiB) at startup:

$$\text{Arena} = [b_0, b_1, \dots, b_{M-1}]$$

Allocations within the arena proceed linearly via a bump pointer $P_{\text{alloc}} \in [0, M)$. For any allocation request of size $S$ with alignment $A = 8$:

$$P_{\text{aligned}} = (P_{\text{alloc}} + A - 1) \ \& \ \sim(A - 1)$$

$$P_{\text{next}} = P_{\text{aligned}} + S$$

If $P_{\text{next}} > M$, the allocator returns an `ArenaOom` error, prompting explicit compaction.

### Turn Watermarks & Constant-Time Rewinding
At the start of turn $t$, the engine records the current watermark $W_t = P_{\text{alloc}}$. During the turn, transient string deltas, tool argument buffers, and intermediate AST nodes are allocated bump-by-bump. Upon turn completion, scratch allocations are reclaimed in $O(1)$ time by resetting the allocation pointer:

$$P_{\text{alloc}} \leftarrow W_t$$

This eliminates heap fragmentation and avoids individual object deallocation overhead.

---

## 3. Self-Healing Streaming Parser

Let $S$ be a streaming string buffer emitted by an LLM containing potential tool calls. The parser implements a multi-stage normalization pipeline:
1. **Markdown Fence Stripping**: Matches and strips leading/trailing codeblock delimiters (` ```json ... ``` `).
2. **Bracket Depth Balancing**: Computes open and closed bracket counters for both `{}` and `[]`. If unmatched openers exist at stream end, the repair engine appends the necessary closing tokens.
3. **Escaped Character Normalization**: Handles unescaped newlines and carriage returns occurring within multi-line patch hunks.

This ensures tool calls are extracted successfully even when models emit truncated tokens under token budget limits.

---

## 4. Two-Phase Staged VFS & Rollback Formalism

Disk modifications are staged through [`SessionVfs`](../../src/infrastructure/vfs_store.rs). When a tool requests a file write at path $F$ with content $C_{\text{new}}$:

1. **Baseline Ingestion**: The system reads the existing disk file $C_{\text{disk}}$ and computes its baseline:
   $$\text{Baseline}(F) = \big(\text{byte\_len}(C_{\text{disk}}), \text{FNV1a-32}(C_{\text{disk}})\big)$$
2. **Overlay Staging**: The new content is staged in memory as an active overlay.
3. **Commit Phase**: Prior to flushing $C_{\text{new}}$ to physical storage, the system re-evaluates the disk state:
   $$\text{CurrentDisk}(F) \stackrel{?}{=} \text{Baseline}(F)$$
   If the check passes, the file is written, and an undo delta is pushed to `TransactionJournal`. If the disk has changed externally, the commit aborts with `VfsConflict`.

---

## 5. Topological Wave Scheduler

Given a set of emitted tool calls $T = \{t_1, t_2, \dots, t_k\}$, each tool is mapped to its side-effect property $\text{Effect}(t) \in \{\text{ReadOnly}, \text{Mutating}\}$:

$$\text{Wave}_{\text{read}} = \{t \in T \mid \text{Effect}(t) = \text{ReadOnly}\}$$

$$\text{Wave}_{\text{write}} = [t_1, t_2, \dots] \quad \text{where} \ \forall t, \text{Effect}(t) = \text{Mutating}$$

The wave scheduler executes $\text{Wave}_{\text{read}}$ concurrently across background worker tasks using async join pools. Following the completion of all read operations, each mutating tool in $\text{Wave}_{\text{write}}$ executes in strict serialized sequence.

---

## 6. Empirical Verification & Conclusion

HEAV3NS-MICRO has been verified against the 5-scenario Agent Flight suite (`FT-01..FT-05`), achieving 100% test pass rate across 19 evaluation assertions. Static Joyzoning analysis reports an Entropy Score of 0.0015 and a 100% Stability Index with zero blocking violations. HEAV3NS-MICRO establishes that agent runtime substrates can be simultaneously hyper-compressed, deterministic, and safe.
