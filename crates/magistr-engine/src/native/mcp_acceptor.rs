//! How a host serves an endpoint-owned MCP connection.

use std::{future::Future, pin::Pin, sync::Arc};

use tokio_util::sync::CancellationToken;

#[cfg(any(unix, windows))]
type Attached = super::LocalStream;
#[cfg(not(any(unix, windows)))]
type Attached = ();

/// A complete attached MCP session. The endpoint owns this future and
/// cancels its token on shutdown. An acceptor must stop and join any service
/// task it started before returning; it must not detach that task. The
/// engine owns connection lifetime without depending on MCP itself.
pub type McpAcceptor = Arc<
    dyn Fn(
            Attached,
            CancellationToken,
            Option<super::NativeEngines>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;
