/// Root-span attributes known only once the envelope exists. These typed
/// fields are the whole vocabulary: tool arguments, query text, file
/// contents and environment values are unrepresentable in a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpanOutcome {
    pub envelope_v: Option<u64>,
    pub notes_count: Option<usize>,
    pub truncated: Option<bool>,
}
