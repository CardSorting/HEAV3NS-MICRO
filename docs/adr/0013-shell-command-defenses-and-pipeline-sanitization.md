<!-- [LAYER: UI] -->
# ADR-0013: Shell Command Guard, Heredoc File-Write Interdiction & Terminal File-Read Anti-Starvation

- **Status**: Accepted
- **Date**: 2026-11-10
- **Deciders**: HEAV3NS Architecture Working Group
- **Technical Story**: Zenith Pass — Shell Defenses Parity (`ADR-0015` & `ADR-0016`)

---

## Status

Accepted

## Context

When coding agents interact with terminal shells via `run_command`, two prevalent anti-patterns cause corruption, deadlock, and context starvation:
1. **Heredoc File Writes (`ADR-0015`)**: Writing files via `cat << 'EOF' > file` or `tee file << EOF` bypasses two-phase VFS staging, clobbers concurrent disk baselines, skips atomic rollback journaling, and fails silently on unescaped subshells.
2. **Terminal File Dumps (`ADR-0016`)**: Dumping whole source files to stdout via `cat src/main.rs`, `head -n 50`, `xargs cat`, or `find -exec cat` pollutes the transcript with unindexed text, risks catastrophic token-budget exhaustion, and causes execution hangs when interactive commands like `cat > file` are invoked without standard input.

### Decision drivers

- Intercept heredoc file-write commands prior to shell execution and guide agents toward `replace_file_content` or `write_file`.
- Detect whole-file terminal dumps while permitting valid filter pipelines (`| rg`, `| jq`, `| wc`), stdout file copies (`cat a > b`), and follow logs (`tail -f`).
- Parse compound statements (`;`, `&&`, `||`, `\n`, `$(...)`) and pipelines with full UTF-8 preservation, without false positives on arithmetic bitshifts (`$((1 << 8))`) or quoted strings.

### Considered options

1. **Option 1**: Fragile regex matching on raw command strings (fails on multiline compound commands, bitshifts, and quoted filenames).
2. **Option 2**: Complete bash grammar parser (high complexity, dependency footprint, violates `PLUMBING` simplicity caps).
3. **Option 3 (Chosen)**: Dedicated stateless `ShellCommandGuard` in `src/plumbing/shell_guard.rs` implementing quote-aware lexical tokenization, compound statement splitting, and pipeline unwrapping under `maxComplexity: 500`.

## Decision

We introduced [`ShellCommandGuard`](../../src/plumbing/shell_guard.rs) and delegated `SafetyPolicy` checks to it:
1. **Heredoc File-Write Detection (`detect_heredoc_file_redirection`)**:
   - Locates top-level unquoted `<<` heredoc operators across multiline compound commands.
   - Catches pre-heredoc redirections (`cat > file << EOF`), piped writes (`cat << EOF | sudo tee -a file`), and closing-delimiter redirections (`EOF > file`).
   - Ignores quoted `<<`, arithmetic bitshifts (`$((1 << 8))`), here-strings (`<<<`), and interpreter heredocs (`python3 - << 'PY'`).
2. **Terminal File-Read Detection (`detect_terminal_file_read`)**:
   - Splits compound statements across operators (`;`, `&&`, `||`, `\n`, `$(...)`).
   - Tokenizes pipelines, unwraps `env`/`command`/`sudo`/full-path prefixes, and isolates file reading commands (`cat`, `head`, `tail`, `less`, `more`, `bat`, `view`, etc.).
   - Catches `xargs cat`, `find -exec cat`, `bash -c "cat ..."`, multi-pager pipes (`cat a | bat`), and interactive `cat > file` deadlocks.
   - Allows valid filter pipelines (`cat file | rg pattern`), stdout file copies (`cat a > b`), follow commands (`tail -f`), and system pseudo-files (`/proc/`, `/sys/`, `/dev/`, `/etc/*-release`).

## Consequences

### Positive

- **Filesystem Integrity**: Edits are steered toward atomic, rollback-enabled VFS tools (`write_file`, `replace_file_content`).
- **Context Starvation Prevention**: Agents are prevented from dumping massive files into terminal output.
- **Deadlock Immunity**: Interactive stdin deadlocks (`cat > file`) are caught prior to process spawning.
- **UTF-8 Safety**: Multi-byte characters and escaped paths in quoted filenames are preserved without truncation or panics.

### Negative / trade-offs

- Shell commands utilizing complex custom wrapper aliases must be invoked explicitly or run through approved VFS inspection tools.

## Verification

```bash
# 1. Verify ADR-0015 and ADR-0016 compound, pipeline, and UTF-8 defenses
cargo test core::zenith_tests::tests::test_adr_0015_compound_multiline_and_closing_heredoc_defenses
cargo test core::zenith_tests::tests::test_adr_0016_compound_wrappers_stderr_and_subshell_file_read_defenses

# 2. Verify all substrate and security tests
cargo test core::substrate_tests::tests::test_secret_redactor_and_sensitive_path_guard
```

