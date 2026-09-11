//! Native workspace diagnostics (chunk arch-14c9): status, journal
//! inspection, the JSONL flight recorder and the doctor report and bundle.
//!
//! Everything here observes. No diagnostic opens a writer, takes the native
//! cache lock, creates state under the workspace, or signals a live owner;
//! a missing or unreadable store is a finding, never a repair. Diagnostics
//! describe caches, breakers and logs — never durable operation or evidence
//! truth, which stay with the journal intents and the receipts.
mod bundle_redaction;
mod doctor_bundle;
mod doctor_data_class;
mod doctor_finding;
mod doctor_options;
mod doctor_probes;
mod doctor_report;
mod doctor_run;
mod doctor_section;
mod doctor_section_id;
mod doctor_severity;
mod engine_span;
mod envelope_versions;
mod index_health;
mod journal_inspection;
mod journal_row;
mod journal_view_options;
mod span_outcome;
mod span_status;
mod tar_writer;
mod tool_span;
mod trace_recorder;
mod utc_timestamp;
mod worker_health;
mod workspace_status;

pub use bundle_redaction::redact_bundle_text;
pub use doctor_bundle::DoctorBundle;
pub use doctor_data_class::DoctorDataClass;
pub use doctor_finding::DoctorFinding;
pub use doctor_options::DoctorOptions;
pub use doctor_report::DoctorReport;
pub use doctor_run::DoctorRun;
pub use doctor_section::DoctorSection;
pub use doctor_section_id::DoctorSectionId;
pub use doctor_severity::DoctorSeverity;
pub use engine_span::EngineSpan;
pub use envelope_versions::EnvelopeVersions;
pub use index_health::IndexHealth;
pub use journal_inspection::JournalInspection;
pub use journal_row::JournalRow;
pub use journal_view_options::JournalViewOptions;
pub use span_outcome::SpanOutcome;
pub use span_status::SpanStatus;
pub use tool_span::ToolSpan;
pub use trace_recorder::TraceRecorder;
pub(crate) use utc_timestamp::now_unix_ms;
pub use utc_timestamp::utc_timestamp;
pub use worker_health::WorkerHealth;
pub use workspace_status::WorkspaceStatus;
