//! The engines, opened before the terminal is taken over.

use std::path::{Path, PathBuf};
use std::time::Duration;

use magistr_engine::{NativeEngines, NativeReader, ProjectAuthority, TasConfig, WriterRole};
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
/// opening reads the store and may fail, and that must happen on a normal
/// terminal, never inside the alternate screen. [`EngineClient::open`]
/// admits this process as the project's host and holds the writer — one
/// cockpit or loop per project and store, beside the agent's own
/// `magistr mcp-serve` in the other role — while [`EngineClient::observe`]
/// holds a reader only, so a viewer never contends with a running host.
#[derive(Clone, Debug)]
pub struct EngineClient {
    reader: NativeReader,
    /// The admitted writer, when this client was opened or attached as a
    /// host rather than as an observer.
    writer: Option<NativeEngines>,
    socket: PathBuf,
    attached: FeedAttached,
}

impl EngineClient {
    /// Admit this process as the host of the project at `root` and open its
    /// engines, the store resolved from the process environment the way
    /// `magistr mcp-serve` resolves it.
    ///
    /// # Errors
    ///
    /// `root` is not a directory, the store's directory cannot be created,
    /// or the host lease is refused because another cockpit or loop holds
    /// this project.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, EngineError> {
        Self::with_config(TasConfig::from_env(root))
    }

    /// [`Self::open`] on an explicit store configuration.
    ///
    /// # Errors
    ///
    /// As [`Self::open`].
    pub fn with_config(config: TasConfig) -> Result<Self, EngineError> {
        Self::store_ready(&config)?;
        let engines = ProjectAuthority::admit(&config, WriterRole::Host)
            .map_err(|source| EngineError::Admission {
                root: config.project_root().to_path_buf(),
                source,
            })?
            .open();
        Ok(Self::attach(engines))
    }

    /// Observe the project at `root` without admission: a reader for the
    /// seed, the feed and the re-seed timer, and no writer. The launcher and
    /// the bare roadmap cockpit open this way, so they run beside a loop or
    /// a `run` that holds the host lease.
    ///
    /// # Errors
    ///
    /// `root` is not a directory, or the store's directory cannot be
    /// created.
    pub fn observe(root: impl Into<PathBuf>) -> Result<Self, EngineError> {
        Self::observe_config(TasConfig::from_env(root))
    }

    /// [`Self::observe`] on an explicit store configuration.
    ///
    /// # Errors
    ///
    /// As [`Self::observe`].
    pub fn observe_config(config: TasConfig) -> Result<Self, EngineError> {
        Self::store_ready(&config)?;
        Ok(Self {
            socket: config.feed_socket_path(),
            reader: NativeReader::open(&config),
            writer: None,
            attached: FeedAttached::new(),
        })
    }

    /// The client over a writer the entry point already admitted — the loop
    /// driver's, so a cockpit run beside a loop never opens a second cached
    /// writer on the store.
    #[must_use]
    pub fn attach(engines: NativeEngines) -> Self {
        Self {
            socket: engines.config().feed_socket_path(),
            reader: engines.reader(),
            writer: Some(engines),
            attached: FeedAttached::new(),
        }
    }

    fn store_ready(config: &TasConfig) -> Result<(), EngineError> {
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
        Ok(())
    }

    /// The read capability every client holds.
    #[must_use]
    pub const fn reader(&self) -> &NativeReader {
        &self.reader
    }

    /// The admitted writer, `None` for an observer.
    #[must_use]
    pub const fn writer(&self) -> Option<&NativeEngines> {
        self.writer.as_ref()
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
            self.reader.clone(),
            self.attached.clone(),
            inbox,
            Self::RESEED,
        )
    }
}

#[cfg(test)]
#[path = "client.tests.rs"]
mod tests;
