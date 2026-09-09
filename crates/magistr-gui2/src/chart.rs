use magistr_cockpit::cockpit::Proposal;
use magistr_cockpit::layout::PxRect;
use qs_gpu::color::Srgba;
use qs_gpu::frame::{DrawList, Instance};

use crate::lexicon::Lexicon;
use crate::medium::Medium;

/// The charting: the proposal drawn as a constellation — every star a filled disc in
/// its workstream's hue, every dependency an edge pulled between two — each born at its
/// own alpha, so the stars appear one by one and the edges follow them the way the
/// agent reasons them out.
///
/// The placement is [`Chart::place`]'s, a pure function of the proposal and the field:
/// a star's column is how deep it stands in the dependency graph, its row its order
/// among the stars at that depth. The composition pins the labels from the same
/// function, so a name and its star cannot come apart.
#[derive(Clone, Debug)]
pub struct Chart {
    field: PxRect,
    scale: f32,
    lex: Lexicon,
    medium: Medium,
}

/// A proposed star's radius (`r="5"`), an edge's width
/// (`stroke-width="1.3"`).
const STAR_PX: f32 = 5.0;
const EDGE_PX: f32 = 1.3;
/// The field's margins the constellation keeps clear, as shares.
const MARGIN_X: f32 = 0.16;
const MARGIN_Y: f32 = 0.22;
/// Most stamps one edge is walked with.
const STAMPS_MAX: usize = 256;
/// Day's stars sit a little back from full ink, as the sky's do.
const DAY_ALPHA: f32 = 0.92;

impl Chart {
    /// A chart over `field` (physical pixels) whose placement is in
    /// logical pixels multiplied by `scale`, in `medium`'s lexicon.
    #[must_use]
    pub const fn new(field: PxRect, scale: f32, lex: Lexicon, medium: Medium) -> Self {
        Self {
            field,
            scale,
            lex,
            medium,
        }
    }

    /// Where each of `proposal`'s stars sits on a field `size` logical
    /// pixels big, in list order: the dependency depth across, the
    /// order within a depth down.
    #[must_use]
    pub fn place(proposal: &Proposal, size: (f32, f32)) -> Vec<(f32, f32)> {
        let chunks = &proposal.chunks;
        // The depth as the longest chain of dependencies behind the
        // star, relaxed as many times as there are stars so a deep
        // star sits past every star it stands on whatever the list
        // order; a cycle, which the drafter never writes, simply stops
        // deepening.
        let mut depths = vec![0usize; chunks.len()];
        for _ in 0..chunks.len() {
            for (index, chunk) in chunks.iter().enumerate() {
                let deepest = chunk
                    .deps
                    .iter()
                    .filter_map(|dep| chunks.iter().position(|c| &c.id == dep))
                    .map(|parent| depths[parent] + 1)
                    .max()
                    .unwrap_or(0);
                depths[index] = depths[index].max(deepest.min(chunks.len()));
            }
        }
        let columns = depths.iter().copied().max().map_or(1, |d| d + 1);
        let mut rows_at = vec![0usize; columns];
        let mut row_of = vec![0usize; chunks.len()];
        for (index, depth) in depths.iter().enumerate() {
            row_of[index] = rows_at[*depth];
            rows_at[*depth] += 1;
        }
        let (w, h) = size;
        let x0 = w * MARGIN_X;
        let x1 = w * (1.0 - MARGIN_X);
        let y0 = h * MARGIN_Y;
        let y1 = h * (1.0 - MARGIN_Y);
        depths
            .iter()
            .zip(&row_of)
            .map(|(&depth, &row)| {
                let x = if columns > 1 {
                    x0 + (x1 - x0) * depth as f32 / (columns - 1) as f32
                } else {
                    (x0 + x1) * 0.5
                };
                let rows = rows_at[depth].max(1);
                let y = if rows > 1 {
                    y0 + (y1 - y0) * row as f32 / (rows - 1) as f32
                } else {
                    (y0 + y1) * 0.5
                };
                (x, y)
            })
            .collect()
    }

    /// Draw `proposal` into `list`, each star at `alphas[index]` of its
    /// ink and each edge at the lesser of its two stars'.
    pub fn show(&self, proposal: &Proposal, alphas: &[f32], list: &mut DrawList) {
        if self.field.width <= 0.0 || self.field.height <= 0.0 {
            return;
        }
        let placed = Self::place(
            proposal,
            (
                self.field.width / self.scale,
                self.field.height / self.scale,
            ),
        );
        let alpha_of = |index: usize| alphas.get(index).copied().unwrap_or(1.0);
        let point = |index: usize| {
            let (x, y) = placed[index];
            (
                self.scale.mul_add(x, self.field.x),
                self.scale.mul_add(y, self.field.y),
            )
        };
        for (to, chunk) in proposal.chunks.iter().enumerate() {
            for dep in &chunk.deps {
                let Some(from) = proposal.chunks.iter().position(|c| &c.id == dep) else {
                    continue;
                };
                let alpha = alpha_of(from).min(alpha_of(to));
                if alpha <= 0.0 {
                    continue;
                }
                self.edge(point(from), point(to), alpha, list);
            }
        }
        for (index, chunk) in proposal.chunks.iter().enumerate() {
            let alpha = alpha_of(index);
            if alpha <= 0.0 {
                continue;
            }
            let hue = self.hue(proposal.hue_of(&chunk.group));
            let full = if self.medium == Medium::Day {
                DAY_ALPHA
            } else {
                1.0
            };
            let r = STAR_PX * self.scale;
            let (x, y) = point(index);
            list.instances.push(Instance::rect(
                x - r,
                y - r,
                r * 2.0,
                r * 2.0,
                r,
                Srgba::new(hue.r, hue.g, hue.b, alpha * full),
            ));
        }
    }

    /// The hue a workstream at `index` wears — the volume's three, then
    /// round again.
    #[must_use]
    pub fn hue(&self, index: usize) -> Srgba {
        Self::hue_in(&self.lex, index)
    }

    /// The hue for workstream `index` in `lex`: mauve, blue, teal, and
    /// round again.
    #[must_use]
    pub fn hue_in(lex: &Lexicon, index: usize) -> Srgba {
        match index % 3 {
            0 => lex.mauve,
            1 => lex.blue,
            _ => lex.teal,
        }
    }

    /// One edge, walked as a brush of round stamps at the ink pitch —
    /// there is no line primitive and an instance does not rotate.
    fn edge(&self, from: (f32, f32), to: (f32, f32), alpha: f32, list: &mut DrawList) {
        let width = EDGE_PX * self.scale;
        let ink = Srgba::new(self.lex.edge.r, self.lex.edge.g, self.lex.edge.b, alpha);
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = libm::hypotf(dx, dy);
        if length <= 0.0 {
            return;
        }
        let stamps = ((length / width).ceil() as usize).min(STAMPS_MAX);
        for n in 0..=stamps {
            let t = (n as f32 * width / length).min(1.0);
            let at = (dx.mul_add(t, from.0), dy.mul_add(t, from.1));
            let r = width * 0.5;
            list.instances
                .push(Instance::rect(at.0 - r, at.1 - r, width, width, r, ink));
        }
    }
}

#[cfg(test)]
#[path = "chart.tests.rs"]
mod tests;
