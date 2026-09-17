//! What this server is, whoever answers: the handshake and the core tool
//! surface as one definition, served by the rmcp handler over an admitted
//! workspace and by the bridge itself while no owner is reachable, so a
//! client discovers the same server either way.

use std::borrow::Cow;

use rmcp::model::{
    Implementation, InitializeResult, ListToolsResult, ProtocolVersion, ServerCapabilities,
    ServerInfo,
};
use serde_json::Value;

use super::tool_surface;
use crate::mcp_tools::protocol_cap;

/// The protocol eras this server will negotiate or advertise: everything
/// below 2026-07-28, as `magistr mcp-serve` caps them too, so every Claude
/// client accepts the result shapes.
const SERVED_PROTOCOL_VERSIONS: &[ProtocolVersion] = &[
    ProtocolVersion::V_2024_11_05,
    ProtocolVersion::V_2025_03_26,
    ProtocolVersion::V_2025_06_18,
];

/// The declared capability of one workspace server, independent of any
/// owner: the same `initialize` result and the same core tools the admitted
/// handler serves. Plugin tools are known only to an owner and are appended
/// by the handler.
#[derive(Clone, Copy, Debug)]
pub(super) struct Capability {
    offers_script: bool,
}

impl Capability {
    pub(super) const fn new(offers_script: bool) -> Self {
        Self { offers_script }
    }

    /// Discovery information before negotiation: tools and tasks, the
    /// instructions, this binary's version and the default served era.
    pub(super) fn server_info() -> ServerInfo {
        // SEP-2663: declaring tasks is what makes `tasks/get` reachable at
        // all — rmcp answers the tasks/* methods with -32601 unless the
        // server advertised the extension.
        let mut info = ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks()
                .build(),
        )
        .with_instructions(tool_surface::INSTRUCTIONS)
        .with_server_info(Implementation::new("magistr", env!("CARGO_PKG_VERSION")));
        info.protocol_version = ProtocolVersion::V_2025_06_18;
        info
    }

    pub(super) fn supported_protocol_versions() -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(SERVED_PROTOCOL_VERSIONS)
    }

    /// The `initialize` result for a client that asked for `requested`,
    /// negotiated as the served handler negotiates it: the request is capped
    /// below 2026-07-28 first, a served era is echoed, anything else gets
    /// the default era.
    pub(super) fn initialize(requested: &ProtocolVersion) -> InitializeResult {
        let mut info = Self::server_info();
        let requested = protocol_cap::capped(requested.clone());
        if SERVED_PROTOCOL_VERSIONS.contains(&requested) {
            info.protocol_version = requested;
        }
        info
    }

    /// The core `tools/list` result as a served era carries it: no
    /// `resultType` discriminator, no cursor.
    pub(super) fn core_listing(self) -> Value {
        let mut listing =
            ListToolsResult::with_all_items(tool_surface::core_tools(self.offers_script));
        listing.result_type = None;
        serde_json::to_value(listing).unwrap_or_else(|_| Value::Object(Default::default()))
    }
}

#[cfg(test)]
#[path = "capability.tests.rs"]
mod tests;
