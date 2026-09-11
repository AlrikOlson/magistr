use std::path::Path;

use serde::{Deserialize, Serialize};

/// Network-filesystem detection with explicit provenance and fallback limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkDetection {
    pub network_fs: bool,
    pub method: String,
    pub fstype: Option<String>,
}

impl NetworkDetection {
    /// `override_value` is the caller-supplied WORKBENCH_NETWORK_FS value.
    /// Only "1" and "0" override detection. macOS mount types currently fall
    /// back to local; operators can force network mode with the override.
    #[must_use]
    pub fn detect(root: &Path, override_value: Option<&str>) -> Self {
        match override_value {
            Some("1") => return Self::new(true, "env-override", None),
            Some("0") => return Self::new(false, "env-override", None),
            _ => {}
        }
        let path = root.as_os_str().to_string_lossy();
        if path.starts_with("//") || path.starts_with("\\\\") {
            return Self::new(true, "unc-path", None);
        }
        std::fs::read_to_string("/proc/mounts")
            .ok()
            .and_then(|mounts| Self::from_mounts(root, &mounts))
            .unwrap_or_else(|| Self::new(false, "default-local", None))
    }

    #[must_use]
    pub fn journal_mode(&self) -> &'static str {
        if self.network_fs { "TRUNCATE" } else { "WAL" }
    }

    fn new(network_fs: bool, method: &str, fstype: Option<String>) -> Self {
        Self {
            network_fs,
            method: method.into(),
            fstype,
        }
    }

    fn from_mounts(root: &Path, mounts: &str) -> Option<Self> {
        mounts
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                fields.next()?;
                let mount = fields
                    .next()?
                    .replace("\\040", " ")
                    .replace("\\011", "\t")
                    .replace("\\012", "\n")
                    .replace("\\134", "\\");
                let fstype = fields.next()?;
                root.starts_with(&mount).then_some((mount.len(), fstype))
            })
            .max_by_key(|(length, _)| *length)
            .map(|(_, fstype)| {
                Self::new(
                    matches!(
                        fstype,
                        "nfs"
                            | "nfs4"
                            | "cifs"
                            | "smbfs"
                            | "smb3"
                            | "sshfs"
                            | "9p"
                            | "afs"
                            | "ncpfs"
                    ) || fstype.starts_with("fuse."),
                    "proc-mounts",
                    Some(fstype.into()),
                )
            })
    }
}

#[cfg(test)]
#[path = "network_detection.tests.rs"]
mod tests;
