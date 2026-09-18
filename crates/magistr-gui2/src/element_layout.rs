use taffy::compute_leaf_layout;
use taffy::geometry::Size;
use taffy::prelude::{
    AvailableSpace, Display, FlexDirection, NodeId, Style, TaffyTree, auto, length,
};
use taffy::style::{AlignItems, FlexWrap, JustifyContent, LengthPercentageAuto, Position};

use crate::element::{Align, Axis, Element, Justify, Kind};
use crate::error::Gui2Error;
use crate::fonts::EmbeddedFonts;
use crate::text::TextEngine;
use crate::type_set::TypeSet;
use crate::voice::Voice;
use crate::voice_run::VoiceRun;

/// Where layout put every element — f32 pixels end to end.
///
/// # The whole point of this module
///
/// taffy's flexbox is driven here in raw pixels: a pad is the token's
/// own f32, a text leaf is measured through the real shaper
/// ([`TextEngine`]) at its [`Voice`]'s size, a row of text is as tall
/// as the voice's own line box, and a paragraph breaks where its
/// shaped words stop fitting — the volume's max-width and leading
/// semantics, with no column arithmetic anywhere.
///
/// # One wrapper, both passes
///
/// The measure closure and the final pass break text with the same
/// [`wrap_ranges`], and emission draws the ranges this struct stored —
/// so the rows the layout paid height for are exactly the rows that
/// get ink. Two wrappers that merely agreed today would drift.
pub struct ElementLayout {
    /// `[x, y, w, h]` per element, tree order, window-absolute pixels.
    rects: Vec<[f32; 4]>,
    /// Per element: the byte ranges of `content` on each wrapped row.
    /// Empty for surfaces.
    rows: Vec<Vec<(usize, usize)>>,
}

impl ElementLayout {
    /// Lay `root` out over a `width` x `height` pixel viewport,
    /// measuring text through `text`'s shaper.
    ///
    /// # Errors
    ///
    /// [`Gui2Error::Elements`] if taffy refuses the tree, which a tree
    /// built from [`Element`]s never does.
    pub fn compute(
        root: &Element,
        width: f32,
        height: f32,
        text: &mut TextEngine,
    ) -> Result<Self, Gui2Error> {
        let mut taffy: TaffyTree<usize> = TaffyTree::new();
        // The volume's own geometry is fractional — a switch thumb at
        // 2.5px, a crumb glyph dropped 1.5px — and the browser the
        // captures were cut from does not round layout. Rounding here
        // would shear every half-pixel inset off its capture.
        taffy.disable_rounding();
        let mut order = Vec::with_capacity(root.count());
        root.each(&mut |_, element| order.push(element));
        let mut next = 0usize;
        let root_id = build(&mut taffy, root, &mut next)?;

        let mut style = flex_style(root);
        style.size = Size {
            width: length(width),
            height: length(height),
        };
        taffy.set_style(root_id, style).map_err(elements)?;

        let available = Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        };
        taffy
            .compute_layout_with_measure(root_id, available, |inputs, _, context, style| {
                compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, avail| {
                        context
                            .and_then(|index| order.get(*index))
                            .map_or(Size::ZERO, |element| measure(element, known, avail, text))
                    },
                )
            })
            .map_err(elements)?;

        let mut this = Self {
            rects: vec![[0.0; 4]; order.len()],
            rows: vec![Vec::new(); order.len()],
        };
        let mut index = 0usize;
        this.place(&taffy, root, root_id, (0.0, 0.0), &mut index, text)?;
        Ok(this)
    }

    /// `[x, y, w, h]` of the element at `index` in tree order.
    #[must_use]
    pub fn rect(&self, index: usize) -> [f32; 4] {
        self.rects.get(index).copied().unwrap_or([0.0; 4])
    }

    /// Carry the elements at `range` (tree order) `dy` pixels down — a
    /// paint-time move for a subtree whose layout is right and whose
    /// place is *between* two layouts: the feed's rows while a scroll
    /// glides between rows (stage-scroll-anchor-and-rate). Every rect
    /// in the range moves, so the paint, the hit boxes and the pictures
    /// agree; the wrapped text ranges are untouched.
    pub fn translate(&mut self, range: std::ops::Range<usize>, dy: f32) {
        for rect in self.rects.iter_mut().take(range.end).skip(range.start) {
            rect[1] += dy;
        }
    }

    /// How many elements were placed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rects.len()
    }

    /// Whether nothing was placed at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// The wrapped rows of the text element at `index` — byte ranges
    /// into its content.
    pub(crate) fn text_rows(&self, index: usize) -> &[(usize, usize)] {
        self.rows.get(index).map_or(&[], Vec::as_slice)
    }

    /// Walk the placed tree, accumulating window-absolute origins and
    /// breaking each text element's content at its placed width.
    fn place(
        &mut self,
        taffy: &TaffyTree<usize>,
        element: &Element,
        id: NodeId,
        origin: (f32, f32),
        index: &mut usize,
        text: &mut TextEngine,
    ) -> Result<(), Gui2Error> {
        let layout = taffy.layout(id).map_err(elements)?;
        let x = origin.0 + layout.location.x;
        let y = origin.1 + layout.location.y;
        let at = *index;
        *index += 1;
        self.rects[at] = [x, y, layout.size.width, layout.size.height];
        if let Kind::Text { voice, content } = &element.kind {
            // Half a pixel of grace: the measure pass asked for the
            // ceiling of the shaped width, and taffy's whole-pixel
            // rounding may hand back a hair less than was asked for —
            // without it a word would wrap that the measure said fits.
            // A nowrap run never breaks at all: squeezed, it clips.
            let limit = (!element.nowrap).then_some(layout.size.width + 0.5);
            self.rows[at] = if element.editor_cursor.is_some() {
                editor_ranges(text, voice, content, limit.unwrap_or(f32::MAX))
            } else {
                wrap_ranges(text, voice, content, limit)
            };
        }
        let children = taffy.children(id).map_err(elements)?;
        for (child, child_id) in element.children.iter().zip(children) {
            self.place(taffy, child, child_id, (x, y), index, text)?;
        }
        Ok(())
    }
}

