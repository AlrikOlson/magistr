use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::WorkerDataFormat;

#[cfg(feature = "sandbox")]
use super::{WorkerPolicy, WorkerSubprocessSpec};

/// Work admitted by the host and executed entirely inside its native worker.
/// Paths identify parser inputs; guest artifacts travel as bytes and are never
/// read through ambient worker filesystem authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum WorkerRequest {
    Ping,
    Parse {
        path: String,
        language: String,
        source: String,
    },
    ValidateData {
        path: String,
        format: WorkerDataFormat,
        source: String,
    },
    Search {
        path: String,
        pattern: String,
        haystack: String,
        case_insensitive: bool,
    },
    Structural {
        path: String,
        language: String,
        source: String,
        pattern: String,
    },
    Rewrite {
        path: String,
        language: String,
        source: String,
        pattern: String,
        replacement: String,
    },
    Extract {
        path: String,
        language: String,
        source: String,
    },
    #[cfg(feature = "sandbox")]
    Component {
        guest: Vec<u8>,
        export: String,
        args: Vec<i64>,
        capabilities: Vec<String>,
        policy: WorkerPolicy,
    },
    #[cfg(feature = "sandbox")]
    Wasi {
        guest: Vec<u8>,
        argv: Vec<String>,
        policy: WorkerPolicy,
    },
    #[cfg(feature = "sandbox")]
    InvokeComponent {
        guest: Vec<u8>,
        export: String,
        payload: String,
        capabilities: Vec<String>,
        policy: WorkerPolicy,
        subprocess_specs: Vec<WorkerSubprocessSpec>,
        subprocess_root: String,
    },
    #[cfg(feature = "sandbox")]
    Resolver {
        guest: Vec<u8>,
        symbol: String,
        capabilities: Vec<String>,
        policy: WorkerPolicy,
        subprocess_specs: Vec<WorkerSubprocessSpec>,
        subprocess_root: String,
    },
    #[cfg(feature = "lang-pack")]
    RegisterLanguage {
        id: String,
        extensions: Vec<String>,
        grammar_wasm: Vec<u8>,
        tag_queries: String,
        /// Host trust verdict; this selects the native promotion lane only.
        blessed: bool,
    },
    #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
    LoadLanguagePack {
        guest: Vec<u8>,
        capabilities: Vec<String>,
        policy: WorkerPolicy,
        blessed: bool,
    },
    #[cfg(feature = "sandbox")]
    ClassifyLanguagePack {
        guest: Vec<u8>,
        paths: Vec<String>,
        capabilities: Vec<String>,
        policy: WorkerPolicy,
    },
}

impl WorkerRequest {
    /// Quarantine exactly this request, so editing a file or changing its
    /// parser options makes the corrected input eligible again. Stream into
    /// the digest instead of allocating a second full guest/source buffer.
    pub(crate) fn input_key(&self) -> Option<String> {
        let label = match self {
            Self::Ping => return None,
            Self::Parse { path, .. }
            | Self::ValidateData { path, .. }
            | Self::Search { path, .. }
            | Self::Structural { path, .. }
            | Self::Rewrite { path, .. }
            | Self::Extract { path, .. } => path.as_str(),
            #[cfg(feature = "sandbox")]
            Self::Component { .. } => "component",
            #[cfg(feature = "sandbox")]
            Self::Wasi { .. } => "wasi",
            #[cfg(feature = "sandbox")]
            Self::InvokeComponent { .. } => "invoke-component",
            #[cfg(feature = "sandbox")]
            Self::Resolver { .. } => "resolver",
            #[cfg(feature = "lang-pack")]
            Self::RegisterLanguage { id, .. } => id.as_str(),
            #[cfg(all(feature = "sandbox", feature = "lang-pack"))]
            Self::LoadLanguagePack { .. } => "language-pack",
            #[cfg(feature = "sandbox")]
            Self::ClassifyLanguagePack { .. } => "language-pack-classify",
        };
        let mut hash = HashWriter(Sha256::new());
        serde_json::to_writer(&mut hash, self).ok()?;
        let label: String = label.chars().take(120).collect();
        Some(format!("{label}:{:x}", hash.0.finalize()))
    }
}

struct HashWriter(Sha256);

impl std::io::Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
