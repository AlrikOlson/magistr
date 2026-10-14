//! Resident sessions share admitted project/workspace resources while their
//! workflow executions and source journals remain separate. This is the host
//! factory's resource pool, not another provider or process lifecycle.
use super::{
    InteractiveHost,
    interactive_host::{Resources, combine},
    session_registry::{Finish, Open, Opening, SessionRegistry},
    workflow_client::EngineClient,
};
use crate::agent::AgentConfig;
use color_eyre::eyre;
use magistr_domain::identity::SessionAddress;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Default)]
struct Admissions {
    roots: BTreeMap<PathBuf, (String, Resources)>,
}

impl Admissions {
    async fn session(
        &mut self,
        config: &AgentConfig,
        address: &SessionAddress,
    ) -> eyre::Result<Resources> {
        let root = PathBuf::from(&address.workspace);
        if !self.roots.contains_key(&root) {
            let shared = self
                .roots
                .values()
                .find(|(project, _)| project == &address.project)
                .and_then(|(_, resources)| resources.engines.writer().cloned());
            let serve = shared.is_none();
            let config_root = root.clone();
            let execution = address.session.clone();
            let engines = tokio::task::spawn_blocking(move || match shared {
                Some(writer) => writer
                    .for_working_copy(&config_root, &execution)
                    .map(EngineClient::attach)
                    .map_err(eyre::Report::from),
                None => EngineClient::with_config(magistr_core::evidence::writer_config_from_env(
                    &config_root,
                ))
                .map_err(|error| match &error {
                    super::workflow_client::EngineError::Admission { source, .. }
                        if source.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        eyre::eyre!("Another Magistr session has this project open.")
                    }
                    _ => eyre::Report::from(error),
                }),
            })
            .await??;
            let resources = Resources::open_served(engines, config.inputs(), serve).await?;
            if !resources.is_admitted() {
                let refusal = resources.source_report();
                let cleanup = resources.close().await;
                return Err(eyre::eyre!(
                    "The working copy could not be opened: {refusal}. {}",
                    cleanup.err().unwrap_or_default()
                ));
            }
            self.roots
                .insert(root.clone(), (address.project.clone(), resources));
        }
        let resources = self
            .roots
            .get(&root)
            .ok_or_else(|| eyre::eyre!("The working copy was not admitted."))?;
        let writer = resources
            .1
            .engines
            .writer()
            .cloned()
            .ok_or_else(|| eyre::eyre!("The project writer was not admitted."))?;
        let execution = address.session.clone();
        let engines = tokio::task::spawn_blocking(move || {
            writer
                .for_working_copy(&root, &execution)
                .map(EngineClient::attach)
        })
        .await??;
        Ok(resources.1.fork(engines))
    }

    async fn close(&mut self) -> Result<(), String> {
        let mut closes = tokio::task::JoinSet::new();
        for (_, (_, resources)) in std::mem::take(&mut self.roots) {
            closes.spawn(resources.close_last());
        }
        let mut result = Ok(());
        while let Some(closed) = closes.join_next().await {
            result = combine(
                result,
                closed
                    .map_err(|error| error.to_string())
                    .and_then(|result| result),
            );
        }
        result
    }
}

impl SessionRegistry {
    /// Start the application's one session owner. Configuration and admission run
    /// away from the surface; provider lifetime remains in InteractiveHost.
    #[must_use]
    pub fn start(
        configure: impl FnMut(&Path) -> eyre::Result<AgentConfig> + Send + 'static,
    ) -> Self {
        let pool = Arc::new(Mutex::new(Admissions::default()));
        let configure = Arc::new(std::sync::Mutex::new(configure));
        let admissions = Arc::clone(&pool);
        let open: Open<InteractiveHost> = Box::new(move |opening: Opening| {
            let configure = Arc::clone(&configure);
            let admissions = Arc::clone(&admissions);
            Box::pin(async move {
                let root = PathBuf::from(&opening.address.workspace);
                let mut config = tokio::task::spawn_blocking(move || {
                    let mut configure = configure
                        .lock()
                        .map_err(|_| eyre::eyre!("The session settings could not be read."))?;
                    let config = configure(&root)?;
                    opening.choice.apply(config).map_err(eyre::Report::from)
                })
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| format!("{error:#}"))?;
                config.execution_id = Some(opening.address.session.clone());
                let configured_root = config
                    .cwd
                    .canonicalize()
                    .map_err(|error| error.to_string())?;
                if configured_root != Path::new(&opening.address.workspace) {
                    return Err("The session configuration names another working copy.".into());
                }
                let resources = admissions
                    .lock()
                    .await
                    .session(&config, &opening.address)
                    .await
                    .map_err(|error| format!("{error:#}"))?;
                opening
                    .observation
                    .set_reader(resources.engines.reader().clone());
                let opened = InteractiveHost::open_admitted(
                    config,
                    resources,
                    opening.inbox,
                    opening.commands,
                )
                .await
                .map_err(|error| format!("{error:#}"))?;
                Ok(opened)
            })
        });
        let finish: Finish =
            Box::new(move || Box::pin(async move { pool.lock().await.close().await }));
        Self::start_with(open, finish)
    }
}

#[cfg(test)]
#[path = "session_admissions.tests.rs"]
mod tests;
