//! The native viewports shared by paint, pointer focus and accessible bounds.
use crate::comp_roadmap::RoadmapComposition as R;
use crate::comp_root::RootComposition;
use crate::comp_sky::SkyComposition;
use crate::comp_workspace::WorkspaceComposition as W;
use crate::{Element, ElementLayout};
use std::ops::Range;

pub(crate) struct FrameClips(pub Vec<(Range<usize>, [f32; 4])>);

impl FrameClips {
    pub fn of(tree: &Element, layout: &ElementLayout) -> Self {
        let mut regions: Vec<_> = [
            RootComposition::STAGE,
            crate::comp_composer::ComposerComposition::PILL,
            W::REASONING,
            W::NEARBY,
            W::DETAILS,
            R::LIST,
            R::REQUESTS,
            R::INPUT,
            SkyComposition::FIELD,
        ]
        .into_iter()
        .filter_map(|id| {
            let range = crate::frame::subtree_of(tree, id)?;
            let rect = layout.rect(range.start);
            Some((range, rect))
        })
        .collect();
        regions.sort_by_key(|(range, _)| range.start);
        Self(regions)
    }
    pub fn visible(&self, index: usize, mut rect: [f32; 4]) -> Option<[f32; 4]> {
        for (range, clip) in &self.0 {
            if range.contains(&index) {
                rect = crate::frame::intersect(rect, *clip)?;
            }
        }
        Some(rect)
    }
}
