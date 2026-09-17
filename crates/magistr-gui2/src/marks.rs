use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use magistr_cockpit::render::resolve;
use magistr_cockpit::render::{Color, Style};
use magistr_cockpit::theme::{Pen, Theme};
use magistr_theme::Palette;
use qs_gpu::atlas::ImageKey;
use qs_gpu::color::Srgba;
use qs_gpu::frame::{DrawList, Instance};

use crate::icon::Icon;
use crate::icon_role::IconRole;
use crate::mark::Mark;
use crate::mark_raster::mark_raster;
use crate::sigil::Sigil;
use crate::text::TextEngine;

/// The Almagest set on the window: marks rasterized from their geometry, admitted to
/// the atlas once per distinct pixel grid, and drawn as textured quads.
///
/// The route is [`Chrome`](crate::chrome::Chrome)'s — the colour page through
/// [`TextEngine::image`], one `Instance::image` per quad — for the same reason: the
/// atlas already rations uploads, evicts by CLOCK and never drops an entry used this
/// frame, and a mark is a picture like any other. qs-gpu keeps its *own* icon set on
/// the coverage page behind `AtlasKey::Icon`, which would cost a byte per texel rather
/// than four and would tint at draw time — but that key names the file explorer's
/// `IconKind` enum, so joining it is a framework change this set does not need at 24
/// pixels square.
///
/// # The role picks the ink
///
/// [`IconRole`] is not decoration on the call: it is what a mark is drawn *in*, derived
/// from the control language (`docs/architecture/desktop.md#controls`) rather than
/// chosen per call site. A `Kind` mark is a classifier beside the label that matters,
/// so it takes the muted pen; `Nav` and `Utility` are things you act on and take full
/// ink; `Empty` is the faintest ink the theme has, because the design puts an empty
/// slot at the quietest end of the language. Two roles sharing an ink is not a gap — a
/// nav chip and a utility button differ in whether a word sits beside the mark
/// ([`IconRole::takes_label`]), not in colour.
#[derive(Clone, Copy, Debug)]
pub struct Marks {
    /// One ink per [`IconRole`], in [`IconRole::ALL`]'s order.
    inks: [(u8, u8, u8); 4],
    /// What a sheet of marks is painted on.
    ground: (u8, u8, u8),
}

impl Marks {
    /// The two sizes the design declares for the set: the volume's
    /// `.ic` and `.ic.lg`.
    pub const SMALL: u32 = 13;
    pub const LARGE: u32 = 24;

    /// Every mark the specimen sheet draws — the number that has to
    /// become resident in the atlas before the sheet is whole.
    pub const SPECIMEN: usize =
        Icon::ALL.len() * 2 + Sigil::VOLUME_PROJECTS.len() * 2 + IconRole::ALL.len();

    /// The set for `theme`'s palette.
    #[must_use]
    pub fn for_theme(theme: &Theme) -> Self {
        let pen = Pen::for_theme(theme);
        let palette = theme.palette();
        let fg = |style: &Style| rgb(style.fg, &palette, true);
        Self {
            inks: [
                fg(&pen.note),         // Kind
                fg(&pen.stage),        // Nav
                fg(&pen.tab_inactive), // Empty
                fg(&pen.stage),        // Utility
            ],
            ground: rgb(pen.base.bg, &palette, false),
        }
    }

    /// What a mark of `role` is drawn in.
    #[must_use]
    pub const fn ink(&self, role: IconRole) -> (u8, u8, u8) {
        match role {
            IconRole::Kind => self.inks[0],
            IconRole::Nav => self.inks[1],
            IconRole::Empty => self.inks[2],
            IconRole::Utility => self.inks[3],
        }
    }

    /// The canvas a sheet of marks is drawn on.
    #[must_use]
    pub const fn ground(&self) -> Srgba {
        Srgba::new(
            self.ground.0 as f32 / 255.0,
            self.ground.1 as f32 / 255.0,
            self.ground.2 as f32 / 255.0,
            1.0,
        )
    }

    /// Draw `mark` as a `px`-square mark of `role`, its top-left corner
    /// at `at`.
    ///
    /// Answers whether a quad went out. `false` means the atlas
    /// declined the picture this frame — the framework admits two a
    /// frame — so the mark arrives on a later frame rather than being
    /// lost; a caller that must have every mark at once (a golden)
    /// builds until [`TextEngine::converged`].
    ///
    /// The raster is made at exactly `px` and drawn at exactly `px`, so
    /// a mark is never resampled — which is the whole reason the set is
    /// held as geometry and not as one bitmap per glyph.
    pub fn draw(
        &self,
        mark: Mark<'_>,
        role: IconRole,
        at: (f32, f32),
        px: u32,
        text: &mut TextEngine,
        list: &mut DrawList,
    ) -> bool {
        let ink = self.ink(role);
        let key = key((mark, px, ink));
        let Some(entry) = text.image(key, |_| mark_raster(&mark.shapes(), px, ink)) else {
            return false;
        };
        let size = px as f32;
        list.instances.push(Instance::image(
            at.0,
            at.1,
            size,
            size,
            entry.uv,
            Srgba::new(1.0, 1.0, 1.0, 1.0),
        ));
        true
    }

    /// The specimen sheet's size in pixels.
    #[must_use]
    pub const fn sheet() -> (u32, u32) {
        (Sheet::WIDTH, Sheet::HEIGHT)
    }

