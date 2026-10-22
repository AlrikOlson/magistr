use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ConfigLayer, ConfigLayerDigest};
use crate::snapshot::frame;

/// The applicable external configuration as observed: every layer that
/// resolves outside the workspace tree (the organization policy and the
/// user layer — the workspace's own `.magistr/workspace.toml` is a source input of
/// the snapshot, not an external layer), each by presence and content
/// digest, in precedence order. A consumer records the combined
/// [`digest`](Self::digest) beside its source revision so that a policy
/// edited outside the tree moves the evidence even when every selected
/// byte is unchanged.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConfigDigest {
    pub layers: Vec<ConfigLayerDigest>,
}

impl ConfigDigest {
    /// Observe the layers at `paths`, lock-free. A missing file is
    /// recorded as absent; a path that exists but cannot be read (a
    /// directory, a permission refusal) refuses the observation rather
    /// than recording a guess.
    pub fn observe(paths: &[(ConfigLayer, PathBuf)]) -> io::Result<Self> {
        let mut layers = Vec::with_capacity(paths.len());
        for (layer, path) in paths {
            let observed = match std::fs::read(path) {
                Ok(bytes) => ConfigLayerDigest {
                    layer: *layer,
                    present: true,
                    digest: hex::encode(Sha256::digest(&bytes)),
                },
                Err(error) if error.kind() == io::ErrorKind::NotFound => ConfigLayerDigest {
                    layer: *layer,
                    present: false,
                    digest: String::new(),
                },
                Err(error) => {
                    return Err(io::Error::new(
                        error.kind(),
                        format!(
                            "read the {layer} configuration layer {}: {error}",
                            path.display()
                        ),
                    ));
                }
            };
            layers.push(observed);
        }
        Ok(Self { layers })
    }

    /// SHA-256 over every layer's name, presence and digest, each
    /// length-framed, in order. Content only: no path, no clock.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut digest = Sha256::new();
        frame(&mut digest, &(self.layers.len() as u64).to_le_bytes());
        for layer in &self.layers {
            frame(&mut digest, layer.layer.as_str().as_bytes());
            frame(&mut digest, &[u8::from(layer.present)]);
            frame(&mut digest, layer.digest.as_bytes());
        }
        hex::encode(digest.finalize())
    }

    /// The layers that differ between this observation and a later one,
    /// in precedence order; a layer only one side observed counts.
    #[must_use]
    pub fn changes_to(&self, later: &Self) -> Vec<ConfigLayer> {
        let mut changed: Vec<ConfigLayer> = self
            .layers
            .iter()
            .filter(|before| {
                later
                    .layers
                    .iter()
                    .find(|after| after.layer == before.layer)
                    != Some(*before)
            })
            .map(|before| before.layer)
            .collect();
        for after in &later.layers {
            if !self.layers.iter().any(|before| before.layer == after.layer)
                && !changed.contains(&after.layer)
            {
                changed.push(after.layer);
            }
        }
        changed
    }
}

#[cfg(test)]
#[path = "config_digest.tests.rs"]
mod tests;
