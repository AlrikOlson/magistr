use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use magistr_workflow::infra::Broadcaster;
use magistr_workflow::mcp::UnifiedService;

use super::family_services::FamilyServices;
use super::native_config::NativeConfig;
use super::native_engines::NativeEngines;
use super::native_reader::NativeReader;
use super::open_engines::open_engines;
use super::owner_generation::OwnerGeneration;
use super::owner_lease::OwnerLease;
use super::owner_socket::owner_socket_path;
use super::writer_role::WriterRole;

/// An admitted project writer: the project's native identity, its shared
/// advisory lease, the live feed when this writer publishes one,
/// and — once, by consuming the authority — the opening of the native engines.
///
/// The admission also mints the owner's [`OwnerGeneration`] and names the
/// [`owner_socket_path`] its endpoint is served on
/// ([`OwnerEndpoint`](super::OwnerEndpoint)), so a second surface on the
/// same project attaches to this writer instead of contending for it.
///
/// Every native writer in Magistr is admitted here before it can mutate:
/// the MCP transport as [`WriterRole::Service`], the cockpit, the loops and
/// one-shot commands as [`WriterRole::Host`]. Reading needs no admission; a
/// [`NativeReader`] opens the store's locked merge-on-save files directly and
/// reloads when they move.
///
/// Admission coordinates one writer per project across every host kind and
/// linked working copy. A second writer is refused before opening engines.
/// Direct store or lock-file modification bypasses cooperative admission;
/// no distributed ownership is claimed:
/// attachment is local, and the generation fences attached clients of this
/// owner, not arbitrary writers of the store.
pub struct ProjectAuthority {
    project_id: String,
    config: NativeConfig,
    role: WriterRole,
    lease: tokio::sync::Mutex<OwnerLease>,
    generation: OwnerGeneration,
    _working_copy: super::WorkingCopyLease,
    feed: OnceLock<Broadcaster>,
}

impl std::fmt::Debug for ProjectAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectAuthority")
            .field("project_id", &self.project_id)
            .field("role", &self.role)
            .field("publishing", &self.feed.get().is_some())
            .finish_non_exhaustive()
    }
}

impl ProjectAuthority {
    /// Admit `role` for the project at `config.project_root`, resolving its
    /// native identity the way the external server would.
    ///
    /// # Errors
    /// `WouldBlock` when another writer already holds this
    /// project and store; any other I/O failure creating or opening the lock
    /// file. Nothing is opened after a refusal.
    pub fn admit(config: &NativeConfig, role: WriterRole) -> io::Result<Self> {
        Self::admit_for_project(config, config.resolve_project_id(), role)
    }

    /// Admit `role` under an explicit project id.
    ///
    /// # Errors
    /// As [`Self::admit`].
    pub fn admit_for_project(
        config: &NativeConfig,
        project_id: String,
        role: WriterRole,
    ) -> io::Result<Self> {
        super::ProjectLocation::resolve(config.project_root())?;
        super::ProjectLocation::require_open(config.project_root())?;
        super::ExecutionContext::new(
            config.project_root(),
            config.execution_context().execution_id,
        )?;
        let working_copy = super::WorkingCopyLease::active(config.project_root())?;
        super::ProjectLocation::require_open(config.project_root())?;
        let mut lease = OwnerLease::acquire(config, &project_id, role)?;
        super::WorkflowUpgrade::apply(config, &project_id, &mut lease)?;
        Ok(Self {
            project_id,
            config: config.clone(),
            role,
            generation: lease.generation(),
            lease: tokio::sync::Mutex::new(lease),
            _working_copy: working_copy,
            feed: OnceLock::new(),
        })
    }

    /// Bind the live feed on `socket` before the engines open, so every
    /// engine emits through it. Locks the publisher path beside the project
    /// lock, refuses a live listener or an unbindable path, and reclaims only
    /// a refused or missing endpoint.
    ///
    /// # Errors
    /// `AddrInUse` for a live or unprobeable endpoint, `Unsupported` off Unix,
    /// the lock's own failures, or a bind failure. The project lease is
    /// released with the authority the error consumed.
    pub async fn publish(self, socket: &Path) -> io::Result<Self> {
        self.publish_feed(socket).await?;
        Ok(self)
    }

    pub(crate) async fn publish_feed(&self, socket: &Path) -> io::Result<()> {
        let mut lease = self.lease.lock().await;
        if self.feed.get().is_none() {
            let feed = lease.broadcaster(socket).await?;
            self.feed
                .set(feed)
                .map_err(|_| io::Error::other("The project feed is already running."))?;
        }
        Ok(())
    }

    /// The project every engine is keyed by.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// The store configuration the engines open on.
    #[must_use]
    pub fn config(&self) -> &NativeConfig {
        &self.config
    }

    /// Which writer class this admission is.
    #[must_use]
    pub const fn role(&self) -> WriterRole {
        self.role
    }

    /// The live feed this writer publishes, when [`Self::publish`] bound one.
    #[must_use]
    pub fn feed(&self) -> Option<&Broadcaster> {
        self.feed.get()
    }

    /// Which incarnation of the project's owner this admission is: minted in
    /// the lock file as the lease was taken, carried by every attached
    /// client's command, and refused on any command from another.
    #[must_use]
    pub const fn generation(&self) -> OwnerGeneration {
        self.generation
    }

    /// Where this owner's endpoint is served, and where a second surface
    /// on the same project attaches. Derived from the
    /// lease's own key, never announced.
    #[must_use]
    pub fn owner_socket_path(&self) -> PathBuf {
        owner_socket_path(&self.config, &self.project_id, self.role)
    }

    /// Open the native engines under this admission and hand out the write
    /// capability. The writer and every clone of it carry the lease, so the
    /// lock outlives the authority for as long as any writer does.
    ///
    /// The engines open behind the library's four family services,
    /// composed exactly as the standalone binary composes them; the writer
    /// carries out every verb through those services' own tool handlers
    /// (see [`NativeEngines`]), and [`Self::serve`] hands the same services
    /// to an MCP transport. The raw engines never leave this crate.
    #[must_use]
    pub fn open(self) -> NativeEngines {
        let mut config = self.config.clone();
        config.owner_generation = self.generation().get();
        let (think, ship, roadmap, signal) =
            open_engines(&config, self.project_id.clone(), self.feed().cloned());
        let services =
            FamilyServices::compose(think, ship, roadmap, signal, self.config.evidence.clone());
        let (think, ship, roadmap, signal) = services.engines();
        let reader = NativeReader::from_shared(
            self.project_id.clone(),
            config,
            think,
            ship,
            roadmap,
            signal,
        );
        NativeEngines::admitted(reader, Arc::new(self), services)
    }

    /// Compose an MCP adapter over an already-admitted execution view.
    #[must_use]
    pub fn mcp_service(engines: &NativeEngines) -> UnifiedService {
        engines.services().unified()
    }

    /// Open the native engines under this admission for an MCP transport:
    /// the [`UnifiedService`] that serves the four families on the wire,
    /// and the writer over the very same services, so a mutation the
    /// transport makes for itself (a gate it ran, a phase it recorded) is
    /// the same handler on the same engines as a tool call from the agent.
    #[must_use]
    pub fn serve(self) -> (UnifiedService, NativeEngines) {
        let engines = self.open();
        (Self::mcp_service(&engines), engines)
    }
}

#[cfg(test)]
#[path = "project_authority.tests.rs"]
mod tests;
