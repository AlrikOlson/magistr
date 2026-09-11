//! Native confirmation tokens deliberately invalidate retained `wb1` tokens.
//! The composing owner supplies its private workspace key and session identity.
mod confirmation_error;
mod confirmation_seal;
pub use confirmation_error::ConfirmationError;
pub use confirmation_seal::ConfirmationSeal;
