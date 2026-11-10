<!-- [LAYER: UI] -->
# Rust Substrate Architecture

<p align="center">
  <a href="README.md">Wiki Hub</a> ·
  <a href="mental-models.md">Mental Models</a> ·
  <a href="../architecture/current.md">Architecture</a> ·
  <a href="../papers/whitepaper.md">Whitepaper</a> ·
  <a href="../adr/0010-native-desktop-integration-and-rpc-protocol.md">ADR-0010</a>
</p>

A deep technical exploration of memory layout, pointer arithmetic, zero-allocation parsing, desktop IPC bridges, and concurrency in **HEAV3NS-MICRO**.

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

### Pure RFC 4648 Base64 Encoder
In [`src/plumbing/text_tokens.rs`](../../src/plumbing/text_tokens.rs), a zero-dependency Base64 encoder converts binary assets (PDFs, images, data tables) into ASCII streams for JSON-RPC transport:
```rust
pub fn encode_base64(bytes: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    // 3-byte -> 4-character bit shifting with '=' padding
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

## 4. Shell Command Guard & Pipeline Parser (`src/plumbing/shell_guard.rs`)

Located in [`src/plumbing/shell_guard.rs`](../../src/plumbing/shell_guard.rs), [`ShellCommandGuard`](../../src/plumbing/shell_guard.rs) provides quote-, escape-, and UTF-8-aware defensive parsing for `run_command`:
- **Top-Level Heredoc Defenses (`ADR-0015`)**: Detects multiline compound heredocs, pre-heredoc redirections, `sudo tee`, and closing-delimiter redirects (`EOF > file`), while ignoring quoted `<<`, bitshifts (`$((1 << 8))`), and interpreter stdin (`python3 - << 'PY'`).
- **Terminal File Dump Defenses (`ADR-0016`)**: Tokenizes pipelines and compound statements (`;`, `&&`, `||`, `\n`, `$(...)`), unwraps `env`/`command`/`sudo`/full paths, distinguishes `2>/dev/null` from stdout redirects, and intercepts `xargs cat`, `find -exec head`, and interactive `cat > file` deadlocks without false positives on filter pipelines (`| rg`, `| jq`, `| wc`) or `/proc/`, `/sys/`, `/dev/` reads.

---

## 5. KV-Cache Deterministic Prompt Architecture (`src/domain/prompt_spec.rs`)

Located in [`src/domain/prompt_spec.rs`](../../src/domain/prompt_spec.rs), [`SystemPromptSpec`](../../src/domain/prompt_spec.rs) maximizes provider prompt-cache hit rates:
- **Static Prefix Split**: The constitutional mandate, tool rules, untrusted data fences, and authority directives are rendered into an immutable `static_prefix` (~400 tokens) with an 8-character FNV-1a hex fingerprint (`cache_prefix_fingerprint`), ensuring 100% byte identity across distinct workspaces.
- **Dynamic Suffix**: Workspace environment details, Joyzoning architecture summaries, active `SoulProfile` axioms, and sanitized governance rules (`SOUL.md`, `.cursorrules`) are appended dynamically in `dynamic_suffix`.
- **Model Family Optimization**: Provides tailored execution hints for OpenAI reasoning, Gemini, Anthropic, and compact local models without inflating prompt token budgets.

---

## 6. Standard I/O Inter-Process Communication Engine

Located in [`src/ui/stdio_worker.rs`](../../src/ui/stdio_worker.rs) and [`src/ui/rpc_dispatch.rs`](../../src/ui/rpc_dispatch.rs):
- **Stream Isolation**: JSON-RPC control and data frames flow across standard output (`stdout`), while diagnostic logging, metrics, and panic traces flow across standard error (`stderr`).
- **Control Frame Invariant**: All outbound frames incorporate a `"v": 1` version token to guarantee backward-compatible protocol negotiation, including `"prompt-inspect-v1"` capability exchange.
- **Asynchronous Task Disconnect Tolerance**: In-flight prompts survive client disconnects; unconfirmed submissions are checked via `task.lookup` without duplicate tool execution.

---

## 7. Multi-State Snapshot Topology (`GameStateSnapshot`)

The substrate maintains atomic multi-state snapshots across four independent subsystems:
1. **Transcript History**: User prompts, assistant messages, and tool activity records.
2. **Conversation Memory**: Key-value facts and project preferences (`MemoryStore`).
3. **Transactional VFS**: Staged disk overlays and disk baseline hashes (`SessionVfs`).
4. **Arena Slab**: Bump pointer watermarks in the 16-MiB contiguous buffer.

Invoking `AgentEngine::rewind_to_snapshot` restores all four dimensions simultaneously, guaranteeing that memory, transcript, and file edits never diverge.

---

## 8. Architectural Boundaries (Joyzoning Isolation)

The Rust module hierarchy strictly enforces Joyzoning across 70 modules:
- `src/domain/interfaces/mod.rs` acts as the port interface boundary (`39` formal `I<Name>` traits).
- `src/infrastructure/` modules never import from `src/ui/`.
- `src/core/` interacts with persistence, VFS, and HTTP solely through traits.
- All source files remain under **Tier 1 Optimal (< 800 lines, `SPI-203`)**.
