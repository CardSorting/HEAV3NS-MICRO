// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::{ISessionPersistence, IWorkspacePromptProbe};
use crate::domain::types::skill_types::{SkillManifest, WorkspacePromptFacts};
use crate::domain::types::vfs_types::{VfsDiskBaseline, VfsFileOverlay};
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::secret_redactor::{DeterministicSecretRedactor, SafetyPolicy};
use crate::plumbing::text_tokens::{render_unified_diff, truncate_text_to_token_budget};
use crate::plumbing::vision_and_dom::{decode_image_metadata, sanitize_trojan_unicode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

impl VfsDiskBaseline {
    pub fn capture_checked(path: &Path) -> Result<Self, String> {
        use std::io::Read;
        const MAX_VFS_BASELINE_BYTES: u64 = 16 * 1024 * 1024;
        match fs::File::open(path) {
            Ok(file) => {
                let mut buf = Vec::new();
                file.take(MAX_VFS_BASELINE_BYTES + 1).read_to_end(&mut buf)
                    .map_err(|e| format!("Failed to inspect staged target '{}': {e}", path.display()))?;
                if buf.len() as u64 > MAX_VFS_BASELINE_BYTES {
                    return Err(format!("Staged target '{}' exceeds {} MiB baseline cap", path.display(), MAX_VFS_BASELINE_BYTES / (1024 * 1024)));
                }
                Ok(Self::from_bytes(Some(&buf)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::from_bytes(None)),
            Err(e) => Err(format!("Failed to inspect staged target '{}': {e}", path.display())),
        }
    }

    pub fn capture(path: &Path) -> Self {
        Self::capture_checked(path).unwrap_or_else(|_| Self::from_bytes(None))
    }

    pub fn verify(&self, path: &Path) -> Result<(), String> {
        let current = Self::capture_checked(path)?;
        if !self.matches(&current) {
            return Err(format!(
                "Staged file changed on disk since review: {}. The staged change was preserved.",
                path.display()
            ));
        }
        Ok(())
    }
}

/// Staged Virtual File System Overlay for reviewable changesets and atomic commits.
#[derive(Debug, Clone, Default)]
pub struct SessionVfs {
    staged: BTreeMap<String, VfsFileOverlay>,
}

impl SessionVfs {
    pub const fn new() -> Self {
        Self {
            staged: BTreeMap::new(),
        }
    }

    fn normalize_key(path: &str) -> String {
        let mut out = std::path::PathBuf::new();
        for comp in Path::new(path).components() {
            if !matches!(comp, std::path::Component::CurDir) {
                out.push(comp);
            }
        }
        out.to_string_lossy().into_owned()
    }

    pub fn stage_write(
        &mut self,
        path: &str,
        content: impl Into<String>,
        baseline: Option<VfsDiskBaseline>,
    ) {
        let key = Self::normalize_key(path);
        self.staged.insert(
            key.clone(),
            VfsFileOverlay {
                path: key,
                content: content.into(),
                is_deleted: false,
                timestamp: now_ms(),
                disk_baseline: baseline,
            },
        );
    }

    pub fn stage_delete(&mut self, path: &str, baseline: Option<VfsDiskBaseline>) {
        let key = Self::normalize_key(path);
        self.staged.insert(
            key.clone(),
            VfsFileOverlay {
                path: key,
                content: String::new(),
                is_deleted: true,
                timestamp: now_ms(),
                disk_baseline: baseline,
            },
        );
    }

    pub fn get_file(&self, path: &str) -> Option<&VfsFileOverlay> {
        self.staged.get(&Self::normalize_key(path))
    }

    pub fn has_staged(&self, path: &str) -> bool {
        self.staged.contains_key(&Self::normalize_key(path))
    }

    pub fn unstage(&mut self, path: &str) -> bool {
        self.staged.remove(&Self::normalize_key(path)).is_some()
    }

    pub fn clear(&mut self) {
        self.staged.clear();
    }

    pub fn export_staged(&self) -> Vec<VfsFileOverlay> {
        self.staged.values().cloned().collect()
    }

    pub fn generate_diff(&self, path: &str) -> Option<String> {
        let key = Self::normalize_key(path);
        let overlay = self.staged.get(&key)?;
        if crate::infrastructure::search_tools::verify_model_path_access(Path::new(&key)).is_err() {
            return None;
        }
        let original = crate::infrastructure::search_tools::read_text_file_bounded(Path::new(&key), 4 * 1024 * 1024).unwrap_or_default();
        let updated = if overlay.is_deleted { "" } else { &overlay.content };
        Some(render_unified_diff(&key, &original, updated))
    }

    pub fn generate_diff_in(&self, workspace_root: &Path, path: &str) -> Option<String> {
        let key = Self::normalize_key(path);
        let overlay = self.staged.get(&key)?;
        let resolved = crate::infrastructure::search_tools::resolve_and_verify_workspace_path(workspace_root, &key).ok()?;
        let original = crate::infrastructure::search_tools::read_text_file_bounded(&resolved, 4 * 1024 * 1024).unwrap_or_default();
        let updated = if overlay.is_deleted { "" } else { &overlay.content };
        Some(render_unified_diff(&key, &original, updated))
    }

    pub fn commit_all(&mut self) -> Result<Vec<String>, String> {
        let overlays = self.export_staged();
        let mut targets = Vec::with_capacity(overlays.len());
        for ov in &overlays {
            let target = Path::new(&ov.path);
            if target.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return Err(format!("Refusing VFS commit with parent traversal '{}'", ov.path));
            }
            crate::infrastructure::search_tools::verify_model_path_access(target)?;
            if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
                if fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink()) {
                    return Err(format!("Refusing VFS commit through symlinked parent '{}'", parent.display()));
                }
            }
            if fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(format!("Refusing VFS commit through symlink '{}'", ov.path));
            }
            if let Some(ref baseline) = ov.disk_baseline {
                baseline.verify(target)?;
            }
            targets.push(target.to_path_buf());
        }
        self.commit_overlays(overlays, targets)
    }

    pub fn commit_all_in(&mut self, workspace_root: &Path) -> Result<Vec<String>, String> {
        let overlays = self.export_staged();
        let mut targets = Vec::with_capacity(overlays.len());
        for ov in &overlays {
            let resolved = crate::infrastructure::search_tools::resolve_and_verify_workspace_path(workspace_root, &ov.path)?;
            if let Some(ref baseline) = ov.disk_baseline {
                baseline.verify(&resolved)?;
            }
            targets.push(resolved);
        }
        self.commit_overlays(overlays, targets)
    }

    fn commit_overlays(&mut self, overlays: Vec<VfsFileOverlay>, targets: Vec<std::path::PathBuf>) -> Result<Vec<String>, String> {
        let mut committed = Vec::with_capacity(overlays.len());
        for (ov, resolved) in overlays.into_iter().zip(targets.into_iter()) {
            let res = if ov.is_deleted {
                match fs::remove_file(&resolved) {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(e) => Err(format!("Failed deleting '{}': {e}", ov.path)),
                }
            } else {
                crate::infrastructure::search_tools::atomic_write_file(&resolved, ov.content.as_bytes())
            };
            if let Err(e) = res {
                for done in &committed {
                    self.staged.remove(done);
                }
                return Err(e);
            }
            committed.push(ov.path);
        }
        self.staged.clear();
        Ok(committed)
    }
}

