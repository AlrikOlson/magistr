use std::{path::Path, sync::Arc};

use sha2::{Digest, Sha256};

/// Ceiling on the interpreter artifact the host will send to the worker.
const INTERPRETER_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// The retained QuickJS-ng `qjs-wasi.wasm` bytes the script tool runs. The
/// composing host chooses the artifact; its hash is reported so a caller can
/// pin the provenance recorded beside the reference tree.
#[derive(Clone, Debug)]
pub struct ScriptInterpreter {
    bytes: Arc<Vec<u8>>,
    sha256: String,
}

impl ScriptInterpreter {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_file() || metadata.len() > INTERPRETER_MAX_BYTES {
            return Err(std::io::Error::other(format!(
                "{} is not a regular interpreter artifact within {INTERPRETER_MAX_BYTES} bytes",
                path.display()
            )));
        }
        Ok(Self::from_bytes(std::fs::read(path)?))
    }

    #[must_use]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes: Arc::new(bytes),
        }
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
