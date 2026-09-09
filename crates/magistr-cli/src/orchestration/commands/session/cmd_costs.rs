use super::*;
use crate::cli;
use crate::cost;
use crate::error::{self, ResultExt};
use crate::utils::format as fmt;
use color_eyre::eyre;

pub fn cmd_costs(query: &cli::CostQuery) -> eyre::Result<()> {
    let path = cost::costs_path::costs_path().ok_or_else(|| {
        error::MagistrError::Config("could not determine data directory (HOME not set)".into())
    })?;

    // `--policy` asks about policy, not history: it reads the roster, not
    // the filters below.
    if let Some(dir) = query.policy.as_deref() {
        return cmd_costs_policy(dir, &path, query.format);
    }

    let entries = cost::storage::load_entries(&path).suggest()?;

    // Apply filters
    let mut filtered = entries;

    if let Some(proj) = query.project.as_deref() {
        filtered = cost::filter_by_project::filter_by_project(&filtered, proj);
    }

    if let Some(days) = query.since_days {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
        filtered = cost::filter_since::filter_since(&filtered, cutoff);
    }

    // Sort by timestamp descending (most recent first)
    filtered.sort_by_key(|e| std::cmp::Reverse(e.timestamp));

    // Limit output
    filtered.truncate(query.limit);

    // Handle time-bucketed summary mode
    if let Some(period) = query.summary {
        let tp = match period {
            cli::CostSummaryPeriod::Daily => cost::time_period::TimePeriod::Daily,
            cli::CostSummaryPeriod::Weekly => cost::time_period::TimePeriod::Weekly,
            cli::CostSummaryPeriod::Monthly => cost::time_period::TimePeriod::Monthly,
        };
        return cmd_costs_summary(&filtered, tp, query.format);
    }

    if query.chargeback {
        let dimension = query
            .group_by
            .map_or(cost::CostDimension::Actor, dimension_of);
        let period = period_of(
            query
                .chargeback_period
                .unwrap_or(cli::CostSummaryPeriod::Monthly),
        );
        return cmd_costs_chargeback(&filtered, dimension, period, query.format);
    }

    if let Some(axis) = query.group_by {
        return cmd_costs_grouped(&filtered, dimension_of(axis), query.format);
    }

    // Handle format-specific output
    match query.format {
        cli::CostFormat::Json => {
            let summary = cost::summarize_costs::summarize(&filtered);
            let envelope = serde_json::json!({
                "entries": filtered,
                "summary": {
                    "total_cost": summary.total_cost,
                    "total_turns": summary.total_turns,
                    "entry_count": summary.entry_count,
                },
            });
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        cli::CostFormat::Csv => {
            print!("{}", cost::csv_export::to_csv(&filtered));
        }
        cli::CostFormat::Table => {
            if filtered.is_empty() {
                eprintln!("{}", fmt::dim("No cost entries found."));
                eprintln!(
                    "{}",
                    fmt::dim("Cost data is recorded when agent sessions complete.")
                );
                return Ok(());
            }

            let summary = cost::summarize_costs::summarize(&filtered);

            eprintln!("{}\n", fmt::bold("💰 Cost History"));

            for entry in &filtered {
                let ts = entry.timestamp.format("%Y-%m-%d %H:%M");
                let duration_secs = entry.duration_ms / 1000;
                let duration = if duration_secs < 60 {
                    format!("{duration_secs}s")
                } else {
                    format!("{}m {:02}s", duration_secs / 60, duration_secs % 60)
                };

                eprintln!(
                    "  {}  {}  {} turns  {}  {}",
                    fmt::dim(&ts.to_string()),
                    fmt::yellow(&format!("${:.4}", entry.cost_usd)),
                    entry.turns,
                    duration,
                    fmt::dim(&entry.project)
                );
            }

            eprintln!();
            eprintln!(
                "  {} {} across {} sessions, {} turns",
                fmt::bold("Total:"),
                summary.cost_display(),
                summary.entry_count,
                summary.total_turns
            );
        }
    }

    Ok(())
}
