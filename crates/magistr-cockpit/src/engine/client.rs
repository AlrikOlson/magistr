//! The engines, opened before the terminal is taken over.

use std::path::{Path, PathBuf};
use std::time::Duration;

use magistr_engine::{NativeEngines, TasConfig};
use tokio::task::JoinHandle;

use super::attached::FeedAttached;
use super::error::EngineError;
use super::feed::spawn_feed;
use super::reseed::spawn_reseed;
use crate::channel::Inbox;

/// The project's think-and-ship engines, in-process, and the socket their
/// live feed attaches to.
///
/// Open it before the terminal is taken over (magistr-tui's `RealTerminal::stdio`):
/// opening reads the store and may fail, and both must happen on a normal
/// terminal, never inside the alternate screen.
#[derive(Clone, Debug)]
pub struct EngineClient {
    engines: NativeEngines,
    socket: PathBuf,
    attached: FeedAttached,
}

impl EngineClient {
    /// Open the engines for the project at `root`, the store resolved from
    /// the process environment the way `magistr mcp-serve` resolves it.
    ///
    /// # Errors
    ///
    /// `root` is not a directory, or the store's directory cannot be
    /// created.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, EngineError> {
        Self::with_config(TasConfig::from_env(root))
    }

    /// Open the engines on an explicit store configuration.
    ///
    /// # Errors
    ///
    /// The project root is not a directory, or a persistent store's
    /// directory cannot be created.
    pub fn with_config(config: TasConfig) -> Result<Self, EngineError> {
        let root = config.project_root();
        if !root.is_dir() {
            return Err(EngineError::NoProject(root.to_path_buf()));
        }
        if config.persist {
            std::fs::create_dir_all(&config.data_dir).map_err(|source| EngineError::Store {
                path: config.data_dir.clone(),
                source,
            })?;
        }
        Ok(Self {
            socket: config.feed_socket_path(),
            engines: NativeEngines::open(&config),
            attached: FeedAttached::new(),
        })
    }

    /// The engines.
    #[must_use]
    pub const fn engines(&self) -> &NativeEngines {
        &self.engines
    }

    /// The broadcast socket the feed attaches to.
    #[must_use]
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Attach the live feed: every frame on the socket and every change
    /// of attachment reaches `inbox` as `Msg::Engine`, reconnecting for as
    /// long as the loop lives. Abort the handle to stop it early.
    pub fn subscribe(&self, inbox: Inbox) -> JoinHandle<()> {
        spawn_feed(self.socket.clone(), inbox, self.attached.clone())
    }

    /// How often a cockpit with no feed attached re-reads the store.
    /// Slow on purpose: it is a safety net under the broadcast socket,
    /// not a substitute for it, and the read is neither free nor silent.
    pub const RESEED: Duration = Duration::from_secs(15);

    /// Re-read the store every [`EngineClient::RESEED`] while the feed
    /// is not attached, posting each changed read on `inbox` as
    /// `Msg::Engine` — how a cockpit opened without the loop driver sees
    /// what another process wrote. Abort the handle to stop it early;
    /// see [`spawn_reseed`] for what it declines to do.
    pub fn reseed(&self, inbox: Inbox) -> JoinHandle<()> {
        spawn_reseed(
            self.engines.clone(),
            self.attached.clone(),
            inbox,
            Self::RESEED,
        )
    }
}

#[cfg(test)]
#[path = "client.tests.rs"]
mod tests;