/// Workspace `SKILL.md` discovery and progressive skill activation adapter.
pub struct SkillsIngestor;

impl SkillsIngestor {
    pub fn discover(workspace_root: &Path) -> Vec<SkillManifest> {
        let mut manifests = Vec::new();
        let search_roots = [
            workspace_root.join(".agents/skills"),
            workspace_root.join(".claude/skills"),
            workspace_root.join("skills"),
        ];
        for root in &search_roots {
            if fs::symlink_metadata(root).map(|m| m.file_type().is_symlink() || !m.is_dir()).unwrap_or(true) {
                continue;
            }
            let Ok(entries) = fs::read_dir(root) else { continue };
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_symlink()).unwrap_or(true) {
                continue;
            }
            let p = entry.path();
                let skill_md = if p.is_dir() {
                    p.join("SKILL.md")
                } else if p.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
                    p.clone()
                } else {
                    continue
                };
                if fs::symlink_metadata(&skill_md).map(|m| m.file_type().is_symlink()).unwrap_or(true) {
                    continue;
                }
                if let Ok(raw) = crate::infrastructure::search_tools::read_text_file_bounded(&skill_md, 512 * 1024) {
                    let folder_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("skill");
                    manifests.push(Self::parse_skill_md(folder_name, &skill_md, &raw));
                }
            }
        }
        manifests.sort_by(|a, b| a.name.cmp(&b.name));
        manifests.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
        manifests
    }

    pub fn parse_skill_md(default_name: &str, path: &Path, raw: &str) -> SkillManifest {
        let mut name = default_name.to_string();
        let mut description = "Workspace Skill".to_string();
        let normalized_raw = if raw.trim_start().starts_with("<!--") {
            raw.split_once("-->")
                .map(|(_, rest)| rest.trim_start())
                .unwrap_or(raw)
        } else {
            raw
        };
        let mut body = normalized_raw.trim().to_string();
        if let Some(after_open) = normalized_raw
            .strip_prefix("---\n")
            .or_else(|| normalized_raw.strip_prefix("---\r\n"))
        {
            if let Some(end_idx) = after_open.find("\n---") {
                let fm = &after_open[..end_idx];
                body = after_open[end_idx + 4..].trim().to_string();
                for line in fm.lines() {
                    if let Some(v) = line.strip_prefix("name:") {
                        let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
                        if !v.is_empty() {
                            name = v.to_string();
                        }
                    } else if let Some(v) = line.strip_prefix("description:") {
                        let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
                        if !v.is_empty() {
                            description = v.to_string();
                        }
                    }
                }
            }
        }
        SkillManifest {
            name,
            description,
            path: path.to_string_lossy().into_owned(),
            instructions: body,
        }
    }

    pub fn format_skills_context(manifests: &[SkillManifest]) -> Option<String> {
        if manifests.is_empty() {
            return None;
        }
        let items: Vec<serde_json::Value> = manifests
            .iter()
            .take(64)
            .map(|m| serde_json::json!({ "name": m.name, "description": m.description }))
            .collect();
        Some(serde_json::json!({ "availableSkills": items }).to_string())
    }

    /// Scans `skills/`, `.agents/skills/`, `.claude/skills/`, and `souls/` drop vaults for multi-format assets.
    pub fn read_drop_vault_files(workspace_root: &Path) -> Vec<(String, String)> {
        let mut files = Vec::new();
        let vaults = [
            workspace_root.join("skills"),
            workspace_root.join(".agents/skills"),
            workspace_root.join(".claude/skills"),
            workspace_root.join("souls"),
        ];
        for vault in &vaults {
            if fs::symlink_metadata(vault).map(|m| m.file_type().is_symlink() || !m.is_dir()).unwrap_or(true) {
                continue;
            }
            let Ok(entries) = fs::read_dir(vault) else { continue };
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_symlink()).unwrap_or(true) {
                continue;
            }
            let p = entry.path();
                if p.is_dir() {
                    let skill_md = p.join("SKILL.md");
                    if fs::symlink_metadata(&skill_md).map(|m| m.file_type().is_symlink()).unwrap_or(true) {
                    continue;
                }
                if let Ok(raw) = crate::infrastructure::search_tools::read_text_file_bounded(&skill_md, 512 * 1024) {
                        let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("skill.md").to_string();
                        files.push((fname, raw));
                    }
                } else if p.is_file() {
                    let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    let supported = [".md", ".json", ".xml", ".yaml", ".yml", ".txt", ".prompt"]
                        .iter()
                        .any(|ext| fname.to_ascii_lowercase().ends_with(ext));
                    if supported {
                        if let Ok(raw) = crate::infrastructure::search_tools::read_text_file_bounded(&p, 512 * 1024) {
                            files.push((fname, raw));
                        }
                    }
                }
            }
        }
        files
    }

    pub fn inspect_workspace_image(workspace_root: &Path, rel_path: &str) -> Result<Value, String> {
        let candidate = crate::infrastructure::search_tools::resolve_and_verify_workspace_path(workspace_root, rel_path)?;
        let meta_fs = fs::metadata(&candidate).map_err(|e| format!("Failed to stat image '{}': {e}", candidate.display()))?;
        if meta_fs.len() > 32 * 1024 * 1024 {
            return Err(format!("Image '{}' exceeds 32 MiB inspection cap ({} bytes)", candidate.display(), meta_fs.len()));
        }
        let bytes = fs::read(&candidate).map_err(|e| format!("Failed to read image '{}': {e}", candidate.display()))?;
        let mut meta = decode_image_metadata(&bytes)?;
        meta["path"] = json!(rel_path);
        Ok(meta)
    }
}

