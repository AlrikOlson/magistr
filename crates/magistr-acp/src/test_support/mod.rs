//! In-process ACP peers and their observations exercise the transport without a renderer.
mod acp_observed;
mod fake_acp_agent;
pub use acp_observed::AcpObserved;
pub use fake_acp_agent::FakeAcpAgent;