    /// Emit the whole specimen into `list`: the 27 glyphs at
    /// [`Marks::LARGE`] and again at [`Marks::SMALL`], the volume's four
    /// project sigils at both sizes, and one row of the same glyph in
    /// each of the four roles' inks.
    ///
    /// Answers how many quads went out — which is
    /// [`Marks::SPECIMEN`] only once every picture is resident, and is
    /// how [`Frame::specimen`](crate::Frame::specimen) knows the sheet
    /// is whole.
    ///
    /// The sheet is drawn through exactly the route a real placement
    /// takes — the same rasterizer, the same atlas page, the same quad
    /// — so a still of it is a statement about the painter rather than
    /// about a second drawing beside it.
    pub fn specimen(&self, text: &mut TextEngine, list: &mut DrawList) -> usize {
        let mut drawn = 0;
        for (index, icon) in Icon::ALL.into_iter().enumerate() {
            for (band, px) in [
                (Sheet::GLYPHS_LARGE, Self::LARGE),
                (Sheet::GLYPHS_SMALL, Self::SMALL),
            ] {
                let mark = Mark::Icon(icon);
                let at = Sheet::grid(band, index, px);
                drawn += usize::from(self.draw(mark, IconRole::Kind, at, px, text, list));
            }
        }
        for (index, project) in Sigil::VOLUME_PROJECTS.into_iter().enumerate() {
            for (band, px) in [
                (Sheet::SIGILS_LARGE, Self::LARGE),
                (Sheet::SIGILS_SMALL, Self::SMALL),
            ] {
                let mark = Mark::Sigil(project);
                let at = Sheet::grid(band, index, px);
                drawn += usize::from(self.draw(mark, IconRole::Nav, at, px, text, list));
            }
        }
        for (index, role) in IconRole::ALL.into_iter().enumerate() {
            let at = Sheet::grid(Sheet::ROLES, index, Self::LARGE);
            drawn += usize::from(self.draw(
                Mark::Icon(Icon::Session),
                role,
                at,
                Self::LARGE,
                text,
                list,
            ));
        }
        drawn
    }
}

/// Where the specimen puts things.
///
/// A plain grid rather than a laid-out surface, because the sheet's job
/// is to show every mark at both sizes in one picture a human can scan
/// — the cockpit's own layout engine has nothing to add to a contact
/// sheet, and running the marks through it would make the golden a
/// picture of the layout too.
struct Sheet;

impl Sheet {
    const MARGIN: f32 = 16.0;
    const COLUMNS: usize = 9;
    /// A cell is the mark plus its gutter; the mark is centred in it.
    const LARGE_PITCH: f32 = 40.0;
    const SMALL_PITCH: f32 = 24.0;

    const GLYPHS_LARGE: Band = Band {
        top: Self::MARGIN,
        pitch: Self::LARGE_PITCH,
    };
    const GLYPHS_SMALL: Band = Band {
        top: Self::MARGIN + 3.0 * Self::LARGE_PITCH + Self::MARGIN,
        pitch: Self::SMALL_PITCH,
    };
    const SIGILS_LARGE: Band = Band {
        top: Self::GLYPHS_SMALL.top + 3.0 * Self::SMALL_PITCH + Self::MARGIN,
        pitch: Self::LARGE_PITCH,
    };
    const SIGILS_SMALL: Band = Band {
        top: Self::SIGILS_LARGE.top + Self::LARGE_PITCH,
        pitch: Self::SMALL_PITCH,
    };
    const ROLES: Band = Band {
        top: Self::SIGILS_SMALL.top + Self::SMALL_PITCH + Self::MARGIN,
        pitch: Self::LARGE_PITCH,
    };

    const WIDTH: u32 = Self::MARGIN as u32 * 2 + Self::COLUMNS as u32 * Self::LARGE_PITCH as u32;
    const HEIGHT: u32 = Self::ROLES.top as u32 + Self::LARGE_PITCH as u32 + Self::MARGIN as u32;

    /// Where the `index`th `px`-square mark of `band` goes, centred in
    /// its cell and rounded to whole pixels so the raster is drawn 1:1.
    fn grid(band: Band, index: usize, px: u32) -> (f32, f32) {
        let column = index % Self::COLUMNS;
        let row = index / Self::COLUMNS;
        let inset = (band.pitch - px as f32) / 2.0;
        (
            (Self::MARGIN + column as f32 * band.pitch + inset).round(),
            (band.top + row as f32 * band.pitch + inset).round(),
        )
    }
}

/// One row band of the specimen: where it starts and how far apart its
/// cells are.
#[derive(Clone, Copy)]
struct Band {
    top: f32,
    pitch: f32,
}

/// A terminal colour as the palette's own bytes — the resolution both
/// renderers share (the resolve table is the same call in the
/// GPU's float form).
fn rgb(color: Color, palette: &Palette, foreground: bool) -> (u8, u8, u8) {
    let [r, g, b] = resolve(color, palette, foreground);
    (r, g, b)
}

/// A stable [`ImageKey`] for whatever a mark was rasterized from.
fn key(from: impl Hash) -> ImageKey {
    let mut hasher = DefaultHasher::new();
    from.hash(&mut hasher);
    ImageKey(hasher.finish())
}

#[cfg(test)]
#[path = "marks.tests.rs"]
mod tests;
