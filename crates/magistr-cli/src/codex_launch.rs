//! Portable entry point for an installed Codex CLI, separate from agent transport.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use color_eyre::eyre::{self, WrapErr};

use crate::config::inputs::ProjectInputs;

const VERIFIED_VERSION: &str = "0.153.4";
const PROJECT_SKILLS: [&str; 9] = [
    "craft",
    "init",
    "next",
    "next-loop",
    "roadmap",
    "roadmap-refresh",
    "signals",
    "validate",
    "workspace-routing",
];

struct ConfigLocations {
    codex_home: PathBuf,
    home: PathBuf,
    system_config: PathBuf,
}

struct LaunchPlan {
    launch_root: PathBuf,
    working_root: PathBuf,
    args: Vec<OsString>,
    config_layers: Vec<PathBuf>,
    suppressed: Vec<PathBuf>,
}

struct Invocation {
    working_root: PathBuf,
    has_cd: bool,
    profile: Option<String>,
}

/// Start the installed Codex CLI or print read-only compatibility diagnostics.
///
/// The installed CLI retains its authentication, MCP configuration, arguments,
/// stdio, and process exit behavior. This does not select an ACP provider.
/// The user's home and the Codex home come from the captured `inputs`.
///
/// # Errors
/// Returns an error if Codex, the project, or a configuration layer is invalid.
pub fn launch(
    project_root: &Path,
    args: &[OsString],
    diagnose: bool,
    inputs: &ProjectInputs,
) -> eyre::Result<()> {
    let codex = which::which("codex").wrap_err(
        "Codex CLI is not on PATH. Install or activate Codex, then retry codex-launch.",
    )?;
    let home = inputs
        .home_dir()
        .ok_or_else(|| eyre::eyre!("Cannot locate the home directory for Codex skill discovery"))?;
    let codex_home = inputs
        .codex_home()
        .ok_or_else(|| eyre::eyre!("Cannot locate the Codex home for Codex skill discovery"))?;
    let locations = ConfigLocations {
        codex_home: std::path::absolute(codex_home)?,
        home: std::path::absolute(home)?,
        system_config: PathBuf::from("/etc/codex/config.toml"),
    };
    let plan = build_plan(project_root, args, &locations)?;
    if diagnose {
        return diagnose_plan(&codex, &plan, &locations);
    }
    let mut command = Command::new(codex);
    // Relative forwarded -C paths are interpreted from the wrapper's project
    // root, while the compatibility plan uses their resolved destination.
    command
        .current_dir(&plan.launch_root)
        .env("CODEX_HOME", &locations.codex_home)
        .args(plan.args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec()).wrap_err("Could not start Codex CLI")
    }
    #[cfg(not(unix))]
    {
        let status = command.status().wrap_err("Could not start Codex CLI")?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn diagnose_plan(codex: &Path, plan: &LaunchPlan, locations: &ConfigLocations) -> eyre::Result<()> {
    let output = Command::new(codex)
        .arg("--version")
        .output()
        .wrap_err("Could not inspect Codex CLI version")?;
    eyre::ensure!(output.status.success(), "Codex --version failed");
    let version =
        String::from_utf8(output.stdout).wrap_err("Codex --version returned non-UTF-8 output")?;
    let version = version.trim();
    let report = serde_json::json!({
        "codex": codex,
        "installed_version": version,
        "verified_version": VERIFIED_VERSION,
        "matches_verified_version": version == format!("codex-cli {VERIFIED_VERSION}"),
        "compatibility": "process-local skill selections; verified against config/read and skills/list on 0.153.4",
        "working_root": plan.working_root,
        "codex_home": locations.codex_home,
        "selection_layers": plan.config_layers,
        "suppressed_personal_skills": plan.suppressed,
        "transport": "installed Codex CLI; Magistr ACP Codex provider is not connected",
    });
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    writeln!(stdout)?;
    Ok(())
}

fn build_plan(
    project_root: &Path,
    args: &[OsString],
    locations: &ConfigLocations,
) -> eyre::Result<LaunchPlan> {
    let launch_root = project_root.canonicalize().wrap_err_with(|| {
        format!(
            "Cannot resolve Codex project root {}",
            project_root.display()
        )
    })?;
    eyre::ensure!(
        launch_root.is_dir(),
        "Codex project root must be a directory"
    );
    let invocation = invocation(&launch_root, args)?;
    let mut selections = BTreeMap::new();
    let mut config_layers = Vec::new();
    let mut global = toml::Table::new();
    for path in [
        locations.system_config.clone(),
        locations.codex_home.join("config.toml"),
    ] {
        if let Some(config) = read_config(&path)? {
            merge_selections(&mut selections, &config, &path)?;
            global.extend(config);
            config_layers.push(path);
        }
    }
    let profile = invocation
        .profile
        .as_deref()
        .or_else(|| global.get("profile").and_then(toml::Value::as_str));
    if let Some(profile) = profile {
        eyre::ensure!(
            !profile.is_empty()
                && !profile.contains(['/', '\\'])
                && profile != "."
                && profile != "..",
            "Codex profile must be a profile name, not a path"
        );
        let path = locations.codex_home.join(format!("{profile}.config.toml"));
        if let Some(config) = read_config(&path)? {
            merge_selections(&mut selections, &config, &path)?;
            global.extend(config);
            config_layers.push(path);
        }
    }
    let roots = project_directories(&launch_root, &invocation.working_root, &global);
    let mut project_names = BTreeSet::new();
    for directory in &roots {
        for (_, name) in discover_skills(&directory.join(".agents/skills"))? {
            if PROJECT_SKILLS.contains(&name.as_str()) {
                project_names.insert(name);
            }
        }
    }
    let mut suppressed = Vec::new();
    for root in [
        locations.codex_home.join("skills"),
        locations.home.join(".agents/skills"),
    ] {
        for (path, name) in discover_skills(&root)? {
            if project_names.contains(&name) {
                selections.insert(path.clone(), false);
                suppressed.push(path);
            }
        }
    }
    // Explicit project selections override inherited choices, including an
    // intentional re-enable. Later CLI -c arguments retain final precedence.
    for directory in roots {
        let path = directory.join(".codex/config.toml");
        if let Some(config) = read_config(&path)? {
            merge_selections(&mut selections, &config, &path)?;
            config_layers.push(path);
        }
    }
    suppressed.retain(|path| selections.get(path) == Some(&false));
    suppressed.sort();
    suppressed.dedup();
    let mut forwarded = Vec::new();
    if !invocation.has_cd {
        forwarded.extend([OsString::from("-C"), launch_root.clone().into_os_string()]);
    }
    if !selections.is_empty() {
        let mut entries = Vec::new();
        for (path, enabled) in selections {
            let path = path.to_str().ok_or_else(|| {
                eyre::eyre!(
                    "Codex skill configuration requires UTF-8 paths: {}",
                    path.display()
                )
            })?;
            entries.push(format!(
                "{{path={},enabled={enabled}}}",
                toml::Value::String(path.to_owned())
            ));
        }
        forwarded.extend([
            OsString::from("-c"),
            OsString::from(format!("skills.config=[{}]", entries.join(","))),
        ]);
    }
    forwarded.extend_from_slice(args);
    Ok(LaunchPlan {
        launch_root,
        working_root: invocation.working_root,
        args: forwarded,
        config_layers,
        suppressed,
    })
}

fn invocation(project_root: &Path, args: &[OsString]) -> eyre::Result<Invocation> {
    let mut directory = None;
    let mut profile = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        let Some(flag) = arg.to_str() else { continue };
        if flag == "-C" || flag == "--cd" {
            directory =
                Some(PathBuf::from(args.next().ok_or_else(|| {
                    eyre::eyre!("Codex {flag} requires a directory")
                })?));
        } else if let Some(value) = flag.strip_prefix("--cd=").or_else(|| {
            flag.strip_prefix("-C")
                .filter(|value| !value.is_empty())
                .map(|value| value.strip_prefix('=').unwrap_or(value))
        }) {
            directory = Some(PathBuf::from(value));
        } else if flag == "-p" || flag == "--profile" {
            profile = Some(
                args.next()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| eyre::eyre!("Codex {flag} requires a UTF-8 profile name"))?
                    .to_owned(),
            );
        } else if let Some(value) = flag.strip_prefix("--profile=").or_else(|| {
            flag.strip_prefix("-p")
                .filter(|value| !value.is_empty())
                .map(|value| value.strip_prefix('=').unwrap_or(value))
        }) {
            profile = Some(value.to_owned());
        } else if matches!(
            flag,
            "-c" | "--config" | "-m" | "--model" | "-i" | "--image"
        ) {
            // A value that resembles -C or -p belongs to its preceding flag.
            args.next();
        }
    }
    let has_cd = directory.is_some();
    let working_root = directory
        .map(|path| project_root.join(path))
        .unwrap_or_else(|| project_root.to_owned())
        .canonicalize()
        .wrap_err("Cannot resolve the Codex working directory selected by -C/--cd")?;
    eyre::ensure!(
        working_root.is_dir(),
        "Codex working directory must be a directory"
    );
    Ok(Invocation {
        working_root,
        has_cd,
        profile,
    })
}

