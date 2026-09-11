/// Marked response text and the detectors applied in stable order, including repeats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedText {
    pub text: String,
    pub kinds: Vec<&'static str>,
}
