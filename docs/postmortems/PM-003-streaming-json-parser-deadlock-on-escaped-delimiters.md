<!-- [LAYER: UI] -->
# PM-003: Streaming JSON Parser Deadlock on Escaped Delimiters

- **Incident Date**: 2026-02-15
- **Severity**: SEV-2
- **Subsystem**: `json_repair` (`src/plumbing/json_repair.rs`)
- **Related ADR**: [ADR-0003](../adr/0003-self-healing-tool-call-parser.md)
- **Resolved By**: Escape character state machine in bracket depth counter

---

## Summary

When an LLM emitted a diff hunk containing escaped quotation marks inside a string literal (`"content": "println!(\"hello\");"`), the parser miscounted string boundaries, entered an infinite bracket-balancing loop, and timed out the agent turn.

---

## Root cause

1. **Why did the parser hang?** The bracket counting algorithm failed to exit string literal mode.
2. **Why was string mode stuck?** It treated the escaped quote (`\"`) as a regular quote terminating the string.
3. **Why did that cause a loop?** Subsequent curly braces inside Rust code were counted as JSON structural delimiters, causing invalid trailing delimiter appending.

---

## Action items

1. **Implement backslash state tracking**: Track preceding backslashes (`\`) so escaped quotes do not toggle string mode.
2. **Add unit test**: Add `test_self_healing_parser_and_streaming_dedup`.
3. **Document Playbook**: Create [PB-03: Flight Test Failures & Parser Desync](../playbooks/flight-test-failures-and-parser-desync.md).

---

## Lessons learned

Lexical tokenizers handling code generation must account for nested language escaping. Naive single-pass character matching fails when generating source code containing strings.
