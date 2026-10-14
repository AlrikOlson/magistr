//! Session admission and negotiated provider facts travel together to all readers.
/// Facts published by the session owner, never inferred from text or silence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionActivity {
    pub active: bool,
    pub queued_turns: usize,
    pub configuration_in_progress: bool,
    pub cancellation_requested: bool,
    /// Unknown until the provider's initialization has been observed.
    pub can_resume: Option<bool>,
    pub can_send_images: Option<bool>,
}
