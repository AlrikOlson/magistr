//! Private native worker isolation; no workspace lock, daemon, or MCP transport.
mod dispatch;
#[cfg(feature = "sandbox")]
mod guest_end;
#[cfg(feature = "lang-pack")]
mod language_lane;
#[cfg(feature = "lang-pack")]
mod language_registration;
#[cfg(feature = "sandbox")]
mod location;
#[cfg(feature = "sandbox")]
mod policy;
mod request;
mod response;
mod search_hit;
mod structural_hit;
#[cfg(feature = "sandbox")]
mod subprocess_spec;
mod symbol;
mod test_range;
#[cfg(feature = "sandbox")]
pub use guest_end::WorkerGuestEnd;
#[cfg(feature = "lang-pack")]
pub use language_lane::WorkerLanguageLane;
#[cfg(feature = "lang-pack")]
pub use language_registration::WorkerLanguageRegistration;
#[cfg(feature = "sandbox")]
pub use location::WorkerLocation;
#[cfg(feature = "sandbox")]
pub use policy::WorkerPolicy;
pub use request::WorkerRequest;
pub use response::WorkerResponse;
pub use search_hit::WorkerSearchHit;
pub use structural_hit::WorkerStructuralHit;
#[cfg(feature = "sandbox")]
pub use subprocess_spec::WorkerSubprocessSpec;
pub use symbol::WorkerSymbol;
pub use test_range::WorkerTestRange;
mod error;
mod frame;
mod options;
mod process;
mod serve;
mod status;
mod supervisor;

pub use error::WorkerError;
pub use options::WorkerOptions;
pub use serve::serve_worker;
pub use status::WorkerStatus;
pub use supervisor::WorkerSupervisor;
