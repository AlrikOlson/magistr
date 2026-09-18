//! Native workspace measurement and clipped emission, after the element layout.
use super::{Element, ElementLayout, Frame, Model, RootComposition, emit, index_of, scissor_of};
use crate::comp_workspace::WorkspaceComposition as W;
use crate::frame_clip::FrameClips;
use crate::workspace_layout::WorkspaceLayout as L;
use magistr_cockpit::cockpit::{
    flight_divider::FlightDivider as D, panel_allocation::PanelAllocation,
};

impl Frame {
    pub(super) fn workspace_report(
        &self,
        model: &mut Model,
        tree: &Element,
        layout: &ElementLayout,
        factor: f32,
    ) -> bool {
        let rect = |id| index_of(tree, id).map(|index| layout.rect(index));
        let mut panels = [None; 4];
        for (divider, owner, first, minimums) in [
            (D::Columns, W::BODY, W::LEFT, L::COLUMNS_MIN),
            (D::Reasoning, W::LEFT, W::CONVERSATION, L::REASONING_MIN),
            (D::Roadmap, W::RIGHT, W::NEARBY, L::DETAILS_MIN),
        ] {
            if rect(W::id(divider.widget().id())).is_none() {
                continue;
            }
            let (Some(owner), Some(first)) = (rect(owner), rect(first)) else {
                continue;
            };
            let axis = if divider == D::Columns { 0 } else { 1 };
            panels[divider.index()] = Some(PanelAllocation {
                origin: owner[axis],
                total: (owner[axis + 2] - L::GAP * factor).max(1.0),
                first: first[axis + 2],
                minimums: (minimums.0 * factor, minimums.1 * factor),
            });
        }
        let stage = rect(RootComposition::STAGE).map(|r| {
            (
                (r[2] / self.metrics.width())
                    .floor()
                    .clamp(1.0, u16::MAX as f32) as u16,
                (r[3] / self.metrics.height()).floor().max(1.0) as usize,
            )
        });
        let details = rect(W::DETAILS)
            .zip(rect(W::DETAIL_CONTENT))
            .map(|(view, content)| (content[3].ceil() as usize, view[3].floor() as usize));
        model.report_native_workspace(panels, stage, details)
    }

    pub(super) fn paint_workspace(
        &mut self,
        model: &Model,
        tree: &Element,
        layout: &ElementLayout,
        clips: &FrameClips,
        viewport: [u32; 2],
    ) {
        let mut from = 0;
        for (range, rect) in &clips.0 {
            emit(
                tree,
                layout,
                &self.state,
                &mut self.text,
                &mut self.list,
                from..range.start,
            );
            self.list.end_batch(None, true);
            if let Some(scissor) = scissor_of(*rect, viewport) {
                emit(
                    tree,
                    layout,
                    &self.state,
                    &mut self.text,
                    &mut self.list,
                    range.clone(),
                );
                if tree_id(tree, range.start) == Some(RootComposition::STAGE) {
                    self.pictures(model, tree, layout);
                }
                self.list.end_batch(Some(scissor), true);
            }
            from = range.end;
        }
        emit(
            tree,
            layout,
            &self.state,
            &mut self.text,
            &mut self.list,
            from..tree.count(),
        );
    }
}

fn tree_id(tree: &Element, at: usize) -> Option<crate::ElementId> {
    let mut found = None;
    tree.each(&mut |index, element| {
        if index == at {
            found = element.id;
        }
    });
    found
}
