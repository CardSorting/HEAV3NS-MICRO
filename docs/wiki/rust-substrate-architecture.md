<!-- [LAYER: UI] -->
# Rust Substrate Architecture

<p align="center">
  <a href="README.md">Wiki Hub</a> ·
  <a href="mental-models.md">Mental Models</a> ·
  <a href="../architecture/current.md">Architecture</a> ·
  <a href="../papers/whitepaper.md">Whitepaper</a>
</p>

A deep technical exploration of memory layout, pointer arithmetic, zero-allocation parsing, and concurrency in **HEAV3NS-MICRO**.

---

## 1. Contiguous Slab Memory Layout

The core memory engine resides in [`src/domain/arena.rs`](../../src/domain/arena.rs). The structure consists of:

```rust
pub struct ArenaAllocator {
    buffer: Vec<u8>,
    capacity: usize,
    offset: usize,
    watermark: usize,
}
```

### Pointer Arithmetic & 8-Byte Alignment
All allocations must be aligned to 8-byte boundaries to prevent unaligned memory faults on modern 64-bit architectures (x86_64, aarch64):

```rust
let aligned_offset = (self.offset + 7) & !7;
let next_offset = aligned_offset + size;
if next_offset > self.capacity {
    return Err(ArenaError::OutOfMemory);
}
self.offset = next_offset;
```

### Constant-Time Watermark Rewind
Turn checkpoints record the current allocation `offset`. At the end of a dialogue turn:
```rust
pub fn rewind_to_checkpoint(&mut self, checkpoint: usize) {
    if checkpoint <= self.offset {
        self.offset = checkpoint;
    }
}
```
Because the underlying memory vector is pre-allocated at startup, rewinding involves only an integer assignment—requiring $O(1)$ constant time and zero system calls.

---

## 2. Zero-Allocation Token Counters & Math Primitives

In [`src/plumbing/hash_and_math.rs`](../../src/plumbing/hash_and_math.rs) and [`src/plumbing/text_tokens.rs`](../../src/plumbing/text_tokens.rs):

### FNV-1a 32-Bit Hash
Used for ultra-fast VFS baseline file hashing:
```rust
pub fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for &byte in bytes {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}
```

### Token Estimation Formalism
Rather than running full BPE tokenizers during rapid streaming checks, the runtime applies a tuned lexical ratio:
- Average 3.8 characters per token for English prose.
- Average 3.2 characters per token for source code and unified diffs.

---

## 3. Streaming Self-Healing Parser Architecture

Located in [`src/plumbing/json_repair.rs`](../../src/plumbing/json_repair.rs):
- **Bracket Depth Counter**: Scans bytes sequentially, ignoring brackets enclosed within string literals (`"`).
- **Escape Character State Machine**: Tracks whether a preceding byte was a backslash (`\`) to correctly identify escaped quotes (`\"`).
- **Trailing Delimiter Append**: Upon reaching the end of the buffer, appends matching `}` or `]` tokens in reverse order of nesting depth.

---

## 4. Architectural Boundaries (Joyzoning Isolation)

The Rust module hierarchy strictly enforces Joyzoning:
- `src/domain/interfaces/mod.rs` acts as the port interface boundary.
- `src/infrastructure/` modules never import from `src/ui/`.
- `src/core/` interacts with persistence, VFS, and HTTP solely through traits.
