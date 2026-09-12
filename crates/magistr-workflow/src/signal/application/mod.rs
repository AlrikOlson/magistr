//! Typed signal operations shared by every workflow transport.

mod error;
mod inbox;
mod promotion;
mod summary;
mod use_cases;

pub use error::SignalError;
pub use inbox::SignalInbox;
pub use promotion::SignalPromotion;
pub use summary::SignalSummary;
pub use use_cases::SignalUseCases;
