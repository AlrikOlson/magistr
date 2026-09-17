use magistr_cockpit::layout::TextRole;
use magistr_cockpit::render::{Attrs, Style};
use qs_text::PxSize;
use qs_text::fontdb::FontId;

use crate::fonts::EmbeddedFonts;
use crate::metrics::CellMetrics;
use crate::text::TextEngine;
use crate::type_set::TypeSet;
use crate::voice::Voice;
use crate::voice_run::VoiceRun;

/// Map each [`TextRole`] to a [`Voice`] from the generated design token table.
///
/// Sizes scale by `cell_px / DEFAULT_PX`, so proportional and monospace text grow
/// together on HiDPI displays. At [`CellMetrics::DEFAULT_PX`], sizes match the source
/// tokens exactly.
///
/// All roles share the cell baseline, keeping mixed-size labels and chips aligned
/// within a row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeScale {
    mono: FontId,
    mono_px: f32,
    factor: f32,
}

impl TypeScale {
    /// The ledger name's size — [`TextRole::Label`].
    pub const LABEL_PX: f32 = Voice::of(TextRole::Label).px;
    /// The turn's body size — [`TextRole::Prose`].
    pub const PROSE_PX: f32 = Voice::of(TextRole::Prose).px;
    /// The moment title's size — [`TextRole::Heading`].
    pub const HEADING_PX: f32 = Voice::of(TextRole::Heading).px;
    /// The weight a heading asks for, and the one break the family has.
    pub const HEADING_WEIGHT: qs_text::fontdb::FontWeight =
        qs_text::fontdb::FontWeight(Voice::of(TextRole::Heading).weight);
    /// The weight at or above which the proportional family takes its
    /// second cut. The volume's heaviest voice is 600, so a rule keyed
    /// on `FontWeight::BOLD` (700) would send every heading to the
    /// regular face; the ledger name's 550 stays regular for the same
    /// reason read the other way.
    const SEMIBOLD: u16 = 600;

    /// The scale on `metrics`' grid.
    #[must_use]
    pub fn new(metrics: &CellMetrics) -> Self {
        let mono_px = metrics.size().to_f32();
        Self {
            mono: metrics.font(),
            mono_px,
            factor: mono_px / CellMetrics::DEFAULT_PX,
        }
    }

    /// How far this grid is from the default one — what every size and
    /// every radius on the native path is multiplied by, so the whole
    /// drawing scales together rather than the type scaling and the
    /// geometry staying put.
    #[must_use]
    pub const fn factor(&self) -> f32 {
        self.factor
    }

    /// The face and size a run in `role` wearing `style`'s attributes is
    /// set in.
    ///
    /// `BOLD` on a proportional run resolves to the semibold face — the
    /// family's one weight break — rather than to a heavier one it does
    /// not have; `ITALIC` takes the family's italic, so a cockpit pen
    /// that leans on a terminal leans here too. A style asking for both
    /// gets the italic: neither family ships a bold italic, and
    /// synthesizing one by shearing the semibold is the very trick this
    /// path exists to stop doing.
    #[must_use]
    pub fn resolve(&self, role: TextRole, style: &Style) -> TypeSet {
        let voice = Voice::of(role);
        let attrs = style.attrs;
        let (italic, bold) = (attrs.contains(Attrs::ITALIC), attrs.contains(Attrs::BOLD));
        let heavy = bold || voice.weight >= Self::SEMIBOLD;
        if voice.is_mono() {
            return TypeSet {
                font: match (italic, bold) {
                    (true, _) => EmbeddedFonts::MONO_ITALIC,
                    (false, true) => EmbeddedFonts::BOLD,
                    (false, false) => self.mono,
                },
                // The grid's voice is sized by the cell and every other
                // mono voice by its own token: a badge and a diff line
                // are drawn at 9 and 10.5 px whatever the cell is.
                size: PxSize::new(self.size(role, &voice)),
                // The data voice is engraved: slashed zero, tabular
                // figures — the voice's own answer, not the role's.
                features: voice.features(),
            };
        }
        let font = match (italic, heavy) {
            (true, _) => EmbeddedFonts::UI_ITALIC,
            (false, true) => EmbeddedFonts::UI_SEMIBOLD,
            (false, false) => EmbeddedFonts::UI_REGULAR,
        };
        TypeSet {
            font,
            size: PxSize::new(self.size(role, &voice)),
            features: voice.features(),
        }
    }

    /// The size `role` is set at on this grid, in physical pixels.
    fn size(&self, role: TextRole, voice: &Voice) -> f32 {
        if role.is_counted() {
            self.mono_px
        } else {
            voice.px * self.factor
        }
    }

    /// The letter-spacing `role` is tracked by, in physical pixels —
    /// added after every grapheme cluster, the way CSS adds it.
    #[must_use]
    pub fn tracking(&self, role: TextRole) -> f32 {
        let voice = Voice::of(role);
        voice.tracking_em * self.size(role, &voice)
    }

    /// The voice `role` is set in, on this grid.
    #[must_use]
    pub fn voice(&self, role: TextRole) -> Voice {
        Voice::of(role)
    }

    /// `line` set in `role`'s voice: transformed as the volume
    /// transforms it, shaped through `engine`'s run cache, and carrying
    /// the tracking to lay over the shaper's advances.
    ///
    /// The one seam. [`ShapedMeasure`](crate::ShapedMeasure) asks it
    /// how wide a run is and [`Painter`](crate::Painter) asks it what
    /// to draw, so a measured width and a painted width are the same
    /// number by construction rather than by two wrappers agreeing.
    pub fn shape<'a>(
        &self,
        engine: &mut TextEngine,
        role: TextRole,
        style: &Style,
        line: &'a str,
    ) -> VoiceRun<'a> {
        let text = Voice::of(role).transform(line);
        let set = self.resolve(role, style);
        let run = engine.run(&text, set.font, set.size, set.features);
        VoiceRun {
            text,
            set,
            run,
            tracking: self.tracking(role),
        }
    }
}

#[cfg(test)]
#[path = "type_scale.tests.rs"]
mod tests;
