use crate::error::{self, ResultExt};
use crate::lint;
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub async fn cmd_doctor(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let selected = crate::scaffold::agents_present(&dir);
    let mut checks = lint::doctor(&dir, selected.iter().any(|agent| agent == "claude"));
    let connection = crate::unified_mcp::check_connection(&dir).await;
    checks.push(lint::doctor::DoctorCheck {
        name: "mcp-connection",
        description: "MCP handshake and read-only context".into(),
        status: if connection.is_ok() {
            lint::doctor::CheckStatus::Pass
        } else {
            lint::doctor::CheckStatus::Fail
        },
        detail: connection.err().map(|error| format!("{error:#}")),
    });
    for agent in ["codex", "claude"] {
        if selected.iter().any(|selected| selected == agent) {
            checks.push(lint::doctor::DoctorCheck {
                name: agent,
                description: format!("{agent} installation"),
                status: if which::which(agent).is_ok() { lint::doctor::CheckStatus::Pass } else { lint::doctor::CheckStatus::Warn },
                detail: which::which(agent).err().map(|_| format!("Install {agent}, then open this project. Project files are already configured.")),
            });
        }
    }
    match crate::scaffold::repair::plan(&dir) {
        Ok(plan) => checks.push(lint::doctor::DoctorCheck {
            name: "setup-files",
            description: "Generated agent setup".into(),
            status: if plan.refusals() > 0 {
                lint::doctor::CheckStatus::Fail
            } else if plan.applies() > 0 {
                lint::doctor::CheckStatus::Warn
            } else {
                lint::doctor::CheckStatus::Pass
            },
            detail: (plan.applies() > 0 || plan.refusals() > 0).then(|| {
                format!(
                    "{} changes, {} refusals. Run magistr repair --dry-run to review.",
                    plan.applies(),
                    plan.refusals()
                )
            }),
        }),
        Err(error) => checks.push(lint::doctor::DoctorCheck {
            name: "setup-files",
            description: "Cannot inspect agent setup".into(),
            status: lint::doctor::CheckStatus::Fail,
            detail: Some(error.to_string()),
        }),
    }
    // The workbench `run` allowlist is seeded from project.json once; flag
    // the two drifting apart.
    checks.push(crate::scaffold::workbench_drift::doctor_check(&dir));

    if renderer.is_machine_readable() {
        let checks_json: Vec<serde_json::Value> = checks
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "description": c.description,
                    "status": c.status.label(),
                    "detail": c.detail,
                })
            })
            .collect();

        let fails = checks
            .iter()
            .filter(|c| c.status == lint::doctor::CheckStatus::Fail)
            .count();

        let result = serde_json::json!({
            "passed": fails == 0,
            "checks": checks_json,
        });

        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "{} — checking project health\n",
            fmt::bold("magistr doctor")
        );
        for c in &checks {
            let icon = c.status.icon();
            let colored_icon = match c.status {
                lint::doctor::CheckStatus::Pass => fmt::green(icon),
                lint::doctor::CheckStatus::Warn => fmt::yellow(icon),
                lint::doctor::CheckStatus::Fail => fmt::red(icon),
            };
            println!("  {colored_icon} {}: {}", c.name, c.description);
            if let Some(detail) = &c.detail {
                println!("    {}", fmt::dim(&format!("→ {detail}")));
            }
        }

        let fails = checks
            .iter()
            .filter(|c| c.status == lint::doctor::CheckStatus::Fail)
            .count();
        let warns = checks
            .iter()
            .filter(|c| c.status == lint::doctor::CheckStatus::Warn)
            .count();

        println!();
        if fails == 0 && warns == 0 {
            println!("{}", fmt::success("All checks passed!"));
        } else {
            println!("  {} fail(s), {} warning(s)", fails, warns);
        }
    }

    let has_failures = checks
        .iter()
        .any(|c| c.status == lint::doctor::CheckStatus::Fail);
    if has_failures {
        std::process::exit(1);
    }

    Ok(())
}
