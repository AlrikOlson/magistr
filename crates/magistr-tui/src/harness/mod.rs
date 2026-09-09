//! A terminal, an agent and a think-and-ship feed made of scripts: bytes
//! and resizes that arrive at chosen moments with the output captured for
//! inspection, agent deltas and feed events posted on the runtime's inbox
//! at chosen moments — and, for the session, a real ACP agent in-process
//! on the other end of the transport. The runtime runs over them exactly
//! as it runs over the real tty and the live producers, so a test can
//! drive the whole cockpit headlessly — keys, mouse, replies to the
//! startup queries, a resize storm, a burst of deltas, a job in flight, a
//! permission round-trip, the quit — and read back every byte it would
//! have sent, with the instant of every write and of every byte delivery
//! for input-to-paint measurements.

pub mod acp_observed;
pub mod capture;
pub mod deliveries;
pub mod fake_acp_agent;
pub mod fake_agent;
pub mod fake_checkpoints;
pub mod fake_engine_port;
pub mod fake_engines;
pub mod fake_input;
pub mod fake_terminal;
pub mod script;
pub mod step;
pub mod timeline;

pub use acp_observed::AcpObserved;
pub use capture::Capture;
pub use deliveries::Deliveries;
pub use fake_acp_agent::FakeAcpAgent;
pub use fake_agent::FakeAgent;
pub use fake_checkpoints::FakeCheckpoints;
pub use fake_engine_port::FakeEnginePort;
pub use fake_engines::FakeEngines;
pub use fake_input::FakeInput;
pub use fake_terminal::FakeTerminal;
pub use script::Script;
pub use step::Step;
pub use timeline::Timeline;
