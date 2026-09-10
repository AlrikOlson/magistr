//! The one trait a model driver implements.

use agent_client_protocol::BoxFuture;
use agent_client_protocol::schema::v1::StopReason;

use crate::{AgentError, ModelChoice, ProviderCapabilities, SessionOpened, SessionStart, Turn};

/// How a model is driven. The scaffolding around a `Provider` owns everything
/// ACP-shaped — the session registry, the per-session turn queue, the
/// permission relay, cancellation and `session/load` — so a provider only
/// answers three questions: which models it offers, how a session starts, and
/// what happens during one turn.
///
/// Every method returns a boxed future so the trait is usable as
/// `Arc<dyn Provider>`: the provider is chosen by name at runtime
/// (`magistr acp-agent --provider <name>`).
pub trait Provider: Send + Sync + 'static {
    /// The provider's name, as `magistr acp-agent --provider` selects it and
    /// as `initialize._meta.magistr.provider` reports it.
    fn name(&self) -> &str;

    /// The models this provider offers. The first is the default a new
    /// session starts on; an empty list means the provider has no picker.
    fn models(&self) -> Vec<ModelChoice>;

    /// The guarantees the adapter implements. Defaults advertise no optional
    /// operations; providers must opt into durable load and content support.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    /// Open (or, when `session.resume` is set, restore) a session. Answers
    /// the id the session is known by from here on — the proposed one, or the
    /// runtime's own — and the history to replay to the client before the
    /// session is reported open (empty for a new session).
    fn start(&self, session: SessionStart) -> BoxFuture<'_, Result<SessionOpened, AgentError>>;

    /// Whether cancellation requires the provider to confirm a terminal
    /// runtime state. Opting in preserves the provider's stop reason or error
    /// and disables the worker's generic cancellation grace period. Such a
    /// provider must bound its own wait and return an explicit error if the
    /// runtime rejects interruption or never confirms that the turn ended.
    ///
    /// The default keeps the existing prompt cancellation behavior for
    /// providers whose work can safely be dropped after the worker's grace.
    fn requires_terminal_cancellation(&self) -> bool {
        self.capabilities().terminal_cancellation_ack
    }

    /// Run one turn. The [`Turn`] is the provider's handle on the wire: it
    /// streams `session/update` notifications, relays permission requests to
    /// the client, and carries the cancellation the client can ask for.
    ///
    /// A provider observes [`Turn::cancelled`] and returns
    /// [`StopReason::Cancelled`] promptly; the scaffolding reports a cancelled
    /// turn as cancelled whatever the provider returns, and drops a provider
    /// that keeps running past a grace period. Providers opting into
    /// [`Self::requires_terminal_cancellation`] instead own the bounded wait
    /// for runtime completion and their result is preserved.
    fn prompt(&self, turn: Turn) -> BoxFuture<'_, Result<StopReason, AgentError>>;
}
