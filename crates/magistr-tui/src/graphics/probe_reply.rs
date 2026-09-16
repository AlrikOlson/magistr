//! What the terminal answered to the startup probe.

/// The probe's findings so far. `complete` flips once the DA1 reply has
/// arrived — every terminal answers DA1, and it was sent last, so nothing
/// more is coming.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProbeReply {
    /// The kitty graphics query was answered `OK`.
    pub kitty: bool,
    /// DA1 listed parameter 4: sixel graphics.
    pub sixel: bool,
    /// The three cursor-position reports of kitty's text sizing
    /// handshake each advanced by the two cells the protocol asked for:
    /// the terminal renders an `OSC 66` run scaled
    /// ([`Headline`](magistr_cockpit::render::Headline)).
    pub text_sizing: bool,
    /// DECRQM reports that SGR pixel mouse mode (1016) can be enabled.
    /// The query reaches the input owner itself, including tmux.
    pub pixel_mouse: bool,
    /// The DA1 reply has been seen; the driver can leave the probe.
    pub complete: bool,
}
