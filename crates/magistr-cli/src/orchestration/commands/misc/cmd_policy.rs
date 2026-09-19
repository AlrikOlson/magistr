//! `magistr policy` — inspect and test the org's workflow guardrails.

use std::path::Path;

use color_eyre::eyre;
use magistr_engine::policy::{GovernancePolicy, PolicyEffect};

use crate::cli::PolicyAction;
use crate::error::{MagistrError, ResultExt};
use crate::output::OutputRenderer;
use crate::permissions::{ViolationReport, policy_request_for};
use crate::team::TeamConfig;
use crate::utils::format as fmt;

pub fn cmd_policy(
    dir: &Path,
    inputs: &crate::config::inputs::ProjectInputs,
    action: Option<PolicyAction>,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    let policy = TeamConfig::governance_for(dir);
    match action.unwrap_or(PolicyAction::Show) {
        PolicyAction::Show => show(&policy, renderer),
        PolicyAction::Check {
            tool,
            command,
            path,
            model,
        } => check(&policy, tool, command, path, model, renderer),
        PolicyAction::Report { actor } => report(actor.as_deref(), renderer, inputs),
    }
}

fn show(policy: &GovernancePolicy, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let engine = policy.compile();

    if renderer.is_machine_readable() {
        let data = serde_json::json!({
            "governed": policy.is_set(),
            "policy": policy,
            "compiled_rules": engine.rules,
        });
        return emit(renderer, &data);
    }

    eprintln!("{}\n", fmt::bold("🛡  Workflow policy"));
    if !policy.is_set() {
        eprintln!(
            "  {}",
            fmt::dim(
                "Ungoverned — no `policy` block in .magistr/team.json. Sessions run on the local profile alone."
            )
        );
        return Ok(());
    }

    list("Approved tools", &policy.tool_allowlist);
    list("Denied tools", &policy.tool_denylist);
    list("Protected paths", &policy.protected_paths);
    list("Approved models", &policy.approved_models);
    if let Some(max) = policy.max_model_tier {
        eprintln!("  {} {max}", fmt::bold("Max model tier:"));
    }
    if let Some(fallback) = &policy.fallback_model {
        eprintln!("  {} {fallback}", fmt::bold("Fallback model:"));
    }
    if !policy.required_gates.is_empty() {
        list("Required gates", &policy.required_gates);
        eprintln!(
            "  {} {}",
            fmt::bold("Gate bypass:"),
            if policy.allow_gate_bypass {
                "permitted"
            } else {
                "refused"
            }
        );
    }

    eprintln!();
    eprintln!("  {} ({})", fmt::bold("Compiled rules"), engine.rules.len());
    for rule in &engine.rules {
        let flag = if rule.override_allowed {
            " (overridable)"
        } else {
            ""
        };
        eprintln!(
            "    {}  {}{}",
            fmt::bold(&format!("{:<16}", rule.effect.as_str())),
            rule.id,
            fmt::dim(flag),
        );
        if !rule.description.is_empty() {
            eprintln!("      {}", fmt::dim(&rule.description));
        }
    }
    Ok(())
}

fn check(
    policy: &GovernancePolicy,
    tool: Option<String>,
    command: Option<String>,
    paths: Vec<String>,
    model: Option<String>,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    if tool.is_none() && command.is_none() && paths.is_empty() && model.is_none() {
        return Err(MagistrError::Other(
            "nothing to check — pass at least one of --tool, --command, --path, --model"
                .to_string(),
        )
        .into());
    }

    // Route the act through exactly the shape enforcement uses, so what
    // this prints is what a session would actually get.
    let mut input = serde_json::Map::new();
    if let Some(command) = &command {
        input.insert("command".into(), serde_json::Value::String(command.clone()));
    }
    if !paths.is_empty() {
        input.insert(
            "paths".into(),
            serde_json::Value::Array(
                paths
                    .iter()
                    .map(|p| serde_json::Value::String(p.clone()))
                    .collect(),
            ),
        );
    }
    let mut request = policy_request_for(
        tool.as_deref().unwrap_or_default(),
        &serde_json::Value::Object(input),
        model.as_deref(),
    );
    if tool.is_none() {
        // A model-only or path-only check must not be answered by a
        // *tool* allowlist it says nothing about — `PolicyRequest::tool`
        // would otherwise leave an empty name behind, which `Tool("*")`
        // happily matches.
        request.tool = None;
        request.risk = None;
    }
    let decision = policy.compile().evaluate(&request);

    if renderer.is_machine_readable() {
        let data = serde_json::json!({
            "request": request.summary(),
            "effect": decision.effect.as_str(),
            "binding_rule": decision.binding,
            "matched_rules": decision.matched,
            "override_allowed": decision.override_allowed,
        });
        return emit(renderer, &data);
    }

    eprintln!("{}\n", fmt::bold("🛡  Policy check"));
    eprintln!("  {} {}", fmt::bold("Request:"), request.summary());
    let verdict = decision.effect.as_str();
    eprintln!(
        "  {} {}",
        fmt::bold("Effect: "),
        if decision.effect == PolicyEffect::Deny {
            fmt::red(verdict)
        } else if decision.effect.is_violation() {
            fmt::yellow(verdict)
        } else {
            fmt::green(verdict)
        }
    );
    eprintln!("  {} {}", fmt::bold("Why:    "), decision.explain());
    if !decision.matched.is_empty() {
        eprintln!(
            "  {} {}",
            fmt::bold("Matched:"),
            decision.matched.join(", ")
        );
    }
    if decision.override_allowed {
        eprintln!("  {}", fmt::dim("a human may override this, on the record"));
    }
    Ok(())
}

fn report(
    actor: Option<&str>,
    renderer: &dyn OutputRenderer,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let path = crate::permissions::audit_log_path(inputs).ok_or_else(|| {
        MagistrError::Config("could not determine data directory (HOME not set)".into())
    })?;
    let mut entries = crate::permissions::load_entries(&path).suggest()?;
    if let Some(who) = actor {
        entries.retain(|e| e.actor.as_deref().is_some_and(|a| a.contains(who)));
    }
    let report = ViolationReport::from_entries(&entries);

    if renderer.is_machine_readable() {
        let data = serde_json::json!({
            "total": report.total,
            "denied": report.denied,
            "by_rule": report.by_rule,
            "by_actor": report.by_actor,
        });
        return emit(renderer, &data);
    }

    eprintln!("{}\n", fmt::bold("🛡  Policy compliance"));
    if report.is_empty() {
        eprintln!(
            "  {}",
            fmt::dim("No policy decision in the audit log — nothing has been caught yet.")
        );
        return Ok(());
    }
    eprintln!(
        "  {} {} ({} denied)",
        fmt::bold("Violations:"),
        report.total,
        report.denied
    );
    eprintln!();
    eprintln!("  {}", fmt::bold("By rule"));
    for (rule, count) in report.ranked_rules() {
        eprintln!("    {:>5}  {rule}", count);
    }
    eprintln!();
    eprintln!("  {}", fmt::bold("By actor"));
    for (who, count) in &report.by_actor {
        eprintln!("    {count:>5}  {who}");
    }
    Ok(())
}

fn list(label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    eprintln!(
        "  {} {}",
        fmt::bold(&format!("{label}:")),
        values.join(", ")
    );
}

fn emit(renderer: &dyn OutputRenderer, data: &serde_json::Value) -> eyre::Result<()> {
    renderer
        .json(&mut std::io::stdout(), data)
        .map_err(|e| MagistrError::Serialize(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
#[path = "cmd_policy.tests.rs"]
mod tests;
