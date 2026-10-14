use super::{SetupOutcome, SetupRequest, TrackerSetupError};

pub fn setup_local(
    engine: &mut crate::roadmap::engine::RoadmapEngine,
    req: &SetupRequest,
    data_dir: &std::path::Path,
    project_id: &str,
) -> Result<SetupOutcome, TrackerSetupError> {
    let provider = req.provider.trim().to_ascii_lowercase();
    let band = req.band.as_deref().map(str::trim);
    let select = |roadmap: &crate::roadmap::domain::Roadmap| {
        let mut already = 0;
        let wanted = roadmap
            .chunks
            .iter()
            .filter(|chunk| chunk.status.is_active())
            .filter(|chunk| {
                band.is_none_or(|band| crate::infra::coerce::priority_band(chunk.priority) == band)
            })
            .filter(|chunk| {
                if roadmap
                    .tracker_opt_ins
                    .iter()
                    .any(|opt| opt.chunk_id == chunk.id && opt.provider == provider && opt.enabled)
                {
                    already += 1;
                    false
                } else {
                    true
                }
            })
            .map(|chunk| chunk.id.clone())
            .collect::<Vec<_>>();
        (wanted, already)
    };
    if req.dry_run {
        let (included, already_included) = select(engine.roadmap());
        return Ok(SetupOutcome {
            included,
            already_included,
            ..SetupOutcome::default()
        });
    }
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut config =
        crate::tracker::config::enable(data_dir, project_id, &provider, req.into.trim(), &now)
            .map_err(TrackerSetupError::config)?;
    if let Some(name) = req.initiative.as_deref() {
        config = crate::tracker::config::set_initiative(data_dir, project_id, name, &now)
            .map_err(|error| TrackerSetupError::config(error).partial())?;
    }
    engine.set_opt_in_inheritance(crate::tracker::inherited_opt_in(&config).map(str::to_owned));
    let included = engine
        .mutate(|edit| {
            let (wanted, already) = select(edit.roadmap());
            for id in &wanted {
                edit.set_tracker_opt_in(id, &provider, true)?;
            }
            Ok((wanted, already))
        })
        .map_err(|error| TrackerSetupError::mutation(error).partial())?;
    Ok(SetupOutcome {
        mirroring_enabled: true,
        included: included.value.0,
        already_included: included.value.1,
        roadmap_source: Some(included.source),
        ..SetupOutcome::default()
    })
}