fn read_config(path: &Path) -> eyre::Result<Option<toml::Table>> {
    match std::fs::read_to_string(path) {
        Ok(content) => content
            .parse()
            .map(Some)
            .wrap_err_with(|| format!("Invalid TOML in Codex config {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).wrap_err_with(|| format!("Cannot read {}", path.display())),
    }
}

fn merge_selections(
    selections: &mut BTreeMap<PathBuf, bool>,
    config: &toml::Table,
    source: &Path,
) -> eyre::Result<()> {
    let Some(entries) = config.get("skills").and_then(|skills| skills.get("config")) else {
        return Ok(());
    };
    let entries = entries
        .as_array()
        .ok_or_else(|| eyre::eyre!("skills.config must be an array in {}", source.display()))?;
    for entry in entries {
        let path = entry
            .get("path")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                eyre::eyre!(
                    "skills.config path must be a string in {}",
                    source.display()
                )
            })?;
        let enabled = entry.get("enabled").map_or(Ok(true), |value| {
            value.as_bool().ok_or_else(|| {
                eyre::eyre!(
                    "skills.config enabled must be a boolean in {}",
                    source.display()
                )
            })
        })?;
        let base = source
            .parent()
            .ok_or_else(|| eyre::eyre!("Config has no parent"))?;
        let mut path = base.join(path);
        if path.is_dir() {
            path.push("SKILL.md");
        }
        let path = path.canonicalize().unwrap_or(path);
        selections.insert(path, enabled);
    }
    Ok(())
}

