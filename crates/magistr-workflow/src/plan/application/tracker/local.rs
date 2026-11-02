use super::{SetupOutcome, SetupRequest, TrackerSetupError};

pub fn setup_local(
    engine: &mut crate::plan::engine::PlanEngine,
    req: &SetupRequest,
    data_dir: &std::path::Path,
    project_id: &str,
) -> Result<SetupOutcome, TrackerSetupError> {
    let provider = req.provider.trim().to_ascii_lowercase();
    let band = req.band.as_deref().map(str::trim);
    let select = |plan: &crate::plan::domain::Plan| {
        let mut already = 0;
        let wanted = plan
            .tasks
            .iter()
            .filter(|task| task.status.is_active())
            .filter(|task| {
                band.is_none_or(|band| crate::infra::coerce::priority_band(task.priority) == band)
            })
            .filter(|task| {
                if plan
                    .tracker_opt_ins
                    .iter()
                    .any(|opt| opt.task_id == task.id && opt.provider == provider && opt.enabled)
                {
                    already += 1;
                    false
                } else {
                    true
                }
            })
            .map(|task| task.id.clone())
            .collect::<Vec<_>>();
        (wanted, already)
    };
    if req.dry_run {
        let (included, already_included) = select(engine.plan());
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
            let (wanted, already) = select(edit.plan());
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
        plan_source: Some(included.source),
        ..SetupOutcome::default()
    })
}