/// Build the taffy tree in the same depth-first order
/// [`Element::each`] numbers, so a taffy node and its element share an
/// index by construction.
fn build(
    taffy: &mut TaffyTree<usize>,
    element: &Element,
    next: &mut usize,
) -> Result<NodeId, Gui2Error> {
    let index = *next;
    *next += 1;
    if matches!(element.kind, Kind::Text { .. } | Kind::Grid { .. }) {
        return taffy
            .new_leaf_with_context(flex_style(element), index)
            .map_err(elements);
    }
    let children = element
        .children
        .iter()
        .map(|child| build(taffy, child, next))
        .collect::<Result<Vec<_>, _>>()?;
    taffy
        .new_with_children(flex_style(element), &children)
        .map_err(elements)
}

/// The element's layout intent as a taffy style — every number already
/// in pixels, handed through unscaled.
fn flex_style(element: &Element) -> Style {
    let [left, top, right, bottom] = element.pad;
    // Any pinned inset lifts the element out of flex flow — the CSS
    // absolute semantics the volume's thumbs, fills and badges use.
    let inset = |px: Option<f32>| px.map_or(LengthPercentageAuto::auto(), length);
    Style {
        display: Display::Flex,
        position: if element.pin.iter().any(Option::is_some) {
            Position::Absolute
        } else {
            Position::Relative
        },
        inset: taffy::geometry::Rect {
            left: inset(element.pin[0]),
            top: inset(element.pin[1]),
            right: inset(element.pin[2]),
            bottom: inset(element.pin[3]),
        },
        flex_direction: match element.axis {
            Axis::Row => FlexDirection::Row,
            Axis::Column => FlexDirection::Column,
        },
        flex_wrap: if element.wrap {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        },
        padding: taffy::geometry::Rect {
            left: length(left),
            right: length(right),
            top: length(top),
            bottom: length(bottom),
        },
        gap: Size {
            width: length(element.gap),
            height: length(element.gap),
        },
        size: Size {
            width: element.width.map_or_else(auto, length),
            height: element.height.map_or_else(auto, length),
        },
        max_size: Size {
            width: element.max_width.map_or_else(auto, length),
            height: auto(),
        },
        // Left auto everywhere else on purpose: auto is CSS's own
        // automatic minimum (an item never shrinks below its content),
        // and only the feed chain declares the 0.0 that gives it up.
        min_size: Size {
            width: element.min_width.map_or_else(auto, length),
            height: element.min_height.map_or_else(auto, length),
        },
        align_self: element.stretch.then_some(AlignItems::STRETCH),
        flex_grow: element.grow,
        flex_shrink: element.shrink,
        align_items: Some(match element.align {
            Align::Start => AlignItems::START,
            Align::Center => AlignItems::CENTER,
            Align::End => AlignItems::END,
            Align::Stretch => AlignItems::STRETCH,
        }),
        justify_content: Some(match element.justify {
            Justify::Start => JustifyContent::START,
            Justify::Center => JustifyContent::CENTER,
            Justify::End => JustifyContent::END,
            Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        }),
        ..Style::default()
    }
}

