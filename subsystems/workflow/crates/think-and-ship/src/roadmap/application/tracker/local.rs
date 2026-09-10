use super::{SetupOutcome, SetupRequest};
use crate::cli::PUSH_INTERVAL_ENV;
use anyhow::Result;
pub fn setup_local(
    engine: &mut crate::roadmap::engine::RoadmapEngine,
    req: &SetupRequest,
    data_dir: &std::path::Path,
    project_id: &str,
    cwd: &std::path::Path,
) -> Result<SetupOutcome> {
    let mut outcome = SetupOutcome::default();
    let provider = req.provider.trim().to_ascii_lowercase();
    let target = req.into.trim();

    // ── 2. Local config, only now that the destination is real ────────────
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    if !req.dry_run {
        crate::tracker::config::enable(data_dir, project_id, &provider, target, &now)?;
        // The roof's name, when the caller chose one. After enable(), so the
        // explicit choice lands on top of the carried-through config; absent
        // means keep — enable() already preserves a prior name (1c14ceb).
        if let Some(name) = req.initiative.as_deref() {
            crate::tracker::config::set_initiative(data_dir, project_id, name, &now)?;
        }
        // Consent was granted a moment ago, to an engine that may be serving a
        // LIVE MCP session (the tracker_setup tool hands us its engine). Arm the
        // inheritance now, from the config we just wrote, or every chunk created
        // before the next restart would still be born invisible — the exact gap
        // this is fixing, reintroduced one restart wide.
        let armed =
            crate::tracker::inherited_opt_in(&crate::tracker::config::load(data_dir, project_id))
                .map(str::to_string);
        engine.set_opt_in_inheritance(armed);
    }
    outcome.mirroring_enabled = !req.dry_run;

    // ── 3. Bulk include ───────────────────────────────────────────────────
    let wanted: Vec<String> = {
        let already: std::collections::HashSet<String> = engine
            .chunks_opted_in(&provider)
            .into_iter()
            .map(|c| c.id.clone())
            .collect();
        let band = req.band.as_deref().map(str::trim);
        engine
            .roadmap()
            .chunks
            .iter()
            // ACTIVE only, by the shared rule — the same one a newly-born chunk
            // is judged by, so the bulk include and the inheritance can never
            // disagree about what counts as work.
            .filter(|c| c.status.is_active())
            .filter(|c| band.is_none_or(|b| crate::infra::coerce::priority_band(c.priority) == b))
            .filter(|c| {
                if already.contains(&c.id) {
                    outcome.already_included += 1;
                    false
                } else {
                    true
                }
            })
            .map(|c| c.id.clone())
            .collect()
    };

    for id in &wanted {
        if !req.dry_run {
            engine
                .set_tracker_opt_in(id, &provider, true)
                .map_err(anyhow::Error::msg)?;
        }
        outcome.included.push(id.clone());
    }

    // ── 4. Auto-push ──────────────────────────────────────────────────────
    //
    // SEARCH for the entry rather than guessing its path. The guess resolved by
    // IDE marker directory, so a repo with a `.cursor/` folder pointed at
    // `.cursor/mcp.json` even when the live entry was in `~/.claude.json` — and
    // then reported "run init here", which would have created another config
    // instead of touching the one in use.
    if req.push_secs > 0 {
        let secs = req.push_secs.to_string();
        match crate::cli::setup::find_server_entry(cwd) {
            None => outcome.auto_push_no_entry = true,
            Some(entry_at) => {
                match crate::cli::setup::merge_server_env(
                    &entry_at,
                    PUSH_INTERVAL_ENV,
                    &secs,
                    req.dry_run,
                )? {
                    crate::cli::setup::EnvMerge::Written { .. } => {
                        outcome.auto_push = Some(secs);
                        outcome.auto_push_at = Some(entry_at.describe());
                    }
                    crate::cli::setup::EnvMerge::AlreadySet => {
                        outcome.auto_push = Some(secs);
                        outcome.auto_push_at = Some(entry_at.describe());
                        outcome.auto_push_unchanged = true;
                    }
                    crate::cli::setup::EnvMerge::ExternalConfig { at } => {
                        outcome.auto_push_manual = Some(format!(
                            "Your think-and-ship entry is in {at}. That file holds \
                             every project you have, and rewriting it to set one \
                             variable would reorder and perturb the rest — so add \
                             this by hand:\n      \"{PUSH_INTERVAL_ENV}\": \"{secs}\"",
                        ));
                    }
                    // Only reachable if the file changed under us between the
                    // search and the write.
                    crate::cli::setup::EnvMerge::NoServerEntry => {
                        outcome.auto_push_no_entry = true;
                    }
                }
            }
        }
    }

    Ok(outcome)
}
