<!-- [LAYER: UI] -->
# Metrics & Memory Encyclopedia

<p align="center">
  <a href="README.md">Wiki Hub</a> ·
  <a href="../papers/ascendant-intelligence.md">Ascendant Intelligence</a> ·
  <a href="../papers/companion-brief.md">Companion Brief</a> ·
  <a href="../architecture/current.md">Architecture</a> ·
  <a href="../CONSTITUTION.md">Constitution</a>
</p>

Quantitative formulas, memory budgeting rules, spatial geometry equations, Bayesian confidence calibration, and Joyzoning stability indexing in **HEAV3NS-MICRO**.

---

## 1. Memory Budgeting & Arena Sizing (`REWIND`)

The default memory slab in [`src/domain/arena.rs`](../../src/domain/arena.rs) is initialized to:

$$\text{Capacity} = 16 \times 1024 \times 1024 \ \text{bytes} = 16{,}777{,}216 \ \text{bytes (16 MiB)}$$

| Threshold | Utilization | Action Taken |
| :--- | :--- | :--- |
| **Normal** | $0\% - 60\%$ ($0 - 9.6$ MiB) | Linear bump allocation proceeding normally. |
| **Warning** | $60\% - 80\%$ ($9.6 - 12.8$ MiB) | Automatic advisory logged; proactive compaction scheduled. |
| **Critical** | $80\% - 95\%$ ($12.8 - 15.2$ MiB) | Compaction triggered immediately across dialogue turns. |
| **Exhaustion** | $> 95\%$ ($> 15.2$ MiB) | Returns `ArenaOom`; turn aborted safely to protect host. |

---

## 2. Spatial Grounding & 2D Sweep-Line Occlusion Math (`COMPUTER USE`)

In [`src/plumbing/spatial_geometry.rs`](../../src/plumbing/spatial_geometry.rs) and [`src/domain/spatial_mediator.rs`](../../src/domain/spatial_mediator.rs):

### 15% Inset Safe Point
For a bounding box $R = [x, y, w, h]$ with margin $m = 0.15$:

$$P_{\text{safe}} = \left(\text{clamp}\left(x + \frac{w}{2}, \ x + m w, \ x + (1 - m)w\right), \ \text{clamp}\left(y + \frac{h}{2}, \ y + m h, \ y + (1 - m)h\right)\right)$$

### Exact Multi-Rectangle Occlusion Union
Given intersections $\{I_1, \dots, I_k\}$ where $I_j = R_{\text{target}} \cap R_{\text{occluder}, j}$, `calculate_rectangles_union_area` sorts unique $X$-coordinates $[x_0, x_1, \dots, x_p]$ and sweeps vertical slices $\Delta x_i = x_{i+1} - x_i$, merging overlapping $Y$-intervals $[y_{\text{start}}, y_{\text{end}}]$ in each slice:

$$\text{Area}\left(\bigcup_{j=1}^k I_j\right) = \sum_{i=0}^{p-1} \Delta x_i \cdot \text{MergedLength}_Y(x_i, x_{i+1})$$

---

## 3. `BROCCOLI` Laplace-Smoothed Confidence Calibration

In [`src/domain/types/learned_knowledge_types.rs`](../../src/domain/types/learned_knowledge_types.rs), each `LearnedInteractionPattern` updates its confidence after every verified execution outcome ($S$ successes, $F$ failures):

$$C(S, F) = \text{clamp}\left(\frac{S + 1}{S + F + 2}, \ 0.10, \ 0.99\right)$$

---

## 4. FNV-1a 32-Bit Hash Specification (`REWIND` & `SPIDER`)

Used for `VfsDiskBaseline` fingerprinting and `EnvironmentalObservation` frame hashing (`frame_hash`):
- **Offset Basis**: `0x811c9dc5` ($2{,}166{,}136{,}261$)
- **FNV Prime**: `0x01000193` ($16{,}777{,}619$)

$$\text{hash} \leftarrow (\text{hash} \oplus b_i) \times \text{Prime} \pmod{2^{32}}$$

---

## 5. Joyzoning Stability Index ($SI$) & Structural Entropy ($E_S$)

Across all `280+` scanned files (`329` local dependency edges):
1. **Depth**: `0` (all relative imports $\le 3$ levels).
2. **Naming**: `0` (100% PGA alignment between directory path and `[LAYER]` header).
3. **Orphan**: `0` (zero unreferenced modules).
4. **Coupling**: `0` (zero cross-zone leaks, zero domain I/O, 39 formal `I<Name>` traits).
5. **Complexity**: `0.0189` ($E_S = 0.0038$, all files `< 800` lines Tier 1 Optimal).

$$SI = 100\% \quad (\text{Grade: A+ — SOVEREIGN})$$