/// Computes a fast deterministic SHA-256 fingerprint of workspace source files and manifests.
pub fn workspace_revision(workspace_root: &Path) -> String {
    let mut files: Vec<(String, String)> = Vec::new();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        if out.len() >= 2_000 {
            return;
        }
        let Ok(rd) = fs::read_dir(dir) else { return };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.')
                || matches!(
                    name.as_str(),
                    "node_modules" | "target" | "dist" | "build" | "vendor" | "coverage"
                )
            {
                continue;
            }
            if entry.file_type().map(|t| t.is_symlink()).unwrap_or(true) {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if p.is_file() {
                if entry.metadata().map(|m| m.len() <= 512 * 1024).unwrap_or(false) {
                    if let Ok(bytes) = fs::read(&p) {
                        if bytes.len() <= 512 * 1024 {
                            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().into_owned();
                            let mut h = Sha256::new();
                            h.update(&bytes);
                            out.push((rel, format!("{:x}", h.finalize())));
                        }
                    }
                }
            }
        }
    }
    walk(workspace_root, workspace_root, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut root_hasher = Sha256::new();
    for (rel, digest) in files {
        root_hasher.update(rel.as_bytes());
        root_hasher.update(b":");
        root_hasher.update(digest.as_bytes());
        root_hasher.update(b"\n");
    }
    format!("{:x}", root_hasher.finalize())[..16].to_string()
}

