//! Embedded, proportional typography for graphics terminals.

use crate::fonts::Faces;
use crate::layout::{Measure, TextMetrics, TextRole};
use crate::pixel::CellSize;
use fontdue::{Font, FontSettings, Metrics};
use std::collections::HashMap;

/// One font cache and ruler, shared by pixel layout and painting.
#[derive(Debug)]
pub struct Typography {
    fonts: &'static Vec<Font>,
    glyphs: HashMap<(char, usize, u32), (Metrics, Vec<u8>)>,
    cell: CellSize,
}

impl Default for Typography {
    fn default() -> Self {
        static FONTS: std::sync::OnceLock<Vec<Font>> = std::sync::OnceLock::new();
        let fonts = FONTS.get_or_init(|| {
            [
                Faces::SANS,
                Faces::SANS_BOLD,
                Faces::REGULAR,
                Faces::BOLD,
                Faces::FALLBACK,
            ]
            .into_iter()
            .map(|bytes| {
                Font::from_bytes(bytes, FontSettings::default()).expect("valid embedded face")
            })
            .collect()
        });
        Self {
            fonts,
            glyphs: HashMap::new(),
            cell: CellSize::DEFAULT,
        }
    }
}

impl Typography {
    pub const fn cell(&self) -> CellSize {
        self.cell
    }

    pub fn set_cell(&mut self, cell: CellSize) {
        if self.cell != cell {
            self.glyphs.clear();
            self.cell = cell;
        }
    }

    pub fn size(&self, role: TextRole) -> f32 {
        f32::from(self.cell.height) * if role.is_mono() { 0.75 } else { 0.83 }
    }

    fn face(&self, ch: char, role: TextRole, bold: bool) -> usize {
        let index = usize::from(role.is_mono()) * 2 + usize::from(bold);
        [index, 4, 5]
            .into_iter()
            .find(|&candidate| self.font(candidate).lookup_glyph_index(ch) != 0)
            .unwrap_or(4)
    }

    fn font(&self, index: usize) -> &'static Font {
        if index < self.fonts.len() {
            return &self.fonts[index];
        }
        // Most sessions use only the small UI faces. Parse the broad fallback
        // once, and only when the conversation contains a missing character.
        static CJK: std::sync::OnceLock<Font> = std::sync::OnceLock::new();
        CJK.get_or_init(|| {
            Font::from_bytes(Faces::CJK, FontSettings::default()).expect("valid embedded CJK face")
        })
    }

    pub fn advance(&self, text: &str, role: TextRole) -> f32 {
        if role.is_counted() {
            return f32::from(crate::render::width(text)) * f32::from(self.cell.width);
        }
        text.chars()
            .map(|ch| {
                self.font(self.face(ch, role, false))
                    .metrics(ch, self.size(role))
                    .advance_width
            })
            .sum()
    }

    pub fn baseline(&self, role: TextRole) -> f32 {
        let font = &self.fonts[usize::from(role.is_mono()) * 2];
        let size = self.size(role);
        font.horizontal_line_metrics(size).map_or(size, |m| {
            (f32::from(self.cell.height) - (m.ascent - m.descent)) / 2.0 + m.ascent
        })
    }

    pub fn glyph(&mut self, ch: char, role: TextRole, bold: bool) -> &(Metrics, Vec<u8>) {
        let index = self.face(ch, role, bold);
        let size = self.size(role);
        let font = self.font(index);
        self.glyphs
            .entry((ch, index, size.to_bits()))
            .or_insert_with(|| font.rasterize(ch, size))
    }
}

impl Measure for Typography {
    fn intrinsic(&mut self, text: &TextMetrics) -> (f32, f32) {
        let width = |s: &str| {
            let regular = self.advance(s, text.role());
            let bold: f32 = s
                .chars()
                .map(|ch| {
                    self.font(self.face(ch, text.role(), true))
                        .metrics(ch, self.size(text.role()))
                        .advance_width
                })
                .sum();
            if s.is_empty() {
                0.0
            } else {
                regular.max(bold) + 1.0
            }
        };
        (
            text.text().lines().map(width).fold(0.0, f32::max),
            text.text()
                .split_whitespace()
                .map(width)
                .fold(0.0, f32::max),
        )
    }

    fn wrap<'a>(&mut self, text: &'a TextMetrics, width: f32) -> Vec<&'a str> {
        text.wrap_by(width, &mut |s| self.advance(s, text.role()))
    }

    fn line_height(&mut self) -> f32 {
        f32::from(self.cell.height)
    }
}
