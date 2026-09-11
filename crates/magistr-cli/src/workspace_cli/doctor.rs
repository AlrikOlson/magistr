//! `magistr workspace doctor [bundle|config]`: the read-only doctor plane.
//! `doctor` prints the six-section report, `doctor bundle` packs the same
//! probes into a redacted tar.gz, `doctor config` prints the five-layer
//! resolved configuration with per-key provenance. No lock is taken, so
//! every form runs beside a live server. Exit 0 healthy, 1 findings.

use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

use color_eyre::eyre;
use magistr_workspace::{
    config::{ResolveOptions, ResolvedConfig},
    contracts::ErrorKind,
    diagnostics::{DoctorBundle, DoctorOptions, DoctorRun},
    service::WorkspaceOpenError,
};

use super::{admitted_root, exit::EXIT_USAGE, fail, session_flags};
use crate::cli::WorkspaceDoctorMode;

/// Run one doctor form with the retained renderings and exit codes.
pub fn run_doctor(
    mode: Option<WorkspaceDoctorMode>,
    dir: &Path,
    config: &[String],
) -> eyre::Result<()> {
    match mode {
        None => report(&admitted_root(dir), &session_flags(config), None),
        Some(WorkspaceDoctorMode::Bundle { dir, out, config }) => {
            report(&admitted_root(&dir), &session_flags(&config), Some(out))
        }
        Some(WorkspaceDoctorMode::Config { dir, config }) => {
            resolved_config(&admitted_root(&dir), &session_flags(&config))
        }
    }
}

/// The configuration environment the server itself reads.
fn environment(root: &Path) -> BTreeMap<String, String> {
    ResolveOptions::from_environment(root).env
}

fn resolved_config(
    root: &Path,
    flags: &serde_json::Map<String, serde_json::Value>,
) -> eyre::Result<()> {
    let options = ResolveOptions {
        root: root.to_path_buf(),
        env: environment(root),
        session_flags: flags.clone(),
    };
    match ResolvedConfig::resolve(&options) {
        Ok(config) => {
            println!("{}", config.doctor(root));
            Ok(())
        }
        // A config layer written by a newer binary: the data is the
        // problem, not the environment — the server's own exit.
        Err(error) if error.kind == ErrorKind::UnsupportedVersion => {
            fail(WorkspaceOpenError::EXIT_DATA_ERROR, &error.message)
        }
        Err(error) => Err(eyre::eyre!("{error}")),
    }
}

/// `bundle` is `None` for the report, `Some(target)` for the archive
/// (`None` inside picks the retained timestamped name in the current
/// directory). A diagnostic never destroys a prior diagnostic.
fn report(
    root: &Path,
    flags: &serde_json::Map<String, serde_json::Value>,
    bundle: Option<Option<PathBuf>>,
) -> eyre::Result<()> {
    let options = DoctorOptions::new(root)
        .with_env(environment(root))
        .with_session_flags(flags.clone());
    let run = DoctorRun::observe(&options);
    match bundle {
        None => println!("{}", run.render()),
        Some(out) => {
            let target = std::path::absolute(out.unwrap_or_else(|| {
                PathBuf::from(format!(
                    "workbench-doctor-bundle-{}.tar.gz",
                    run.report.generated_at.replace(':', "-")
                ))
            }))?;
            if target.exists() {
                fail(
                    EXIT_USAGE,
                    &format!("bundle target already exists: {}", target.display()),
                );
            }
            let bundle = DoctorBundle::build(&run)?;
            std::fs::write(&target, &bundle.bytes)?;
            eprintln!("magistr workspace: bundle written to {}", target.display());
            for member in &bundle.members {
                eprintln!("  {member}");
            }
        }
    }
    if run.report.findings_present {
        std::io::stdout().flush()?;
        std::process::exit(1);
    }
    Ok(())
}
