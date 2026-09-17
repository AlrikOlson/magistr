//! Native confirmed mutation pipeline; no tool handlers or workspace owner lock.
mod change;
mod error;
mod expected_state;
mod file_disposition;
mod file_preview;
mod file_result;
mod format_mode;
mod formatter;
mod formatter_options;
mod formatter_process;
mod outcome;
mod prepared;
mod request;
mod service;
mod status;
pub use change::TransactionChange;
pub use error::TransactionError;
pub use expected_state::ExpectedState;
pub use file_disposition::FileDisposition;
pub use file_preview::FilePreview;
pub use file_result::FileResult;
pub use format_mode::FormatMode;
pub use formatter::NativeFormatter;
pub use formatter_options::FormatterOptions;
pub use outcome::TransactionOutcome;
pub use prepared::PreparedTransaction;
pub use request::TransactionRequest;
pub use service::WorkspaceTransactions;
pub use status::TransactionStatus;

mod apply;
mod baseline_source;
mod binding;
mod compensate;
mod created_parents;
#[cfg(test)]
mod faults;
mod native_files;
mod prepare;
mod snapshot;
mod trash;
#[cfg(unix)]
mod unix_swap;
mod verification;
pub use baseline_source::BaselineSource;

mod verification_status;
pub use verification_status::VerificationStatus;
