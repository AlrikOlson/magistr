use std::path::Path;

use crate::cli;
use crate::error::{self, ResultExt};
use crate::session;
use crate::team::auth::require_verified;
use crate::team::{Identity, TeamConfig, is_ci_env, resolve_role};
use crate::utils::format as fmt;
use color_eyre::eyre;

#[allow(clippy::too_many_arguments)] // mirrors the clap surface one-to-one
pub fn cmd_sessions(
    project: Option<&str>,
    since_days: Option<u32>,
    limit: usize,
    format: cli::SessionFormat,
    tag_args: Option<Vec<String>>,
    purge_days: Option<u32>,
    owner: Option<&str>,
    handoff_args: Option<Vec<String>>,
) -> eyre::Result<()> {
    let db_path = session::default_db_path().ok_or_else(|| {
        error::MagistrError::Config("could not determine data directory (HOME not set)".into())
    })?;

    let conn = session::open_db(&db_path).suggest()?;

    // Handle --tag <session-id> <tag>
    if let Some(args) = tag_args
        && args.len() == 2
    {
        session::tag_session(&conn, &args[0], &args[1]).suggest()?;
        eprintln!(
            "{} Tagged session {} with {}",
            fmt::green("✓"),
            fmt::bold(&args[0]),
            fmt::cyan(&args[1])
        );
        return Ok(());
    }

    // Handle --handoff <session-id> <new-owner>
    if let Some(args) = handoff_args
        && args.len() == 2
    {
        let dir = Path::new(".");
        let identity = Identity::resolve(dir);
        let config = TeamConfig::load(dir)?;
        // Ahead of the role, as in `attach`: a role resolved from an
        // identity nobody vouched for is no basis for a handoff.
        require_verified(config.as_ref(), &identity)?;
        let role = resolve_role(config.as_ref(), &identity, is_ci_env());
        let audit_log = crate::permissions::audit_log_path();
        let retention = crate::permissions::load_config(dir)
            .unwrap_or_default()
            .retention;
        let receipt = super::handoff_session(
            &conn,
            &args[0],
            &args[1],
            &identity,
            role,
            audit_log.as_deref(),
            &retention,
        )
        .suggest()?;
        eprintln!("{} {receipt}", fmt::green("✓"));
        return Ok(());
    }

    // Handle --purge <days>
    if let Some(days) = purge_days {
        let policy = session::RetentionPolicy { max_age_days: days };
        let purged = session::purge_old_sessions(&conn, &policy).suggest()?;
        eprintln!(
            "{}",
            fmt::success(&format!("Purged {purged} sessions older than {days} days"))
        );
        return Ok(());
    }

    // Build filter
    let mut filter = session::SessionFilter::new().with_limit(limit);

    if let Some(proj) = project {
        filter = filter.with_project(proj);
    }

    if let Some(days) = since_days {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
        filter = filter.with_since(cutoff);
    }

    if let Some(who) = owner {
        filter = filter.with_owner(who);
    }

    let records = session::query_sessions(&conn, &filter).suggest()?;

    match format {
        cli::SessionFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&records)?);
        }
        cli::SessionFormat::Table => {
            if records.is_empty() {
                eprintln!("{}", fmt::dim("No sessions found."));
                eprintln!(
                    "{}",
                    fmt::dim("Session data is recorded when agent sessions complete.")
                );
                return Ok(());
            }

            let summary = session::SessionSummary::from_records(&records);

            eprintln!("{}\n", fmt::bold("📋 Session History"));

            for record in &records {
                let ts = record.started_at.format("%Y-%m-%d %H:%M");
                let owner_str = record
                    .owner
                    .as_deref()
                    .map(|o| format!("  {}", fmt::cyan(&format!("@{o}"))))
                    .unwrap_or_default();
                let tags_str = if record.tags.is_empty() {
                    String::new()
                } else {
                    format!("  {}", fmt::cyan(&format!("[{}]", record.tags.join(", "))))
                };
                let summary_str = record
                    .summary
                    .as_deref()
                    .map(|s| {
                        let truncated: String = s.chars().take(60).collect();
                        format!("  {}", fmt::dim(&truncated))
                    })
                    .unwrap_or_default();

                eprintln!(
                    "  {}  {}  {} turns  {}  {} files  {}{owner_str}{tags_str}{summary_str}",
                    fmt::dim(&ts.to_string()),
                    fmt::yellow(&record.cost_display()),
                    record.turns,
                    record.duration_display(),
                    record.file_count(),
                    fmt::dim(&record.project),
                );
            }

            eprintln!();
            eprintln!(
                "  {} ${:.2} across {} sessions, {} turns",
                fmt::bold("Total:"),
                summary.total_cost,
                summary.count,
                summary.total_turns
            );
        }
    }

    Ok(())
}
