//! The wire key of the stamp every broadcast frame carries beside its
//! family: the publisher's per-frame counter. Read by the subscriber that
//! decodes the line; written by the broadcaster.

/// The wire key of the publisher's per-frame counter.
pub const SEQ_KEY: &str = "seq";
