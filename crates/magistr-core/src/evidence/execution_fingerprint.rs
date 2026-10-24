//! Bind receipts to the environment and the actual tools, without exposing values.
use magistr_process::{Launch, ProcessSpec, Termination, run_blocking};
use magistr_workspace::file_content::FileContent;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone)]
pub(super) struct ExecutionFingerprint {
    command: String,
    files: Vec<String>,
    pub digest: String,
}

#[derive(Clone)]
struct Cached {
    base: String,
    tools: Vec<PathBuf>,
    stamps: Vec<String>,
    digest: String,
}

impl ExecutionFingerprint {
    pub fn observe(root: &Path, command: Option<&str>, files: &[String]) -> Result<Self, String> {
        let command = command.unwrap_or_default().to_owned();
        let mut environment: Vec<_> = std::env::vars_os()
            .map(|(key, value)| {
                (
                    key.as_encoded_bytes().to_vec(),
                    value.as_encoded_bytes().to_vec(),
                )
            })
            .collect();
        environment.sort();
        let mut configuration = Vec::new();
        for path in [
            "rust-toolchain",
            "rust-toolchain.toml",
            ".cargo/config",
            ".cargo/config.toml",
        ] {
            configuration.push(read_optional(&root.join(path))?);
        }
        if let Some(directory) = std::env::var_os("RUSTUP_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".rustup")))
        {
            configuration.push(read_optional(&directory.join("settings.toml"))?);
        }
        let profile = tool_profile(&command, files);
        for file in files.iter().filter(|file| file.ends_with("package.json")) {
            let directory = root.join(file).parent().unwrap_or(root).to_owned();
            for marker in [
                "node_modules/.package-lock.json",
                "node_modules/.modules.yaml",
                "node_modules/.yarn-integrity",
                ".pnp.cjs",
            ] {
                configuration.push(read_optional(&directory.join(marker))?);
            }
        }
        for interpreter in ["python", "python3"]
            .into_iter()
            .filter(|name| profile.contains("python") && executable(name).is_some())
        {
            // Installed packages can change without changing the interpreter or
            // lockfile. Hash the live distribution set, not a stale version cache.
            configuration.push(probe(root, &[interpreter, "-c", "import hashlib,importlib.metadata as m; print(hashlib.sha256(repr(sorted((d.metadata.get('Name',''),d.version) for d in m.distributions())).encode()).hexdigest())"])?.into_bytes());
        }
        let probes = probes(&profile);
        let mut tools: Vec<_> = probes
            .iter()
            .filter_map(|args| executable(args[0]))
            .collect();
        tools.push(if cfg!(windows) {
            PathBuf::from("C:/Windows/System32/cmd.exe")
        } else {
            PathBuf::from("/bin/sh")
        });
        let stamps: Vec<_> = tools.iter().map(|path| stamp(path)).collect();
        let base = hash(
            &serde_json::to_vec(&(&environment, &configuration, &stamps))
                .map_err(|error| error.to_string())?,
        );
        let key = (root.to_owned(), profile.clone());
        static CACHE: OnceLock<Mutex<HashMap<(PathBuf, String), Cached>>> = OnceLock::new();
        let cache = CACHE.get_or_init(Mutex::default);
        if let Some(hit) = cache
            .lock()
            .map_err(|_| "The toolchain cache could not be read.")?
            .get(&key)
            .cloned()
            && cfg!(unix)
            && hit.base == base
            && hit.tools.iter().map(|path| stamp(path)).collect::<Vec<_>>() == hit.stamps
        {
            return Ok(Self {
                command,
                files: files.to_vec(),
                digest: hit.digest,
            });
        }
        let mut versions = Vec::new();
        for args in probes {
            // An absent optional tool is part of the fingerprint; installing it
            // changes the executable list and therefore invalidates the result.
            if executable(args[0]).is_none() {
                continue;
            }
            versions.push(probe(root, &args)?);
        }
        if profile.contains("cargo") && executable("rustup").is_some() {
            for tool in ["rustc", "cargo"] {
                let spec = ProcessSpec::new(
                    Launch::Argv(vec!["rustup".into(), "which".into(), tool.into()]),
                    Duration::from_secs(3),
                )
                .in_dir(root)
                .with_capture_bytes(8192)
                .with_termination(Termination::Immediate);
                let result = run_blocking(spec).map_err(|error| error.to_string())?;
                if !result.is_complete() || result.exit_code != Some(0) {
                    return Err("The active Rust toolchain could not be identified.".into());
                }
                let path = PathBuf::from(result.stdout.trim());
                if let Some(root) = path.parent().and_then(Path::parent)
                    && let Ok(entries) = std::fs::read_dir(root.join("lib"))
                {
                    tools.extend(
                        entries
                            .flatten()
                            .map(|entry| entry.path())
                            .filter(|path| path.is_file()),
                    );
                }
                tools.push(path);
            }
        }
        let stamps: Vec<_> = tools.iter().map(|path| stamp(path)).collect();
        let digest = hash(
            &serde_json::to_vec(&(&base, &versions, &stamps)).map_err(|error| error.to_string())?,
        );
        let mut cache = cache
            .lock()
            .map_err(|_| "The toolchain cache could not be saved.")?;
        if cache.len() >= 64 {
            cache.clear();
        }
        cache.insert(
            key,
            Cached {
                base,
                tools,
                stamps,
                digest: digest.clone(),
            },
        );
        Ok(Self {
            command,
            files: files.to_vec(),
            digest,
        })
    }

    pub fn unchanged(&self, root: &Path) -> Result<bool, String> {
        Ok(Self::observe(root, Some(&self.command), &self.files)?.digest == self.digest)
    }
}

fn probes(command: &str) -> Vec<Vec<&'static str>> {
    let mut result = Vec::new();
    if command.contains("cargo") || command.contains("rustc") {
        result.extend([vec!["rustc", "-vV"], vec!["cargo", "--version"]]);
        if command.contains("nextest") {
            result.push(vec!["cargo-nextest", "--version"]);
        }
    }
    if command.contains("npm") || command.contains("node") {
        result.push(vec!["node", "--version"]);
        if command.contains("npm") {
            result.push(vec!["npm", "--version"]);
        }
    }
    if command.contains("python") {
        result.extend([vec!["python", "--version"], vec!["python3", "--version"]]);
    }
    if command.split_whitespace().any(|word| word == "git") {
        result.push(vec!["git", "--version"]);
    }
    result
}

