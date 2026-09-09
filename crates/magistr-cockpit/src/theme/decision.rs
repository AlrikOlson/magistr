//! Which palette the cockpit draws with, and where that decision came from.

use magistr_theme::{CvdMode, Palette};

use super::icon_mode::IconMode;
use super::icons::Icons;
use super::mode::ThemeMode;
use super::motion_mode::MotionMode;
use crate::render::Color;

/// Everything about the screen the terminal and the user decide between
/// them: a [`ThemeMode`] and whether the user forced it — a forced theme
/// ignores what the terminal answers to OSC 11 — the [`IconMode`] the
/// glyphs come from, the [`CvdMode`] the signal colours are remapped
/// for, if any, and the [`MotionMode`] deciding whether anything on the
/// screen moves.
///
/// The decision is pure ([`Theme::detect`]); the one read of the process
/// environment is [`Theme::from_os`], which the entry points call before
/// the runtime starts. The runtime's OSC 11 reply then reaches the theme
/// through [`Theme::observe`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Theme {
    mode: ThemeMode,
    forced: bool,
    icons: IconMode,
    cvd: Option<CvdMode>,
    motion: MotionMode,
}

impl Theme {
    /// The environment variable that overrides detection: `light` or
    /// `dark` (see [`ThemeMode::parse`]).
    pub const ENV: &'static str = "MAGISTR_THEME";

    /// Decide the theme: a parsable `env` value wins and is forced;
    /// otherwise the terminal's `background` (its OSC 11 reply) decides by
    /// luminance; with neither, dark.
    /// The environment variable that remaps the signal colours for a
    /// colour vision deficiency: `deuteranopia`, `protanopia` or
    /// `tritanopia` (`--cvd` sets it).
    pub const CVD_ENV: &'static str = "MAGISTR_CVD";

    #[must_use]
    pub fn detect(background: Option<Color>, env: Option<&str>) -> Self {
        match env.and_then(ThemeMode::parse) {
            Some(mode) => Self::forced(mode),
            None => Self {
                mode: background.map_or(ThemeMode::Dark, ThemeMode::for_background),
                forced: false,
                ..Self::default()
            },
        }
    }

    /// The theme this process's environment asks for: `MAGISTR_THEME`
    /// for the palette (or the dark default awaiting the terminal's
    /// answer), `MAGISTR_ICONS`/`TERM_PROGRAM` for the glyphs,
    /// `MAGISTR_CVD` for the signal remapping, and `MAGISTR_MOTION` (or
    /// the `REDUCE_MOTION`/`NO_ANIMATIONS` conventions, or
    /// [`Env::SCREEN_READER`](crate::capability::Mode::SCREEN_READER_ENV)) for
    /// motion.
    #[must_use]
    pub fn from_os() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Resolve every theme input from one supplied environment.
    fn from_lookup(var: impl Fn(&str) -> Option<String>) -> Self {
        // A screen-reader session never animates: a spinner stepping
        // sixteen times a second is sixteen more things for a reader to
        // say. It joins the two `--reduce-motion` conventions here
        // rather than at each motion site, so the guarantee is
        // structural — no tick, no steps.
        let reduce: Vec<String> = MotionMode::REDUCE_ENVS
            .iter()
            .chain(std::iter::once(&crate::capability::Mode::SCREEN_READER_ENV))
            .filter_map(|n| var(n))
            .collect();
        Self::detect(None, var(Self::ENV).as_deref())
            .with_icons(IconMode::detect(
                var(IconMode::ENV).as_deref(),
                var(IconMode::TERM_ENV).as_deref(),
            ))
            .with_cvd(var(Self::CVD_ENV).as_deref().and_then(CvdMode::from_name))
            .with_motion(MotionMode::detect(
                var(MotionMode::ENV).as_deref(),
                reduce.iter().map(String::as_str),
            ))
    }

    /// `mode`, and no reply will change it.
    #[must_use]
    pub fn forced(mode: ThemeMode) -> Self {
        Self {
            mode,
            forced: true,
            ..Self::default()
        }
    }

    /// The same theme drawing `icons`.
    #[must_use]
    pub const fn with_icons(mut self, icons: IconMode) -> Self {
        self.icons = icons;
        self
    }

    /// The same theme with its signal colours remapped for `cvd`.
    #[must_use]
    pub const fn with_cvd(mut self, cvd: Option<CvdMode>) -> Self {
        self.cvd = cvd;
        self
    }

    /// The same theme moving (or not) as `motion` says.
    #[must_use]
    pub const fn with_motion(mut self, motion: MotionMode) -> Self {
        self.motion = motion;
        self
    }

    /// Fold in the terminal's background colour: a detected theme follows
    /// it, a forced one does not. The glyph, CVD and motion choices are
    /// the user's, so the terminal never moves them.
    #[must_use]
    pub fn observe(self, background: Color) -> Self {
        if self.forced {
            self
        } else {
            Self::detect(Some(background), None)
                .with_icons(self.icons)
                .with_cvd(self.cvd)
                .with_motion(self.motion)
        }
    }

    /// Whether decorative motion is on.
    #[must_use]
    pub const fn motion(&self) -> MotionMode {
        self.motion
    }

    /// The glyph vocabulary in force.
    #[must_use]
    pub const fn icon_mode(&self) -> IconMode {
        self.icons
    }

    /// The glyphs the view draws with.
    #[must_use]
    pub const fn icons(&self) -> &'static Icons {
        self.icons.icons()
    }

    /// The colour vision deficiency the signal colours are remapped for.
    #[must_use]
    pub const fn cvd(&self) -> Option<CvdMode> {
        self.cvd
    }

    /// Dark or light.
    #[must_use]
    pub const fn mode(&self) -> ThemeMode {
        self.mode
    }

    /// Whether `MAGISTR_THEME` decided the mode.
    #[must_use]
    pub const fn is_forced(&self) -> bool {
        self.forced
    }

    /// The palette for the mode.
    #[must_use]
    pub fn palette(&self) -> Palette {
        self.mode.palette()
    }
}

#[cfg(test)]
#[path = "decision.tests.rs"]
mod tests;
