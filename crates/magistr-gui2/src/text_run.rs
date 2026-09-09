use crate::component::Component;

/// One text run as the copy-law sweep reads it off an element tree:
/// what it says and which component claimed it (`None` is naked text,
/// which the sweep refuses).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRun {
    /// The run's content, as set — before the voice's own uppercasing.
    pub text: String,
    /// The component the run belongs to.
    pub owner: Option<Component>,
}
