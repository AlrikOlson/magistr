//! Accessibility requests resolve against the same placed controls as native input.
use super::Frame;
use accesskit::{Action, ActionData, ActionRequest, TreeId};
use magistr_cockpit::cockpit::{Model, Msg, Widget};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};

pub(super) fn is_available(model: &Model, target: magistr_cockpit::widget_id::WidgetId) -> bool {
    if model.palette().is_some() {
        return target == Widget::OverlayQuery.id() || Widget::palette_row_index(target).is_some();
    }
    if model.search().is_some() {
        return target == Widget::OverlayQuery.id()
            || Widget::search_row_index(target).is_some()
            || Widget::search_scope_index(target).is_some();
    }
    if model.moment().is_some() || model.review().is_some() || model.ask().is_some() {
        return Widget::from_id(target).is_some_and(|w| {
            w.answers().is_some()
                || matches!(
                    w,
                    Widget::ShowImages
                        | Widget::Stage
                        | Widget::Accept
                        | Widget::Reject
                        | Widget::Clear
                        | Widget::Checkpoint
                )
        }) || Widget::review_file_index(target).is_some();
    }
    true
}

pub(super) fn limit_controls(root: &mut crate::Element, model: &Model) {
    if root
        .id
        .and_then(|id| crate::RootComposition::target_of(model, id))
        .is_some_and(|target| !is_available(model, target))
    {
        root.focusable = false;
        root.walkable = false;
    }
    for child in &mut root.children {
        limit_controls(child, model);
    }
}

impl Frame {
    pub(super) fn focus_widget(&mut self, target: magistr_cockpit::widget_id::WidgetId) {
        if !self
            .access_tree
            .nodes()
            .iter()
            .any(|n| n.target == Some(target) && n.actions.contains(&Action::Focus))
        {
            return;
        }
        let mut id = None;
        if let Some(root) = &self.tree {
            root.each(&mut |index, element| {
                if self.targets.get(index) == Some(&Some(target)) {
                    id = element.id;
                }
            });
        }
        if let Some(id) = id {
            self.state.focus_on(id);
        }
    }
    pub(super) fn control_key(&mut self, model: &Model, key: KeyEvent) -> Option<Vec<Msg>> {
        if !key.mods.is_empty() {
            return None;
        }
        let target = self.focus_target()?;
        let node = self
            .access_tree
            .nodes()
            .iter()
            .find(|n| n.target == Some(target))?;
        let action = match key.code {
            KeyCode::Enter | KeyCode::Char(' ') if node.actions.contains(&Action::Click) => {
                Action::Click
            }
            KeyCode::Left | KeyCode::Up if node.actions.contains(&Action::Decrement) => {
                Action::Decrement
            }
            KeyCode::Right | KeyCode::Down if node.actions.contains(&Action::Increment) => {
                Action::Increment
            }
            _ => return None,
        };
        let request = ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: node.id,
            data: None,
        };
        Some(self.access_action(model, request).unwrap_or_default())
    }

    /// Translate a supported request; refused requests never change focus or the model.
    pub fn access_action(
        &mut self,
        model: &Model,
        request: ActionRequest,
    ) -> Result<Vec<Msg>, String> {
        if request.target_tree != TreeId::ROOT {
            return Err("This control belongs to another window.".into());
        }
        let tree = self.access();
        let node = tree
            .nodes()
            .iter()
            .find(|node| node.id == request.target_node)
            .ok_or("This control is no longer available.")?;
        if node.disabled {
            return Err("This control is unavailable.".into());
        }
        if !node.actions.contains(&request.action) {
            return Err("This control does not support that action.".into());
        }
        let target = node.target.ok_or("This control is no longer available.")?;
        if !is_available(model, target) {
            return Err("Close the open dialog before using this control.".into());
        }
        if matches!(
            request.action,
            Action::Focus | Action::Click | Action::Increment | Action::Decrement
        ) && request.data.is_some()
        {
            return Err("This control does not accept a value for that action.".into());
        }
        let mut element_id = None;
        if let Some((root, _)) = self.placed() {
            root.each(&mut |index, element| {
                if self.targets.get(index) == Some(&Some(target)) {
                    element_id = element.id;
                }
            });
        }
        let element_id = element_id.ok_or("This control is no longer available.")?;
        let (x0, y0, x1, y1) = node.bounds.ok_or("This control has no visible area.")?;
        let (x, y) = (((x0 + x1) / 2.0) as u32, ((y0 + y1) / 2.0) as u32);
        let (col, row) = self
            .metrics
            .cell_at(f64::from(x), f64::from(y), model.size());
        let mouse = |kind, x, y| Msg::Mouse {
            event: MouseEvent::new(kind, col, row).at_px(x, y),
            target: Some(target),
        };
        let messages = match request.action {
            Action::SetValue if target == Widget::OverlayQuery.id() => {
                let Some(ActionData::Value(value)) = request.data else {
                    return Err("Enter text for this control.".into());
                };
                let query = model
                    .palette()
                    .map(|p| p.query())
                    .or_else(|| model.search().map(|s| s.query()))
                    .or_else(|| model.settings().map(|s| s.query()))
                    .ok_or("This search is no longer open.")?;
                std::iter::repeat_n(KeyCode::Backspace, query.chars().count())
                    .chain(value.chars().filter(|c| !c.is_control()).map(KeyCode::Char))
                    .map(|code| Msg::Key(KeyEvent::new(code)))
                    .collect()
            }
            Action::Focus => Vec::new(),
            Action::Click => vec![
                mouse(MouseKind::Down(Button::Left), x, y),
                mouse(MouseKind::Up(Button::Left), x, y),
            ],
            Action::SetValue
                if target == Widget::Composer.id() || target == Widget::Prompt.id() =>
            {
                let Some(ActionData::Value(value)) = request.data else {
                    return Err("Enter text for this control.".into());
                };
                if model.palette().is_some()
                    || model.settings().is_some()
                    || model.search().is_some()
                    || model.moment().is_some()
                {
                    return Err("Close the open dialog before editing the message.".into());
                }
                vec![Msg::PromptReplace(value.into())]
            }
            Action::Increment | Action::Decrement => {
                let delta = if request.action == Action::Increment {
                    20
                } else {
                    -20
                };
                let (end_x, end_y) = if target == Widget::PanelColumns.id() {
                    (x.saturating_add_signed(delta), y)
                } else {
                    (x, y.saturating_add_signed(delta))
                };
                vec![
                    mouse(MouseKind::Down(Button::Left), x, y),
                    mouse(MouseKind::Drag(Button::Left), end_x, end_y),
                    mouse(MouseKind::Up(Button::Left), end_x, end_y),
                ]
            }
            Action::ScrollUp | Action::ScrollDown => {
                let kind = if request.action == Action::ScrollUp {
                    MouseKind::ScrollUp
                } else {
                    MouseKind::ScrollDown
                };
                let count = match request.data {
                    None | Some(ActionData::ScrollUnit(accesskit::ScrollUnit::Item)) => 1,
                    Some(ActionData::ScrollUnit(accesskit::ScrollUnit::Page)) => {
                        ((y1 - y0) / f64::from(self.metrics.height())).max(1.0) as usize
                    }
                    _ => return Err("This scroll amount is unsupported.".into()),
                };
                (0..count.min(1000)).map(|_| mouse(kind, x, y)).collect()
            }
            _ => return Err("This control does not support that action.".into()),
        };
        if node.actions.contains(&Action::Focus) {
            self.state.focus_on(element_id);
        }
        Ok(messages)
    }
}
