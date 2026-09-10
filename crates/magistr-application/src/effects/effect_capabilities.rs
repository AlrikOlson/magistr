/// Available host operations; absence never implies ambient authority.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EffectCapabilities {
    pub read_text_file: bool,
    pub write_text_file: bool,
    pub terminal: bool,
}
