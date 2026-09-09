//! `magistr team` — show identity and role, manage the roster.

use std::path::Path;
use std::str::FromStr;

use color_eyre::eyre;

use crate::cli::TeamAction;
use crate::error::MagistrError;
use crate::output::OutputRenderer;
use crate::permissions::AuditRetention;
use crate::team::{
    Capability, Identity, RosterChange, TeamConfig, TeamMember, TeamRole, is_ci_env, resolve_role,
};
use crate::utils::format as fmt;

use super::edit_roster;

pub fn cmd_team(
    dir: &Path,
    action: Option<TeamAction>,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    match action.unwrap_or(TeamAction::Show) {
        TeamAction::Show => show(dir, renderer),
        TeamAction::Init => init(dir, renderer),
        TeamAction::Add { id, role, name } => {
            let role = parse_role(&role)?;
            mutate(
                dir,
                renderer,
                &id,
                &format!("Added '{id}'"),
                |_| Ok(RosterChange::Add(role)),
                |config| {
                    config.add_member(TeamMember {
                        id: id.clone(),
                        name: name.clone(),
                        role,
                        active: true,
                    })
                },
            )
        }
        TeamAction::Remove { id } => mutate(
            dir,
            renderer,
            &id,
            &format!("Removed '{id}'"),
            |config| current_role(config, &id).map(RosterChange::Remove),
            |config| config.remove_member(&id),
        ),
        TeamAction::SetRole { id, role } => {
            let to = parse_role(&role)?;
            mutate(
                dir,
                renderer,
                &id,
                &format!("Set '{id}' to {to}"),
                |config| current_role(config, &id).map(|from| RosterChange::SetRole { from, to }),
                |config| config.set_role(&id, to),
            )
        }
    }
}

fn show(dir: &Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let identity = Identity::resolve(dir);
    let config = TeamConfig::load(dir)?;
    let role = resolve_role(config.as_ref(), &identity, is_ci_env());

    if renderer.is_machine_readable() {
        let data = serde_json::json!({
            "identity": { "name": identity.name, "email": identity.email },
            "role": role.to_string(),
            "solo": config.is_none(),
            "members": config.as_ref().map(|c| &c.members),
        });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }

    let email = identity
        .email
        .as_deref()
        .map(|e| format!(" <{e}>"))
        .unwrap_or_default();
    eprintln!("{}\n", fmt::bold("👥 Team"));
    eprintln!("  {} {}{email}", fmt::bold("You:"), identity.name);
    eprintln!(
        "  {} {role}",
        fmt::bold("Role:"),
        role = fmt::bold(&role.to_string())
    );

    let capabilities: Vec<&str> = Capability::ALL
        .into_iter()
        .filter(|c| role.can(*c))
        .map(Capability::label)
        .collect();
    eprintln!("  {} {}", fmt::bold("Can:"), capabilities.join(", "));

    match config {
        None => {
            eprintln!();
            eprintln!(
                "  {}",
                fmt::dim(
                    "Solo mode — no .magistr/team.json. Run `magistr team init` to create one."
                )
            );
        }
        Some(config) => {
            eprintln!();
            eprintln!(
                "  {} ({} member{})",
                fmt::bold("Roster"),
                config.members.len(),
                if config.members.len() == 1 { "" } else { "s" },
            );
            for member in &config.members {
                let name = member
                    .name
                    .as_deref()
                    .map(|n| format!(" ({n})"))
                    .unwrap_or_default();
                eprintln!(
                    "    {}  {}{}",
                    fmt::bold(&format!("{:<9}", member.role.to_string())),
                    member.id,
                    fmt::dim(&name),
                );
            }
            eprintln!();
            eprintln!("  {} {}", fmt::bold("Default role:"), config.default_role);
        }
    }
    Ok(())
}

fn init(dir: &Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    if TeamConfig::path(dir).exists() {
        return Err(MagistrError::Other(
            ".magistr/team.json already exists — edit it with `magistr team add/remove/set-role`"
                .to_string(),
        )
        .into());
    }
    let identity = Identity::resolve(dir);
    let config = TeamConfig::new_with_owner(&identity);
    config.save(dir)?;
    report(
        renderer,
        &format!(
            "Created .magistr/team.json with '{}' as Owner",
            identity.id()
        ),
        &config,
    )
}

/// The role a roster edit is about to disturb.
fn current_role(config: &TeamConfig, id: &str) -> Result<TeamRole, MagistrError> {
    config
        .role_of(id)
        .ok_or_else(|| MagistrError::Other(format!("no member '{id}' on the roster")))
}

/// Load-authorize-mutate-save-audit around one roster operation, then
/// print the receipt. The decision itself lives in [`edit_roster`].
fn mutate(
    dir: &Path,
    renderer: &dyn OutputRenderer,
    member_id: &str,
    done: &str,
    change: impl FnOnce(&TeamConfig) -> Result<RosterChange, MagistrError>,
    op: impl FnOnce(&mut TeamConfig) -> Result<(), MagistrError>,
) -> eyre::Result<()> {
    let audit_log = crate::permissions::audit_log_path();
    let config = edit_roster(
        dir,
        member_id,
        change,
        op,
        audit_log.as_deref(),
        &retention_for(dir),
    )?;
    report(renderer, done, &config)
}

fn retention_for(dir: &Path) -> AuditRetention {
    crate::permissions::load_config(dir)
        .unwrap_or_default()
        .retention
}

fn report(renderer: &dyn OutputRenderer, message: &str, config: &TeamConfig) -> eyre::Result<()> {
    if renderer.is_machine_readable() {
        let data = serde_json::json!({ "result": message, "members": config.members });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| MagistrError::Serialize(e.to_string()))?;
    } else {
        eprintln!("{} {message}", fmt::icon_pass());
    }
    Ok(())
}

fn parse_role(role: &str) -> Result<TeamRole, MagistrError> {
    TeamRole::from_str(role).map_err(MagistrError::Other)
}

#[cfg(test)]
#[path = "cmd_team.tests.rs"]
mod tests;