/// How big a leaf wants to be, given what the engine knows: a text
/// element by its wrapped rows, a terminal by its grid.
fn measure(
    element: &Element,
    known: Size<Option<f32>>,
    avail: Size<AvailableSpace>,
    text: &mut TextEngine,
) -> Size<f32> {
    if let Kind::Grid { rows, px } = &element.kind {
        // The grid is the face's own: one measured advance per column,
        // the line height per row, both already whole pixels, so a
        // column is a column on every row.
        let Some(metrics) = text.metrics(*px) else {
            return Size::ZERO;
        };
        let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        #[allow(clippy::cast_precision_loss)]
        let (cols, count) = (cols as f32, rows.len() as f32);
        return Size {
            width: known.width.unwrap_or(cols * metrics.width()),
            height: known.height.unwrap_or(count * metrics.height()),
        };
    }
    let Kind::Text { voice, content } = &element.kind else {
        return Size::ZERO;
    };
    let limit = if element.nowrap {
        None
    } else {
        known.width.or(match avail.width {
            AvailableSpace::Definite(limit) => Some(limit),
            AvailableSpace::MaxContent => None,
            AvailableSpace::MinContent => Some(0.0),
        })
    };
    let ranges = wrap_ranges(text, voice, content, limit);
    let mut widest: f32 = 0.0;
    for (start, end) in &ranges {
        widest = widest.max(shape_voice(text, voice, &content[*start..*end]).width());
    }
    let (line, _) = line_box(text, voice);
    #[allow(clippy::cast_precision_loss)]
    let rows = ranges.len() as f32;
    Size {
        // Rounded up, never down: the flex engine rounds a placed box
        // to whole pixels, and a box a hair narrower than the shaped
        // ink would re-break the paragraph it was measured for.
        width: known.width.unwrap_or_else(|| widest.ceil()),
        height: known.height.unwrap_or_else(|| (line * rows).ceil()),
    }
}

/// Break `content` into rows no wider than `limit`, greedy by words,
/// answering byte ranges into `content`.
///
/// A `\n` always breaks. Words are measured shaped — transform,
/// tracking and all — so a tracked cap breaks where its uppercased ink
/// actually reaches. A word wider than the limit takes its own row and
/// overflows, exactly as CSS overflows an unbreakable word.
pub(crate) fn wrap_ranges(
    text: &mut TextEngine,
    voice: &Voice,
    content: &str,
    limit: Option<f32>,
) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    let mut offset = 0;
    for line in content.split('\n') {
        let base = offset;
        offset += line.len() + 1;
        let Some(limit) = limit else {
            rows.push((base, base + line.len()));
            continue;
        };
        let mut start = 0usize;
        let mut end = 0usize;
        for (at, word) in split_words(line) {
            let candidate = &line[start..at + word.len()];
            if end > start && shape_voice(text, voice, candidate).width() > limit {
                rows.push((base + start, base + end));
                start = at;
            }
            end = at + word.len();
        }
        rows.push((base + start, base + end));
    }
    rows
}

