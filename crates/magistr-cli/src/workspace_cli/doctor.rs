//! `magistr workspace doctor [bundle|config]`: the read-only doctor plane.
//! `doctor` prints the report — the filesystem and configuration probes over the
//! `.magistr/workspace` artifacts, then the `owner` section this host composes from
//! the durable records and one bounded probe of the owner endpoint, and
//! the `operations` section from the run checkpoint — `doctor bundle`
//! packs the same sections into a redacted tar.gz, `doctor config` prints
//! the five-layer resolved configuration with per-key provenance. No lock
//! is taken, so every form runs beside a live server. Exit 0 healthy, 1
//! findings.

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
use crate::config::inputs::ProjectInputs;

/// Run one doctor form with the retained renderings and exit codes. The
/// configuration environment is the one the host captured, so the doctor
/// reads exactly what the server itself would.
pub fn run_doctor(
    mode: Option<WorkspaceDoctorMode>,
    dir: &Path,
    config: &[String],
    inputs: &ProjectInputs,
) -> eyre::Result<()> {
    let env = inputs.workspace_env();
    match mode {
        None => report(&admitted_root(dir), &session_flags(config), env, None),
        Some(WorkspaceDoctorMode::Bundle { dir, out, config }) => report(
            &admitted_root(&dir),
            &session_flags(&config),
            env,
            Some(out),
        ),
        Some(WorkspaceDoctorMode::Config { dir, config }) => {
            resolved_config(&admitted_root(&dir), &session_flags(&config), env)
        }
    }
}

fn resolved_config(
    root: &Path,
    flags: &serde_json::Map<String, serde_json::Value>,
    env: &BTreeMap<String, String>,
) -> eyre::Result<()> {
    let options = ResolveOptions {
        root: root.to_path_buf(),
        env: env.clone(),
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
    env: &BTreeMap<String, String>,
    bundle: Option<Option<PathBuf>>,
) -> eyre::Result<()> {
    let options = DoctorOptions::new(root)
        .with_env(env.clone())
        .with_session_flags(flags.clone());
    let mut run = DoctorRun::observe(&options);
    let owner = super::doctor_owner::owner_section(
        root,
        &options.state_dir,
        super::doctor_owner::PROBE_DEADLINE,
    );
    let workspace_generation = owner.detail["durableGeneration"].as_u64();
    run.append(owner);
    run.append(super::doctor_operations::operations_section(
        root,
        workspace_generation,
    ));
    match bundle {
        None => println!("{}", run.render()),
        Some(out) => {
            let target = std::path::absolute(out.unwrap_or_else(|| {
                PathBuf::from(format!(
                    "workspace-doctor-bundle-{}.tar.gz",
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