fn project_directories(project_root: &Path, cwd: &Path, config: &toml::Table) -> Vec<PathBuf> {
    let markers = config
        .get("project_root_markers")
        .and_then(toml::Value::as_array);
    let root = cwd
        .ancestors()
        .find(|directory| {
            markers.map_or_else(
                || directory.join(".git").exists(),
                |markers| {
                    markers
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .any(|marker| directory.join(marker).exists())
                },
            )
        })
        .unwrap_or_else(|| {
            if cwd.starts_with(project_root) {
                project_root
            } else {
                cwd
            }
        });
    let mut roots: Vec<_> = cwd
        .ancestors()
        .take_while(|path| path.starts_with(root))
        .map(Path::to_owned)
        .collect();
    roots.reverse();
    roots
}

fn discover_skills(root: &Path) -> eyre::Result<Vec<(PathBuf, String)>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_owned()];
    let mut visited = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        if !directory.is_dir() {
            continue;
        }
        let directory = directory
            .canonicalize()
            .wrap_err("Cannot resolve a skill directory")?;
        if !visited.insert(directory.clone()) {
            continue;
        }
        let skill = directory.join("SKILL.md");
        if skill.is_file() {
            let content = std::fs::read_to_string(&skill)
                .wrap_err_with(|| format!("Cannot read skill {}", skill.display()))?;
            if let Some(name) = skill_name(&content) {
                found.push((skill, name));
            }
            continue;
        }
        for entry in std::fs::read_dir(&directory).wrap_err("Cannot list skill directory")? {
            let entry = entry.wrap_err("Cannot read skill directory entry")?;
            if !entry.file_name().to_string_lossy().starts_with('.') && entry.path().is_dir() {
                pending.push(entry.path());
            }
        }
    }
    found.sort();
    Ok(found)
}

fn skill_name(content: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines.take_while(|line| line.trim() != "---") {
        if let Some(value) = line.strip_prefix("name:") {
            let value = value.trim();
            let name = if let Some(value) = value.strip_prefix(['\'', '"']) {
                value.split(['\'', '"']).next()?
            } else {
                value.split_whitespace().next()?
            };
            if !name.is_empty() {
                return Some(name.to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "codex_launch.tests.rs"]
mod tests;
