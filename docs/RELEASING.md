<!-- [LAYER: UI] -->
# Release Process & Versioning

<p align="center">
  <a href="README.md">Docs Hub</a> ·
  <a href="MAINTAINER.md">Maintainer Guide</a> ·
  <a href="../CHANGELOG.md">Changelog</a>
</p>

Release procedures, semantic versioning rules, and stability invariants for **HEAV3NS-MICRO**.

---

## 1. Semantic Versioning Policy

HEAV3NS-MICRO strictly adheres to **Semantic Versioning (SemVer 2.0.0)** (`MAJOR.MINOR.PATCH`):

- **MAJOR**: Breaking changes to the public Rust API traits (`src/domain/interfaces/mod.rs`), breaking modifications to the stdio JSONL wire protocol, or major architectural zone realignments.
- **MINOR**: Addition of new built-in tools (`src/core/tool_specs.rs`), new slash commands, backward-compatible runtime capabilities, or performance enhancements.
- **PATCH**: Bug fixes, documentation updates, diagnostic improvements, or security hardening.

---

## 2. Release Steps

Follow this sequential checklist when publishing a new version:

### Step 1: Branch Preparation
Create a release branch from `main`:
```bash
git checkout -b release/v0.1.0
```

### Step 2: Bump Versions
Update the version string across:
- `Cargo.toml` (`[package] version = "0.1.0"`)
- `package.json` (`"version": "0.1.0"`)
- `docs/README.md` and `docs/papers/companion-brief.md`

### Step 3: Run Full Verification Suite
Execute all automated checks:
```bash
# Verify unit tests and flight benchmarks
cargo test
cargo run -- --flight

# Verify Joyzoning architecture and documentation
npm run joy-check
npm run docs:check
```

### Step 4: Update CHANGELOG
Document changes in [`CHANGELOG.md`](../CHANGELOG.md) under the release version header, referencing relevant ADRs, Playbooks, and Postmortems.

### Step 5: Commit & Tag
```bash
git commit -am "chore: release v0.1.0"
git tag -a v0.1.0 -m "Release HEAV3NS-MICRO v0.1.0"
git push origin release/v0.1.0 --tags
```

### Step 6: Publish Crate
```bash
cargo publish
```