/// Filesystem adapter implementing [`ISessionPersistence`].
#[derive(Debug, Clone, Default)]
pub struct FileSessionPersistence;

impl ISessionPersistence for FileSessionPersistence {
    fn save_jsonl(&self, path: &Path, payload: &str) -> Result<bool, String> {
        if fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            return Err("Refusing symlink session persistence target".to_string());
        }
        crate::infrastructure::search_tools::atomic_write_file(path, payload.as_bytes())?;
        Ok(true)
    }

    fn load_jsonl(&self, path: &Path) -> Result<String, String> {
        use std::io::Read;
        const MAX_SESSION_JSONL_BYTES: u64 = 32 * 1024 * 1024;
        if fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            return Err("Refusing symlink session persistence source".to_string());
        }
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        file.take(MAX_SESSION_JSONL_BYTES + 1).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        if buf.len() as u64 > MAX_SESSION_JSONL_BYTES {
            return Err("Session persistence file exceeds 32 MiB cap".to_string());
        }
        String::from_utf8(buf).map_err(|e| e.to_string())
    }
}

/// Fast, memoized workspace manifest, Joyzoning topology, and governance rule probe implementing [`IWorkspacePromptProbe`].
#[derive(Debug, Clone, Default)]
pub struct WorkspacePromptProbe;

