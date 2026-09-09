//! Session, cost, and permission audit commands.

use crate::cli;
use crate::cost;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// Print time-bucketed cost summaries.
fn cmd_costs_summary(
    entries: &[cost::CostEntry],
    period: cost::time_period::TimePeriod,
    format: cli::CostFormat,
) -> eyre::Result<()> {
    let buckets = cost::summarize_by_period::summarize_by_period(entries, period);

    match format {
        cli::CostFormat::Json => {
            let json_buckets: Vec<serde_json::Value> = buckets
                .iter()
                .map(|b| {
                    serde_json::json!({
                        "period": b.label,
                        "total_cost": b.summary.total_cost,
                        "total_turns": b.summary.total_turns,
                        "sessions": b.summary.entry_count,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_buckets)?);
        }
        cli::CostFormat::Csv => {
            println!("period,total_cost,total_turns,sessions");
            for b in &buckets {
                println!(
                    "{},{:.6},{},{}",
                    b.label, b.summary.total_cost, b.summary.total_turns, b.summary.entry_count
                );
            }
        }
        cli::CostFormat::Table => {
            if buckets.is_empty() {
                eprintln!("{}", fmt::dim("No cost entries found."));
                return Ok(());
            }

            let period_label = match period {
                cost::time_period::TimePeriod::Daily => "Daily",
                cost::time_period::TimePeriod::Weekly => "Weekly",
                cost::time_period::TimePeriod::Monthly => "Monthly",
            };

            eprintln!(
                "{}\n",
                fmt::bold(&format!("💰 {period_label} Cost Summary"))
            );

            let mut grand_total = 0.0;
            let mut grand_turns = 0u32;
            let mut grand_sessions = 0usize;

            for b in &buckets {
                grand_total += b.summary.total_cost;
                grand_turns += b.summary.total_turns;
                grand_sessions += b.summary.entry_count;

                let cost_str = if b.summary.total_cost < 0.01 {
                    format!("${:.4}", b.summary.total_cost)
                } else {
                    format!("${:.2}", b.summary.total_cost)
                };

                eprintln!(
                    "  {}  {}  {} turns  {} sessions",
                    fmt::cyan(&format!("{:<12}", b.label)),
                    fmt::yellow(&format!("{cost_str:>8}")),
                    b.summary.total_turns,
                    b.summary.entry_count
                );
            }

            let total_str = if grand_total < 0.01 {
                format!("${grand_total:.4}")
            } else {
                format!("${grand_total:.2}")
            };

            eprintln!();
            eprintln!(
                "  {} {total_str} across {grand_sessions} sessions, {grand_turns} turns",
                fmt::bold("Total:")
            );
        }
    }

    Ok(())
}

/// Map the CLI's grouping axis onto the reporting dimension.
fn dimension_of(axis: cli::CostGroupBy) -> cost::CostDimension {
    match axis {
        cli::CostGroupBy::Project => cost::CostDimension::Project,
        cli::CostGroupBy::User => cost::CostDimension::Actor,
        cli::CostGroupBy::Model => cost::CostDimension::Model,
        cli::CostGroupBy::Session => cost::CostDimension::Session,
    }
}

/// Map the CLI's bucket onto the reporting period.
fn period_of(period: cli::CostSummaryPeriod) -> cost::time_period::TimePeriod {
    match period {
        cli::CostSummaryPeriod::Daily => cost::time_period::TimePeriod::Daily,
        cli::CostSummaryPeriod::Weekly => cost::time_period::TimePeriod::Weekly,
        cli::CostSummaryPeriod::Monthly => cost::time_period::TimePeriod::Monthly,
    }
}

/// Print spend metered along one attribution axis.
fn cmd_costs_grouped(
    entries: &[cost::CostEntry],
    dimension: cost::CostDimension,
    format: cli::CostFormat,
) -> eyre::Result<()> {
    let groups = cost::summarize_by_dimension(entries, dimension);

    match format {
        cli::CostFormat::Json => {
            let rows: Vec<serde_json::Value> = groups
                .iter()
                .map(|g| {
                    serde_json::json!({
                        dimension.as_str(): g.key,
                        "total_cost": g.summary.total_cost,
                        "total_turns": g.summary.total_turns,
                        "sessions": g.summary.entry_count,
                        "total_tokens": g.summary.total_tokens.total(),
                        "share": g.share,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        cli::CostFormat::Csv => {
            println!(
                "{},total_cost,total_turns,sessions,total_tokens,share",
                dimension.as_str()
            );
            for g in &groups {
                println!(
                    "{},{:.6},{},{},{},{:.4}",
                    csv_field(&g.key),
                    g.summary.total_cost,
                    g.summary.total_turns,
                    g.summary.entry_count,
                    g.summary.total_tokens.total(),
                    g.share
                );
            }
        }
        cli::CostFormat::Table => {
            if groups.is_empty() {
                eprintln!("{}", fmt::dim("No cost entries found."));
                return Ok(());
            }

            eprintln!(
                "{}\n",
                fmt::bold(&format!("💰 Cost by {}", dimension.as_str()))
            );

            for g in &groups {
                eprintln!(
                    "  {}  {}  {:>5}  {} turns  {} sessions",
                    fmt::cyan(&format!("{:<28}", shorten(&g.key, 28))),
                    fmt::yellow(&format!("{:>9}", money(g.summary.total_cost))),
                    fmt::dim(&format!("{:.0}%", g.share * 100.0)),
                    g.summary.total_turns,
                    g.summary.entry_count
                );
            }

            let total: f64 = groups.iter().map(|g| g.summary.total_cost).sum();
            eprintln!();
            eprintln!(
                "  {} {} across {} {}s",
                fmt::bold("Total:"),
                money(total),
                groups.len(),
                dimension.as_str()
            );
        }
    }

    Ok(())
}

/// Print a chargeback report — what each cost centre owes, per period.
fn cmd_costs_chargeback(
    entries: &[cost::CostEntry],
    dimension: cost::CostDimension,
    period: cost::time_period::TimePeriod,
    format: cli::CostFormat,
) -> eyre::Result<()> {
    let rows = cost::chargeback(entries, dimension, period);

    match format {
        cli::CostFormat::Json => {
            let json: Vec<serde_json::Value> = rows
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "period": r.period,
                        dimension.as_str(): r.key,
                        "cost_usd": r.cost_usd,
                        "turns": r.turns,
                        "sessions": r.sessions,
                        "tokens": r.tokens.total(),
                        "share": r.share,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        cli::CostFormat::Csv => {
            println!(
                "period,{},cost_usd,turns,sessions,tokens,share",
                dimension.as_str()
            );
            for r in &rows {
                println!(
                    "{},{},{:.6},{},{},{},{:.4}",
                    r.period,
                    csv_field(&r.key),
                    r.cost_usd,
                    r.turns,
                    r.sessions,
                    r.tokens.total(),
                    r.share
                );
            }
        }
        cli::CostFormat::Table => {
            if rows.is_empty() {
                eprintln!("{}", fmt::dim("No cost entries found."));
                return Ok(());
            }

            eprintln!(
                "{}\n",
                fmt::bold(&format!("🧾 Chargeback by {}", dimension.as_str()))
            );

            let mut current = String::new();
            for r in &rows {
                if r.period != current {
                    if !current.is_empty() {
                        eprintln!();
                    }
                    eprintln!("  {}", fmt::bold(&r.period));
                    current.clone_from(&r.period);
                }
                eprintln!(
                    "    {}  {}  {:>5}  {} turns  {} sessions",
                    fmt::cyan(&format!("{:<26}", shorten(&r.key, 26))),
                    fmt::yellow(&format!("{:>9}", money(r.cost_usd))),
                    fmt::dim(&format!("{:.0}%", r.share * 100.0)),
                    r.turns,
                    r.sessions
                );
            }

            let total: f64 = rows.iter().map(|r| r.cost_usd).sum();
            eprintln!();
            eprintln!("  {} {}", fmt::bold("Charged:"), money(total));
        }
    }

    Ok(())
}

/// Print the budget hierarchy in force for a project directory.
fn cmd_costs_policy(
    dir: &std::path::Path,
    ledger: &std::path::Path,
    format: cli::CostFormat,
) -> eyre::Result<()> {
    let resolved = cost::governed_budget(
        dir,
        ledger,
        &magistr_engine::cost::BudgetConfig::none(),
        chrono::Utc::now(),
    );

    match format {
        cli::CostFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&resolved)?);
        }
        cli::CostFormat::Csv => {
            println!("scope,period,limit,spent,remaining");
            for h in &resolved.headroom {
                println!(
                    "{},{},{:.6},{:.6},{:.6}",
                    h.scope, h.period, h.limit, h.spent, h.remaining
                );
            }
        }
        cli::CostFormat::Table => {
            if !resolved.is_governed() {
                eprintln!("{}", fmt::dim("No team budget policy for this project."));
                eprintln!(
                    "{}",
                    fmt::dim("Add a `budgets` array to .magistr/team.json to publish caps.")
                );
                return Ok(());
            }

            eprintln!("{}\n", fmt::bold("🏛  Budget hierarchy"));

            for h in &resolved.headroom {
                let bar = if h.is_exhausted() {
                    fmt::red("exhausted")
                } else {
                    fmt::green(&format!("{} left", money(h.remaining)))
                };
                eprintln!(
                    "  {}  {}  {} of {}  {}",
                    fmt::cyan(&format!("{:<9}", h.scope.to_string())),
                    fmt::dim(&format!("{:<8}", h.period.to_string())),
                    money(h.spent),
                    money(h.limit),
                    bar
                );
            }

            eprintln!();
            match (resolved.effective.limit, resolved.binding) {
                (Some(limit), Some(scope)) => eprintln!(
                    "  {} {} (bound by the {scope} cap)",
                    fmt::bold("This session may spend:"),
                    money(limit)
                ),
                (Some(limit), None) => eprintln!(
                    "  {} {}",
                    fmt::bold("This session may spend:"),
                    money(limit)
                ),
                (None, _) => eprintln!("  {}", fmt::dim("No hard limit binds this session.")),
            }
        }
    }

    Ok(())
}

/// Format a dollar amount, keeping sub-cent spend legible.
fn money(amount: f64) -> String {
    if amount < 0.01 {
        format!("${amount:.4}")
    } else {
        format!("${amount:.2}")
    }
}

/// Truncate a key to fit a column, with an ellipsis when it does not.
fn shorten(key: &str, width: usize) -> String {
    if key.chars().count() <= width {
        return key.to_string();
    }
    let kept: String = key.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// Escape a CSV field — quote it when it carries a separator.
fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;

mod cmd_costs;
mod cmd_permissions;
mod cmd_sessions;
mod handoff;

pub use cmd_costs::cmd_costs;
pub use cmd_permissions::cmd_permissions;
pub use cmd_sessions::cmd_sessions;
pub use handoff::handoff_session;