fn executable(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|directory| {
        let path = directory.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
        path.is_file().then(|| path.canonicalize().unwrap_or(path))
    })
}

fn read_optional(path: &Path) -> Result<Vec<u8>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("Cannot observe toolchain settings: {error}")),
    }
}

fn stamp(path: &Path) -> String {
    let Ok(metadata) = std::fs::metadata(path) else {
        return format!("{}:missing", path.display());
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}",
            path.display(),
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec()
        )
    }
    #[cfg(not(unix))]
    {
        format!(
            "{}:{}:{:?}:{:?}",
            path.display(),
            metadata.len(),
            metadata.modified(),
            metadata.created()
        )
    }
}
fn hash(bytes: &[u8]) -> String {
    FileContent::decode(bytes.to_vec()).content_hash()
}

fn tool_profile(command: &str, files: &[String]) -> String {
    let mut hints = command.to_owned();
    for file in files {
        match Path::new(file).file_name().and_then(|name| name.to_str()) {
            Some("Cargo.toml") => hints.push_str(" cargo nextest"),
            Some("package.json") => hints.push_str(" npm"),
            Some("pyproject.toml" | "requirements.txt") => hints.push_str(" python"),
            _ if file.ends_with(".py") => hints.push_str(" python"),
            _ => {}
        }
    }
    probes(&hints)
        .iter()
        .map(|probe| probe[0])
        .collect::<Vec<_>>()
        .join(" ")
}

fn probe(root: &Path, args: &[&str]) -> Result<String, String> {
    let result = run_blocking(
        ProcessSpec::new(
            Launch::Argv(args.iter().map(|arg| (*arg).into()).collect()),
            Duration::from_secs(3),
        )
        .in_dir(root)
        .with_capture_bytes(8192)
        .with_termination(Termination::Immediate),
    )
    .map_err(|error| format!("Cannot identify {}: {error}", args[0]))?;
    if !result.is_complete() || result.exit_code != Some(0) {
        return Err(format!(
            "Cannot identify {} within its verification deadline.",
            args[0]
        ));
    }
    Ok(format!("{}\n{}", result.stdout, result.stderr))
}
