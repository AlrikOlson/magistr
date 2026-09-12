use magistr_workflow::infra::{Broadcaster, Domain, Persistence};
use magistr_workflow::roadmap::RoadmapEngine;
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::ship::persistence::{
    Persistence as ShipPersistence, PersistenceConfig as ShipPersistenceConfig,
};
use magistr_workflow::signal::SignalEngine;
use magistr_workflow::think::config::load_config;
use magistr_workflow::think::engine::ReasoningServer;

use super::native_config::NativeConfig;

/// Open the four think-and-ship engines on one shared store.
///
/// The one composition root for engines in Magistr, reached only through
/// [`super::ProjectAuthority`] (a writer, or the transport's family
/// services) and [`super::NativeReader`] (an observer). Persistence follows
/// `config`; every engine is keyed by `project_id`. When `broadcaster` is
/// given (the service that binds the NDJSON socket), every engine emits
/// through it; a reader passes `None` and subscribes to that socket instead.
#[must_use]
pub(crate) fn open_engines(
    config: &NativeConfig,
    project_id: String,
    broadcaster: Option<Broadcaster>,
) -> (ReasoningServer, ShipEngine, RoadmapEngine, SignalEngine) {
    let persist = config.persistence_config();

    let mut roadmap = open_roadmap(config, project_id.clone());
    let mut signal = open_signal(config, project_id.clone());
    let mut ship = open_ship(config, project_id.clone());
    let mut think_config = load_config();
    think_config.persistence = persist;
    // The socket is bound (or not) by the caller; the think engine must not
    // try to bind a second one from the environment.
    think_config.broadcast.path = None;
    let mut think = ReasoningServer::new_for_project(think_config, project_id);

    if let Some(b) = broadcaster {
        think = think.with_broadcaster(
            magistr_workflow::think::broadcast::Broadcaster::from_engine(b.clone()),
        );
        ship = ship.with_broadcaster(magistr_workflow::ship::broadcast::Broadcaster::from_engine(
            b.clone(),
        ));
        roadmap = roadmap.with_broadcaster(
            magistr_workflow::roadmap::broadcast::Broadcaster::from_engine(b.clone()),
        );
        signal = signal
            .with_broadcaster(magistr_workflow::signal::broadcast::Broadcaster::from_engine(b));
    }

    (think, ship, roadmap, signal)
}

/// A fresh roadmap engine loaded from the store.
///
/// Quiet since think-and-ship 0.5.1: `with_persistence` narrates the load
/// (`loaded roadmap with N chunk(s) from disk`) on `tracing::info!` and
/// its one-time repairs on `tracing::warn!`, so nothing reaches stderr
/// unless the host's subscriber asks for it — `init_tracing` defaults to
/// `warn`, which is why `magistr guard` and `magistr roadmap export` print
/// no banner, and why a repair still shows. A caller inside a raw-mode
/// terminal therefore only has to own stderr for what its own subscriber
/// lets through (the cockpit's `RealTerminal::stdio` does).
pub(crate) fn open_roadmap(config: &NativeConfig, project_id: String) -> RoadmapEngine {
    RoadmapEngine::new(project_id).with_persistence(Persistence::new(
        &config.persistence_config(),
        Domain::Roadmap,
    ))
}

/// A fresh signal engine loaded from the store. **Not** quiet, unlike
/// [`open_roadmap`]: through 0.5.2 the signal load still `eprintln!`s
/// `loaded N signal(s) from disk` whenever the store holds signals
/// (`tas-load-banner-ship-signal`), so a caller inside a raw-mode terminal
/// must own stderr for the session.
pub(crate) fn open_signal(config: &NativeConfig, project_id: String) -> SignalEngine {
    SignalEngine::new(project_id).with_persistence(Persistence::new(
        &config.persistence_config(),
        Domain::Signal,
    ))
}

/// A fresh ship engine loaded from the store. **Not** quiet either:
/// through 0.5.2 the ship load `eprintln!`s `ship: loaded N task(s) from
/// disk` whenever an objective is on disk — the banner every headless
/// engine open still prints once the roadmap's went to tracing
/// (`tas-load-banner-ship-signal`). Same raw-mode caveat as [`open_signal`].
pub(crate) fn open_ship(config: &NativeConfig, project_id: String) -> ShipEngine {
    let persist = config.persistence_config();
    ShipEngine::new(project_id).with_persistence(ShipPersistence::new(&ShipPersistenceConfig {
        enabled: persist.enabled,
        data_dir: persist.data_dir,
    }))
}
