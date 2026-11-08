// [LAYER: DOMAIN]
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub type StagedOverlayList = Vec<VfsFileOverlay>;

/// Abstract contract for virtual filesystem baseline verification.
pub trait IDiskBaseline {
    fn is_unchanged(&self, current: &VfsDiskBaseline) -> bool;
}

/// Disk state required to reject stale staged VFS writes at commit time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VfsDiskBaseline {
    pub exists: bool,
    pub size: u64,
    pub content_sha256: Option<String>,
}

impl VfsDiskBaseline {
    pub fn from_bytes(bytes: Option<&[u8]>) -> Self {
        if let Some(data) = bytes {
            let mut hasher = Sha256::new();
            hasher.update(data);
            Self {
                exists: true,
                size: data.len() as u64,
                content_sha256: Some(format!("{:x}", hasher.finalize())),
            }
        } else {
            Self {
                exists: false,
                size: 0,
                content_sha256: None,
            }
        }
    }

    pub fn matches(&self, current: &Self) -> bool {
        self.exists == current.exists && self.content_sha256 == current.content_sha256
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsFileOverlay {
    pub path: String,
    pub content: String,
    #[serde(rename = "isDeleted")]
    pub is_deleted: bool,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none", rename = "diskBaseline")]
    pub disk_baseline: Option<VfsDiskBaseline>,
}