/// Grapheme-safe, pixel-measured editor rows, retaining spaces and final newlines.
pub(crate) fn editor_ranges(
    text: &mut TextEngine,
    voice: &Voice,
    content: &str,
    width: f32,
) -> Vec<(usize, usize)> {
    use unicode_segmentation::UnicodeSegmentation;
    let limit = (width - 2.0).max(1.0);
    let mut rows = Vec::new();
    let mut base = 0;
    for line in content.split('\n') {
        if line.is_empty() {
            rows.push((base, base));
        }
        let mut start = 0;
        while start < line.len() {
            let mut end = start;
            let mut space = None;
            for (at, grapheme) in line[start..].grapheme_indices(true) {
                let next = start + at + grapheme.len();
                if end > start && shape_voice(text, voice, &line[start..next]).width() > limit {
                    end = space.unwrap_or(end);
                    break;
                }
                end = next;
                if grapheme.chars().all(char::is_whitespace) {
                    space = Some(end);
                }
            }
            rows.push((base + start, base + end));
            start = end;
        }
        base += line.len() + 1;
    }
    rows
}

/// Each word of `line` with its byte offset — split on single spaces,
/// the separator the volume's copy actually uses.
fn split_words(line: &str) -> impl Iterator<Item = (usize, &str)> {
    line.split(' ').scan(0usize, |offset, word| {
        let at = *offset;
        *offset += word.len() + 1;
        Some((at, word))
    })
}

/// `slice` set in `voice`: transformed, shaped through the engine's run
/// cache, tracking laid over — the one seam layout and emission both
/// measure through.
pub(crate) fn shape_voice<'t>(
    text: &mut TextEngine,
    voice: &Voice,
    slice: &'t str,
) -> VoiceRun<'t> {
    let set = set_of(voice);
    let transformed = voice.transform(slice);
    let run = text.run(&transformed, set.font, set.size, set.features);
    VoiceRun {
        text: transformed,
        set,
        run,
        tracking: voice.tracking_em * voice.px,
    }
}

/// The face, size and features `voice` is set in — the volume's own
/// resolution: the mono family regular under its engraving, the serif
/// display face for the one serif voice, the sans family breaking to
/// its semibold cut at weight 600.
pub(crate) fn set_of(voice: &Voice) -> TypeSet {
    let font = if voice.is_mono() {
        EmbeddedFonts::REGULAR
    } else if voice.is_serif() {
        EmbeddedFonts::DISPLAY
    } else if voice.weight >= 600 {
        EmbeddedFonts::UI_SEMIBOLD
    } else {
        EmbeddedFonts::UI_REGULAR
    };
    TypeSet {
        font,
        size: qs_text::PxSize::new(voice.px),
        features: voice.features(),
    }
}

/// The line box one row of `voice` occupies: `(height, baseline)`, in
/// pixels — the face's own box floored at the voice's leading, the
/// baseline half the leading over the ascent.
pub(crate) fn line_box(text: &mut TextEngine, voice: &Voice) -> (f32, f32) {
    let set = set_of(voice);
    let Some(metrics) = text.shaper().metrics(set.font, set.size) else {
        // A face with no readable metrics falls back to the voice's own
        // arithmetic, which is always usable.
        let height = (voice.px * voice.leading()).round().max(1.0);
        return (height, voice.px.round().clamp(1.0, height));
    };
    let height = metrics
        .line_height
        .max(set.size.to_f32() * voice.leading())
        .round()
        .max(1.0);
    let leading = (height - (metrics.ascent + metrics.descent)).max(0.0);
    let baseline = (metrics.ascent + leading / 2.0).round().clamp(1.0, height);
    (height, baseline)
}

/// taffy's refusal as this crate's error.
fn elements(error: taffy::TaffyError) -> Gui2Error {
    Gui2Error::Elements(error.to_string())
}

#[cfg(test)]
#[path = "element_layout.tests.rs"]
mod tests;
