<!-- [LAYER: UI] -->
# Metrics & Memory Encyclopedia

<p align="center">
  <a href="README.md">Wiki Hub</a> ·
  <a href="../papers/companion-brief.md">Companion Brief</a> ·
  <a href="../architecture/current.md">Architecture</a> ·
  <a href="../CONSTITUTION.md">Constitution</a>
</p>

Quantitative formulas, memory budgeting rules, hashing specifications, and Joyzoning stability indexing in **HEAV3NS-MICRO**.

---

## 1. Memory Budgeting & Arena Sizing

The default memory slab is initialized to:

$$\text{Capacity} = 16 \times 1024 \times 1024 \ \text{bytes} = 16{,}777{,}216 \ \text{bytes (16 MiB)}$$

### Thresholds & Alarms

| Threshold | Utilization | Action Taken |
| :--- | :--- | :--- |
| **Normal** | $0\% - 60\%$ ($0 - 9.6$ MiB) | Linear bump allocation proceeding normally. |
| **Warning** | $60\% - 80\%$ ($9.6 - 12.8$ MiB) | Automatic advisory logged; proactive compaction scheduled. |
| **Critical** | $80\% - 95\%$ ($12.8 - 15.2$ MiB) | Compaction triggered immediately across dialogue turns. |
| **Exhaustion** | $> 95\%$ ($> 15.2$ MiB) | Returns `ArenaOom`; turn aborted safely to protect host. |

---

## 2. Token Budgeting & Compaction Math

When active session tokens exceed context limits, [`CompactionEngine`](../../src/domain/compaction.rs) compresses historical turns:

$$\text{Compression Ratio} = \frac{\text{Tokens}_{\text{summary}}}{\sum_{i \in \text{compacted}} \text{Tokens}_i} \approx 0.15 - 0.25$$

A dialogue history of 8,000 tokens is compacted into a semantic memory block of approximately 1,200 to 1,600 tokens, achieving an 80% reduction in active context window consumption.

---

## 3. FNV-1a 32-Bit Hash Specification

The Fowler–Noll–Vo 32-bit hash algorithm is used for lightning-fast disk baseline fingerprinting:
- **Offset Basis**: `0x811c9dc5` ($2{,}166{,}136{,}261$)
- **FNV Prime**: `0x01000193` ($16{,}777{,}619$)

For each byte $b_i$:
$$\text{hash} \leftarrow (\text{hash} \oplus b_i) \times \text{Prime} \pmod{2^{32}}$$

Because FNV-1a processes input byte-by-byte with zero heap allocation, computing the baseline for a typical source file takes under $5 \ \mu\text{s}$.

---

## 4. Joyzoning Stability Index ($SI$)

The Joyzoning Stability Index measures architectural health across five entropy pillars:
1. **Depth**: Maximum relative import directory traversals ($\le 3$).
2. **Naming**: PGA alignment between directory path and layer header.
3. **Orphan**: Unreferenced files and disconnected modules.
4. **Coupling**: Cross-layer dependency leaks and domain I/O impurity.
5. **Complexity**: Module line count caps ($< 800$ lines, Tier 1 Optimal).

$$SI = 1.0 - \text{Entropy Score} = 100\% \quad (\text{SOVEREIGN})$$
