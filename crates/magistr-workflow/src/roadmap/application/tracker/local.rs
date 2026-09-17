use super::{SetupOutcome, SetupRequest};
use anyhow::Result;
pub fn setup_local(
    engine: &mut crate::roadmap::engine::RoadmapEngine,
    req: &SetupRequest,
    data_dir: &std::path::Path,
    project_id: &str,
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

    // There is no step 4. The unattended push cadence belonged to the retired
    // standalone binary; Magistr composes these engines in process and runs
    // no cadence, so setup records consent and scope and sends nothing.
    Ok(outcome)
}