impl WorkspacePromptProbe {
    pub fn probe_cached(workspace_root: &Path) -> WorkspacePromptFacts {
        static CACHE: OnceLock<Mutex<BTreeMap<String, (u64, WorkspacePromptFacts)>>> = OnceLock::new();
        let stamp = Self::metadata_stamp(workspace_root);
        let key = workspace_root.to_string_lossy().into_owned();
        let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
        if let Ok(guard) = cache.lock() {
            if let Some((cached_stamp, facts)) = guard.get(&key) {
                if *cached_stamp == stamp {
                    return facts.clone();
                }
            }
        }
        let fresh = Self.probe_workspace(workspace_root);
        if let Ok(mut guard) = cache.lock() {
            if guard.len() >= 64 {
                guard.clear();
            }
            guard.insert(key, (stamp, fresh.clone()));
        }
        fresh
    }

    fn metadata_stamp(workspace_root: &Path) -> u64 {
        let watched = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "Package.swift",
            "native/Package.swift",
            "go.mod",
            "tsconfig.json",
            "Makefile",
            "joyzoning.json",
            "stability.config.json",
            "SOUL.md",
            ".cursorrules",
            "CLAUDE.md",
            ".clinerules",
            ".windsurfrules",
        ];
        let mut acc = 1469598103934665603u64;
        for rel in &watched {
            acc = acc.wrapping_mul(1099511628211);
            if let Ok(meta) = fs::metadata(workspace_root.join(rel)) {
                acc ^= meta.len().wrapping_add(1);
                if let Ok(modified) = meta.modified() {
                    if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                        acc ^= dur.as_nanos() as u64;
                    }
                }
            }
        }
        acc
    }

    fn read_sanitized_governance(workspace_root: &Path) -> (Option<String>, Vec<String>, Vec<String>) {
        let candidates = ["SOUL.md", ".cursorrules", "CLAUDE.md", ".clinerules", ".windsurfrules"];
        let mut collected = Vec::new();
        let mut sources = Vec::new();
        let mut warnings = Vec::new();
        for name in &candidates {
            let p = workspace_root.join(name);
            if fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(true) {
                continue;
            }
            let Ok(raw) = crate::infrastructure::search_tools::read_text_file_bounded(&p, 256 * 1024) else { continue };
            let clean = sanitize_trojan_unicode(&raw);
            let threats = SafetyPolicy::scan_for_threats(&clean, "context");
            if !threats.is_empty() {
                warnings.push(format!("{name} (quarantined: {})", threats.join(", ")));
                continue;
            }
            let redacted = DeterministicSecretRedactor::redact(&clean);
            let filtered: Vec<&str> = redacted
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with("<!--"))
                .take(18)
                .collect();
            if !filtered.is_empty() {
                sources.push((*name).to_string());
                collected.push(format!("[{name}]\n{}", filtered.join("\n")));
            }
        }
        let digest = if collected.is_empty() {
            None
        } else {
            Some(truncate_text_to_token_budget(&collected.join("\n"), 240))
        };
        (digest, sources, warnings)
    }
}

impl IWorkspacePromptProbe for WorkspacePromptProbe {
    fn probe_workspace(&self, workspace_root: &Path) -> WorkspacePromptFacts {
        let manifest_candidates = [
            "Cargo.toml",
            "Package.swift",
            "native/Package.swift",
            "package.json",
            "pyproject.toml",
            "go.mod",
            "tsconfig.json",
            "Makefile",
        ];
        let manifests: Vec<String> = manifest_candidates
            .iter()
            .filter(|rel| workspace_root.join(rel).is_file())
            .map(|rel| (*rel).to_string())
            .collect();

        let joyzoning_detected = workspace_root.join("joyzoning.json").is_file()
            || workspace_root.join("stability.config.json").is_file()
            || (workspace_root.join("src/domain").is_dir() && workspace_root.join("src/core").is_dir());

        let shell = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "/bin/sh".to_string());

        let (governance_digest, governance_sources, governance_warnings) =
            Self::read_sanitized_governance(workspace_root);

        WorkspacePromptFacts {
            manifests,
            joyzoning_detected,
            os: std::env::consts::OS.to_string(),
            shell,
            governance_digest,
            governance_sources,
            governance_warnings,
        }
    }
}
