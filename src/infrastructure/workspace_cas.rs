// [LAYER: INFRASTRUCTURE]
use crate::domain::interfaces::IWorkspaceSnapshotCas;
use crate::domain::types::tick_result::{FileChangeReceipt, TurnReceipt};
use crate::infrastructure::search_tools::is_ignored_dir;
use crate::plumbing::hash_and_math::{fnv1a_32, now_ms};
use crate::plumbing::secret_redactor::{CredentialDenyScope, DeterministicSecretRedactor};
use crate::plumbing::text_tokens::{count_diff_line_stats, render_unified_diff};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_CAS_TRACKED_FILES: usize = 25_000;
const MAX_SINGLE_BLOB_BYTES: u64 = 2 * 1024 * 1024;
const MAX_STORED_SNAPSHOTS: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CasFileEntry {
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CasSnapshotManifest {
    pub id: String,
    pub label: String,
    pub turn_number: u64,
    pub timestamp_ms: u64,
    pub files: BTreeMap<String, CasFileEntry>,
}

/// Pure-Rust Content-Addressable Workspace Snapshot Store (`CAS`).
/// Supersedes external `/usr/bin/git` shadow repositories: zero git dependency, zero untracked workspace clutter,
/// path-scoped turn `/undo` with post-turn edit collision refusal (`WouldBlock`), whole-tree `/restore`,
/// universal non-Git `/diff`, and `0600` durable `receipts.jsonl` persistence!
pub struct WorkspaceSnapshotCas {
    workspace_root: PathBuf,
    cas_root: PathBuf,
    snapshots: Vec<CasSnapshotManifest>,
}

impl WorkspaceSnapshotCas {
    pub fn open(workspace_root: &Path) -> Self {
        let canonical_ws = workspace_root.canonicalize().unwrap_or_else(|_| workspace_root.to_path_buf());
        let ws_hash = format!("{:08x}", fnv1a_32(canonical_ws.to_string_lossy().as_bytes()));
        let base_state = std::env::var("HOME")
            .ok()
            .filter(|h| !h.is_empty())
            .map(|h| PathBuf::from(h).join(".heav3ns"))
            .unwrap_or_else(|| std::env::temp_dir().join("heav3ns-state"));
        let cas_root = base_state.join("workspaces").join(ws_hash);
        let _ = fs::create_dir_all(cas_root.join("blobs"));
        let _ = fs::create_dir_all(cas_root.join("manifests"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&cas_root, fs::Permissions::from_mode(0o700));
        }
        let snapshots = Self::load_existing_manifests(&cas_root);
        Self {
            workspace_root: workspace_root.to_path_buf(),
            cas_root,
            snapshots,
        }
    }

    pub fn snapshots(&self) -> &[CasSnapshotManifest] {
        &self.snapshots
    }

    pub fn earliest_snapshot_id(&self) -> Option<&str> {
        self.snapshots.first().map(|s| s.id.as_str())
    }

    pub fn latest_snapshot_id(&self) -> Option<&str> {
        self.snapshots.last().map(|s| s.id.as_str())
    }

    pub fn append_turn_receipt(&self, receipt: &TurnReceipt) {
        let path = self.cas_root.join("receipts.jsonl");
        if let Ok(line) = serde_json::to_string(receipt) {
            use std::io::Write;
            if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
                let _ = writeln!(file, "{line}");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
                }
            }
        }
    }

    pub fn load_recent_receipts(&self, limit: usize) -> Vec<TurnReceipt> {
        let path = self.cas_root.join("receipts.jsonl");
        let Ok(raw) = fs::read_to_string(path) else {
            return Vec::new();
        };
        let mut parsed: Vec<TurnReceipt> = raw
            .lines()
            .filter_map(|l| serde_json::from_str::<TurnReceipt>(l).ok())
            .collect();
        if parsed.len() > limit {
            parsed = parsed.split_off(parsed.len() - limit);
        }
        parsed
    }

    fn load_existing_manifests(cas_root: &Path) -> Vec<CasSnapshotManifest> {
        let Ok(rd) = fs::read_dir(cas_root.join("manifests")) else {
            return Vec::new();
        };
        let mut list = Vec::new();
        for entry in rd.flatten() {
            if let Ok(raw) = fs::read_to_string(entry.path()) {
                if let Ok(m) = serde_json::from_str::<CasSnapshotManifest>(&raw) {
                    list.push(m);
                }
            }
        }
        list.sort_by_key(|m| (m.timestamp_ms, m.turn_number));
        list
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    fn blob_path(&self, sha256: &str) -> PathBuf {
        self.cas_root.join("blobs").join(sha256)
    }

    fn store_blob(&self, bytes: &[u8]) -> Result<String, String> {
        let hash = Self::sha256_hex(bytes);
        let dest = self.blob_path(&hash);
        if !dest.exists() {
            fs::write(&dest, bytes).map_err(|e| format!("CAS blob write failed: {e}"))?;
        }
        Ok(hash)
    }

    fn read_blob(&self, sha256: &str) -> Option<Vec<u8>> {
        fs::read(self.blob_path(sha256)).ok()
    }

    fn scan_working_tree(&self, store_blobs: bool) -> Result<BTreeMap<String, (CasFileEntry, Option<Vec<u8>>)>, String> {
        let mut tree = BTreeMap::new();
        self.walk_dir(&self.workspace_root, 0, store_blobs, &mut tree)?;
        Ok(tree)
    }

    fn walk_dir(
        &self,
        dir: &Path,
        depth: usize,
        store_blobs: bool,
        out: &mut BTreeMap<String, (CasFileEntry, Option<Vec<u8>>)>,
    ) -> Result<(), String> {
        if depth > 14 || out.len() >= MAX_CAS_TRACKED_FILES {
            return Ok(());
        }
        let Ok(rd) = fs::read_dir(dir) else {
            return Ok(());
        };
        for entry in rd.flatten() {
            if out.len() >= MAX_CAS_TRACKED_FILES {
                break;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_ignored_dir(&name) || matches!(name.as_str(), ".build" | ".heav3ns" | ".codewhale" | ".joy-state") {
                continue;
            }
            let path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                self.walk_dir(&path, depth + 1, store_blobs, out)?;
            } else if ft.is_file() {
                let rel = path
                    .strip_prefix(&self.workspace_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if DeterministicSecretRedactor::classify_sensitive_path(&rel, CredentialDenyScope::ModelContext).is_some() {
                    continue;
                }
                let Ok(meta) = entry.metadata() else { continue };
                if meta.len() > MAX_SINGLE_BLOB_BYTES {
                    continue;
                }
                let Ok(bytes) = fs::read(&path) else { continue };
                let sha256 = if store_blobs {
                    self.store_blob(&bytes)?
                } else {
                    Self::sha256_hex(&bytes)
                };
                out.insert(
                    rel,
                    (
                        CasFileEntry {
                            sha256,
                            size: bytes.len() as u64,
                        },
                        if store_blobs { None } else { Some(bytes) },
                    ),
                );
            }
        }
        Ok(())
    }

    fn atomic_write_workspace_file(&self, rel_path: &str, bytes: &[u8]) -> Result<(), String> {
        let target = self.workspace_root.join(rel_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir failed for '{}': {e}", parent.display()))?;
        }
        let existing_perms = fs::metadata(&target).ok().map(|m| m.permissions());
        let tmp = target.with_extension(format!("heav3ns-cas-tmp-{}", now_ms()));
        fs::write(&tmp, bytes).map_err(|e| format!("temp write failed for '{rel_path}': {e}"))?;
        if let Some(perms) = existing_perms {
            let _ = fs::set_permissions(&tmp, perms);
        }
        fs::rename(&tmp, &target).map_err(|e| format!("atomic rename failed for '{rel_path}': {e}"))
    }

    fn prune_empty_parents(&self, rel_path: &str) {
        let mut cur = self.workspace_root.join(rel_path).parent().map(Path::to_path_buf);
        while let Some(dir) = cur {
            if dir == self.workspace_root || !dir.starts_with(&self.workspace_root) {
                break;
            }
            let is_empty = fs::read_dir(&dir).map(|mut r| r.next().is_none()).unwrap_or(false);
            if is_empty {
                let _ = fs::remove_dir(&dir);
                cur = dir.parent().map(Path::to_path_buf);
            } else {
                break;
            }
        }
    }

    fn find_manifest(&self, id: &str) -> Option<&CasSnapshotManifest> {
        self.snapshots.iter().find(|s| s.id == id)
    }
}

impl IWorkspaceSnapshotCas for WorkspaceSnapshotCas {
    fn capture_snapshot(&mut self, label: &str, turn_number: u64) -> Result<String, String> {
        let scanned = self.scan_working_tree(true)?;
        let files: BTreeMap<String, CasFileEntry> = scanned.into_iter().map(|(k, (v, _))| (k, v)).collect();
        let ts = now_ms();
        let tree_digest = Self::sha256_hex(serde_json::to_string(&files).unwrap_or_default().as_bytes());
        let id = format!("snap_{turn_number}_{}", &tree_digest[..8]);
        let manifest = CasSnapshotManifest {
            id: id.clone(),
            label: label.to_string(),
            turn_number,
            timestamp_ms: ts,
            files,
        };
        let manifest_path = self.cas_root.join("manifests").join(format!("{ts}_{id}.json"));
        if let Ok(serialized) = serde_json::to_string(&manifest) {
            let _ = fs::write(&manifest_path, serialized);
        }
        self.snapshots.push(manifest);
        while self.snapshots.len() > MAX_STORED_SNAPSHOTS {
            self.snapshots.remove(1); // preserve index 0 session baseline!
        }
        Ok(id)
    }

    fn diff_between(&self, base_id: &str, target_id: Option<&str>) -> Result<(Vec<FileChangeReceipt>, String), String> {
        let base = self
            .find_manifest(base_id)
            .ok_or_else(|| format!("Snapshot '{base_id}' not found"))?;

        let live_tree;
        let target_files: BTreeMap<String, (String, Vec<u8>)> = if let Some(tid) = target_id {
            let target = self
                .find_manifest(tid)
                .ok_or_else(|| format!("Target snapshot '{tid}' not found"))?;
            target
                .files
                .iter()
                .map(|(k, v)| (k.clone(), (v.sha256.clone(), self.read_blob(&v.sha256).unwrap_or_default())))
                .collect()
        } else {
            live_tree = self.scan_working_tree(false)?;
            live_tree
                .into_iter()
                .map(|(k, (entry, bytes))| (k, (entry.sha256, bytes.unwrap_or_default())))
                .collect()
        };

        let mut all_paths: BTreeSet<String> = base.files.keys().cloned().collect();
        all_paths.extend(target_files.keys().cloned());

        let mut receipts = Vec::new();
        let mut unified_chunks = Vec::new();

        for path in all_paths {
            let b_entry = base.files.get(&path);
            let t_entry = target_files.get(&path);
            match (b_entry, t_entry) {
                (Some(b), Some((t_sha, t_bytes))) if b.sha256 == *t_sha => {
                    let _ = t_bytes;
                }
                (None, Some((t_sha, t_bytes))) => {
                    let new_str = String::from_utf8_lossy(t_bytes);
                    let (added, removed) = count_diff_line_stats("", &new_str);
                    unified_chunks.push(render_unified_diff(&path, "", &new_str));
                    receipts.push(FileChangeReceipt {
                        path,
                        kind: "added".to_string(),
                        lines_added: added,
                        lines_removed: removed,
                        after_hash: t_sha[..8.min(t_sha.len())].to_string(),
                    });
                }
                (Some(b), None) => {
                    let old_bytes = self.read_blob(&b.sha256).unwrap_or_default();
                    let old_str = String::from_utf8_lossy(&old_bytes);
                    let (added, removed) = count_diff_line_stats(&old_str, "");
                    unified_chunks.push(render_unified_diff(&path, &old_str, ""));
                    receipts.push(FileChangeReceipt {
                        path,
                        kind: "deleted".to_string(),
                        lines_added: added,
                        lines_removed: removed,
                        after_hash: "00000000".to_string(),
                    });
                }
                (Some(b), Some((t_sha, t_bytes))) => {
                    let old_bytes = self.read_blob(&b.sha256).unwrap_or_default();
                    let old_str = String::from_utf8_lossy(&old_bytes);
                    let new_str = String::from_utf8_lossy(t_bytes);
                    let (added, removed) = count_diff_line_stats(&old_str, &new_str);
                    unified_chunks.push(render_unified_diff(&path, &old_str, &new_str));
                    receipts.push(FileChangeReceipt {
                        path,
                        kind: "modified".to_string(),
                        lines_added: added,
                        lines_removed: removed,
                        after_hash: t_sha[..8.min(t_sha.len())].to_string(),
                    });
                }
                (None, None) => {}
            }
        }

        Ok((receipts, unified_chunks.join("\n")))
    }

    fn undo_turn_changes(&mut self, pre_turn_id: &str, post_turn_id: &str, force: bool) -> Result<Vec<String>, String> {
        let pre = self
            .find_manifest(pre_turn_id)
            .cloned()
            .ok_or_else(|| format!("Pre-turn snapshot '{pre_turn_id}' not found"))?;
        let post = self
            .find_manifest(post_turn_id)
            .cloned()
            .ok_or_else(|| format!("Post-turn snapshot '{post_turn_id}' not found"))?;

        let mut touched_paths = BTreeSet::new();
        for p in pre.files.keys().chain(post.files.keys()) {
            let pre_sha = pre.files.get(p).map(|e| e.sha256.as_str());
            let post_sha = post.files.get(p).map(|e| e.sha256.as_str());
            if pre_sha != post_sha {
                touched_paths.insert(p.clone());
            }
        }

        if touched_paths.is_empty() {
            return Ok(Vec::new());
        }

        // Codewhale SHA-256 post-turn collision preflight (`WouldBlock`)
        if !force {
            for rel in &touched_paths {
                let disk_path = self.workspace_root.join(rel);
                match (post.files.get(rel), fs::read(&disk_path).ok()) {
                    (Some(expected), Some(actual_bytes)) => {
                        let actual_sha = Self::sha256_hex(&actual_bytes);
                        if actual_sha != expected.sha256 {
                            return Err(format!(
                                "WouldBlock: '{rel}' was modified after the turn completed. Use `/undo --force` or `/restore {pre_turn_id}` to overwrite external edits."
                            ));
                        }
                    }
                    (None, Some(_)) => {
                        return Err(format!(
                            "WouldBlock: '{rel}' was re-created externally after the turn deleted it. Use `/undo --force` to overwrite."
                        ));
                    }
                    (Some(_), None) => {
                        return Err(format!(
                            "WouldBlock: '{rel}' was deleted externally after the turn modified it. Use `/undo --force` to restore anyway."
                        ));
                    }
                    (None, None) => {}
                }
            }
        }

        let _ = self.capture_snapshot(&format!("pre-undo:{post_turn_id}"), post.turn_number);
        let mut actions = Vec::new();

        for rel in touched_paths {
            if let Some(pre_entry) = pre.files.get(&rel) {
                let bytes = self
                    .read_blob(&pre_entry.sha256)
                    .ok_or_else(|| format!("Missing CAS blob for '{rel}'"))?;
                self.atomic_write_workspace_file(&rel, &bytes)?;
                actions.push(format!("[Restored] {rel}"));
            } else {
                let disk_path = self.workspace_root.join(&rel);
                if disk_path.exists() {
                    fs::remove_file(&disk_path).map_err(|e| format!("Failed to remove '{rel}': {e}"))?;
                    self.prune_empty_parents(&rel);
                }
                actions.push(format!("[Removed] {rel}"));
            }
        }

        Ok(actions)
    }

    fn restore_full_snapshot(&mut self, snapshot_id: &str) -> Result<Vec<String>, String> {
        let target = self
            .find_manifest(snapshot_id)
            .cloned()
            .ok_or_else(|| format!("Snapshot '{snapshot_id}' not found"))?;

        let _ = self.capture_snapshot(&format!("pre-restore:{snapshot_id}"), target.turn_number);
        let live = self.scan_working_tree(false)?;
        let mut actions = Vec::new();

        for (rel, _) in &live {
            if !target.files.contains_key(rel) {
                let disk_path = self.workspace_root.join(rel);
                if disk_path.exists() {
                    let _ = fs::remove_file(&disk_path);
                    self.prune_empty_parents(rel);
                    actions.push(format!("[Removed] {rel}"));
                }
            }
        }

        for (rel, t_entry) in &target.files {
            let needs_write = live
                .get(rel)
                .map(|(l_entry, _)| l_entry.sha256 != t_entry.sha256)
                .unwrap_or(true);
            if needs_write {
                if let Some(bytes) = self.read_blob(&t_entry.sha256) {
                    self.atomic_write_workspace_file(rel, &bytes)?;
                    actions.push(format!("[Restored] {rel}"));
                }
            }
        }

        Ok(actions)
    }
}
