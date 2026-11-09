// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::ISessionPersistence;
use crate::domain::types::skill_types::SkillManifest;
use crate::domain::types::vfs_types::{VfsDiskBaseline, VfsFileOverlay};
use crate::plumbing::hash_and_math::now_ms;
use crate::plumbing::text_tokens::render_unified_diff;
use crate::plumbing::vision_and_dom::decode_image_metadata;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

impl VfsDiskBaseline {
    pub fn capture(path: &Path) -> Self {
        if let Ok(bytes) = fs::read(path) {
            Self::from_bytes(Some(&bytes))
        } else {
            Self::from_bytes(None)
        }
    }

    pub fn verify(&self, path: &Path) -> Result<(), String> {
        let current = Self::capture(path);
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
        Path::new(path).to_string_lossy().into_owned()
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
        let original = fs::read_to_string(&key).unwrap_or_default();
        let updated = if overlay.is_deleted { "" } else { &overlay.content };
        Some(render_unified_diff(&key, &original, updated))
    }

    pub fn commit_all(&mut self) -> Result<Vec<String>, String> {
        let overlays = self.export_staged();
        for ov in &overlays {
            if let Some(ref baseline) = ov.disk_baseline {
                baseline.verify(Path::new(&ov.path))?;
            }
        }
        let mut committed = Vec::with_capacity(overlays.len());
        for ov in overlays {
            let target = Path::new(&ov.path);
            if ov.is_deleted {
                let _ = fs::remove_file(target);
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create dir {}: {e}", parent.display()))?;
                }
                fs::write(target, &ov.content)
                    .map_err(|e| format!("Failed to write {}: {e}", target.display()))?;
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
            if !root.is_dir() {
                continue;
            }
            let Ok(entries) = fs::read_dir(root) else { continue };
            for entry in entries.flatten() {
                let p = entry.path();
                let skill_md = if p.is_dir() {
                    p.join("SKILL.md")
                } else if p.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
                    p.clone()
                } else {
                    continue
                };
                if let Ok(raw) = fs::read_to_string(&skill_md) {
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
            if !vault.is_dir() {
                continue;
            }
            let Ok(entries) = fs::read_dir(vault) else { continue };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let skill_md = p.join("SKILL.md");
                    if let Ok(raw) = fs::read_to_string(&skill_md) {
                        let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("skill.md").to_string();
                        files.push((fname, raw));
                    }
                } else if p.is_file() {
                    let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    let supported = [".md", ".json", ".xml", ".yaml", ".yml", ".txt", ".prompt"]
                        .iter()
                        .any(|ext| fname.to_ascii_lowercase().ends_with(ext));
                    if supported {
                        if let Ok(raw) = fs::read_to_string(&p) {
                            files.push((fname, raw));
                        }
                    }
                }
            }
        }
        files
    }

    pub fn inspect_workspace_image(workspace_root: &Path, rel_path: &str) -> Result<Value, String> {
        let candidate = if Path::new(rel_path).is_absolute() {
            Path::new(rel_path).to_path_buf()
        } else {
            workspace_root.join(rel_path)
        };
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
            let p = entry.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if p.is_file() {
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, payload.as_bytes()).map_err(|e| e.to_string())?;
        Ok(true)
    }

    fn load_jsonl(&self, path: &Path) -> Result<String, String> {
        fs::read_to_string(path).map_err(|e| e.to_string())
    }
}
