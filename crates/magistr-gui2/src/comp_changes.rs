//! Native journal and selected file details over the shared source model.
use crate::comp_root::RootComposition;
use crate::comp_workspace::{WorkspaceComposition as W, control, heading, panel};
use crate::comp_workspace_details::DetailsComposition;
use crate::{Element, Lexicon, Medium, Voice};
use magistr_cockpit::cockpit::{Model, Selected, Widget};

pub struct ChangesComposition;

impl ChangesComposition {
    pub fn el(model: &Model, medium: Medium, cell: (f32, f32), viewport: (f32, f32)) -> Element {
        let lex = Lexicon::of(medium);
        let compact = viewport.0 < 700.0;
        let height = (viewport.1 - 24.0).max(1.0);
        let list_height = if compact { height * 0.4 } else { height };
        let detail_height = if compact {
            height - list_height - 12.0
        } else {
            height
        };
        let mut rows = Element::surface()
            .grow(1.0)
            .height(0.0)
            .min_height(0.0)
            .id(RootComposition::STAGE)
            .label("Changes");
        if model.changes().entries().is_empty() {
            rows = rows.child(Element::text(Voice::STEER, lex.sb0, "No recorded changes."));
        }
        for (index, entry) in model
            .changes()
            .entries()
            .iter()
            .enumerate()
            .skip(model.session_top())
            .take(model.stage_rows().max(1))
        {
            let selected = model.selected() == Some(&Selected::Change(entry.identity()));
            let row = Element::surface()
                .fill(if selected { lex.s0 } else { lex.mantle })
                .child(
                    control(&entry.path, Widget::change_row(index), &lex)
                        .fill(if selected { lex.tx } else { lex.mauve })
                        .height(cell.1)
                        .no_shrink()
                        .min_width(0.0)
                        .stretch(),
                );
            rows = rows.child(row);
        }
        let list = panel(list_height, &lex)
            .child(
                heading("Changes", &lex)
                    .child(Element::row().grow(1.0))
                    .child(control("Conversation", Widget::TabTurn.id(), &lex)),
            )
            .child(rows);
        let details = DetailsComposition::el(model, detail_height.max(1.0), &lex);
        let body = if compact {
            Element::surface().gap(12.0).child(list).child(details)
        } else {
            Element::row()
                .gap(12.0)
                .child(list.width((viewport.0 - 36.0) * 0.4).no_shrink())
                .child(details.grow(1.0).min_width(0.0))
        };
        body.pad_x(12.0)
            .pad_y(12.0)
            .grow(1.0)
            .min_height(0.0)
            .id(W::BODY)
    }
}
