//! Request documents, controls and source-anchored reading change together.
//! The transcript is derived from admitted Turn ranges, never a second stored response.
use super::{Cmd, Model, Pane, ReadingPosition, ReadingRow, RequestPhase, Widget};
use crate::input::{KeyCode, KeyEvent};
use crate::layout::TextRole;
use crate::theme::ThemeMode;
use crate::turn::{ConversationCard, LineKind, Row, RowKind, SpanStyle, TextDocument, Voice};
use crate::widget_id::WidgetId;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestSection {
    pub key: String,
    pub heading: String,
    pub text: Rc<TextDocument>,
    pub action: Option<WidgetId>,
}

type RequestGeometry = ((u32, u64), Rc<Vec<ReadingRow>>);

/// Shared semantic content; each surface measures it with its own ruler.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestHistory {
    pub sections: Vec<RequestSection>,
    pub links: Vec<(String, String)>,
    original: String,
    tools: Vec<String>,
    geometry: std::cell::RefCell<Option<RequestGeometry>>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct RequestReading {
    cache: Option<(u64, Rc<RequestHistory>)>,
    documents: BTreeMap<String, (ThemeMode, Rc<TextDocument>)>,
    pub anchor: Option<ReadingPosition>,
    pub focused_link: Option<usize>,
    pub native_positions: Vec<(ReadingPosition, usize)>,
    pub native_inset: usize,
}

impl PartialEq for RequestReading {
    fn eq(&self, other: &Self) -> bool {
        self.anchor == other.anchor
            && self.focused_link == other.focused_link
            && self.native_positions == other.native_positions
            && self.native_inset == other.native_inset
    }
}

impl RequestHistory {
    #[must_use]
    pub fn of(model: &Model) -> Rc<Self> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        model.theme().mode().hash(&mut hash);
        for request in model.roadmap_requests() {
            request.text.hash(&mut hash);
            request.phase.label().hash(&mut hash);
            request.error.hash(&mut hash);
            request.source_start.hash(&mut hash);
            for line in request.response(model.turn()) {
                line.text().hash(&mut hash);
                line.has_newline().hash(&mut hash);
                line.title().hash(&mut hash);
                line.record().locations.hash(&mut hash);
                line.record().text.hash(&mut hash);
                line.record()
                    .input
                    .as_ref()
                    .map(|value| value.content())
                    .hash(&mut hash);
                if matches!(line.kind(), LineKind::Image(_)) {
                    std::mem::discriminant(line.kind()).hash(&mut hash);
                } else {
                    line.kind().hash(&mut hash);
                }
            }
        }
        let key = hash.finish();
        let mut state = model.request_reading.borrow_mut();
        if let Some((cached, history)) = &state.cache
            && *cached == key
        {
            return Rc::clone(history);
        }
        let mut history = Self::default();
        for (index, request) in model.roadmap_requests().iter().enumerate() {
            history.add(
                model,
                &mut state,
                format!("request:{index}"),
                "You".into(),
                request.text.clone(),
                false,
            );
            history.original.push_str(&format!(
                "You\n{}\n\nMagistr · {}\n",
                request.text,
                request.phase.label()
            ));
            let notice = match request.phase {
                RequestPhase::Queued => {
                    "This request will start after the current work finishes. You can cancel it below."
                }
                RequestPhase::Waiting => "Waiting for the agent to respond.",
                _ => "",
            };
            history.add(
                model,
                &mut state,
                format!("status:{index}"),
                format!("Magistr · {}", request.phase.label()),
                notice.into(),
                false,
            );
            let lines = request.response(model.turn());
            let mut at = 0;
            while at < lines.len() {
                let start = at;
                if matches!(lines[at].kind(), LineKind::Prose(Voice::Agent)) {
                    let mut source = String::new();
                    while at < lines.len()
                        && matches!(lines[at].kind(), LineKind::Prose(Voice::Agent))
                    {
                        source.push_str(lines[at].text());
                        if lines[at].has_newline()
                            || at + 1 < lines.len()
                                && matches!(lines[at + 1].kind(), LineKind::Prose(Voice::Agent))
                        {
                            source.push('\n');
                        }
                        at += 1;
                    }
                    history.original.push_str(&source);
                    history.add(
                        model,
                        &mut state,
                        format!(
                            "reply:{index}:{}",
                            request.source_start.unwrap_or(0) + start
                        ),
                        String::new(),
                        source,
                        false,
                    );
                    continue;
                }
                let line = &lines[at];
                if let Some(card) = ConversationCard::of(model.turn(), line) {
                    let detail = if matches!(line.kind(), LineKind::Check { .. }) {
                        let input = line.record().input.as_ref().map(|value| value.content());
                        input.map_or_else(
                            || line.text().to_owned(),
                            |command| format!("Command: {command}\n{}", line.text()),
                        )
                    } else {
                        card.detail
                    };
                    history.add(
                        model,
                        &mut state,
                        format!("card:{index}:{at}"),
                        format!("{} · {}", card.title, card.status),
                        detail,
                        true,
                    );
                    if let Some(id) = super::session::tool_identity(line) {
                        let target = Widget::request_tool(history.tools.len());
                        history.tools.push(id.into());
                        if let Some(section) = history.sections.last_mut() {
                            section.action = Some(target);
                        }
                    }
                    history.original.push_str(line.text());
                    history.original.push('\n');
                } else if matches!(
                    line.kind(),
                    LineKind::Error | LineKind::Note | LineKind::Image(_)
                ) {
                    history.add(
                        model,
                        &mut state,
                        format!("note:{index}:{at}"),
                        String::new(),
                        line.text().into(),
                        true,
                    );
                    history.original.push_str(line.text());
                    history.original.push('\n');
                }
                at += 1;
            }
            if let Some(error) = &request.error {
                history.add(
                    model,
                    &mut state,
                    format!("error:{index}"),
                    "Could not finish".into(),
                    error.clone(),
                    true,
                );
                history.original.push_str(error);
            }
            history.original.push_str("\n\n");
        }
        for section in &history.sections {
            for block in section.text.blocks() {
                let rows = std::iter::once(&block.row)
                    .chain(block.cell_rows().flat_map(|(label, value)| [label, value]));
                for row in rows {
                    for span in &row.spans {
                        if let SpanStyle::Link(address) = &span.style {
                            let label = row.text.get(span.start..span.end).unwrap_or(address);
                            let pair = (label.to_owned(), address.clone());
                            if !history.links.contains(&pair) {
                                history.links.push(pair);
                            }
                        }
                    }
                }
            }
        }
        state
            .documents
            .retain(|key, _| history.sections.iter().any(|section| section.key == *key));
        let history = Rc::new(history);
        state.cache = Some((key, Rc::clone(&history)));
        history
    }

    fn add(
        &mut self,
        model: &Model,
        state: &mut RequestReading,
        key: String,
        heading: String,
        source: String,
        raw: bool,
    ) {
        let mode = model.theme().mode();
        let document = state
            .documents
            .get(&key)
            .filter(|(cached_mode, doc)| *cached_mode == mode && doc.source() == source)
            .map(|(_, doc)| Rc::clone(doc))
            .unwrap_or_else(|| {
                let doc = Rc::new(if raw {
                    TextDocument::code(source, "", mode)
                } else {
                    TextDocument::markdown(source, mode)
                });
                state.documents.insert(key.clone(), (mode, Rc::clone(&doc)));
                doc
            });
        self.sections.push(RequestSection {
            key,
            heading,
            text: document,
            action: None,
        });
    }

    #[must_use]
    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn rows(
        &self,
        width: f32,
        ruler: u64,
        measure: &mut impl FnMut(&str, TextRole) -> f32,
    ) -> Rc<Vec<ReadingRow>> {
        if let Some((cached, rows)) = &*self.geometry.borrow()
            && *cached == (width.to_bits(), ruler)
        {
            return Rc::clone(rows);
        }
        let mut rows = Vec::new();
        for section in &self.sections {
            if !section.heading.is_empty() {
                rows.push(ReadingRow {
                    row: Row::new(RowKind::Heading, &section.heading),
                    position: ReadingPosition {
                        section: section.key.clone(),
                        source: None,
                    },
                    target: None,
                });
            }
            for row in section.text.rows(width, measure) {
                rows.push(ReadingRow {
                    row: row.row,
                    position: ReadingPosition {
                        section: section.key.clone(),
                        source: Some(row.anchor),
                    },
                    target: None,
                });
            }
        }
        for section in &self.sections {
            if let Some(target) = section.action {
                rows.push(ReadingRow {
                    row: Row::new(
                        RowKind::Prose(Voice::Agent),
                        format!("{} · Open details", section.heading),
                    ),
                    position: ReadingPosition {
                        section: section.key.clone(),
                        source: None,
                    },
                    target: Some(target),
                });
            }
        }
        for (index, (label, address)) in self.links.iter().enumerate() {
            rows.push(ReadingRow {
                row: Row::new(
                    RowKind::Prose(Voice::Agent),
                    format!("{label} · {address} · Copy link"),
                ),
                position: ReadingPosition {
                    section: "links".into(),
                    source: None,
                },
                target: Some(Widget::request_link(index)),
            });
        }
        let rows = Rc::new(rows);
        *self.geometry.borrow_mut() = Some(((width.to_bits(), ruler), Rc::clone(&rows)));
        rows
    }

    #[must_use]
    pub fn top(model: &Model, rows: &[ReadingRow], visible: usize) -> usize {
        model
            .request_reading
            .borrow()
            .anchor
            .as_ref()
            .map_or(model.roadmap_request_scroll(), |at| {
                super::Inspection::row_of(rows, at)
            })
            .min(rows.len().saturating_sub(visible))
    }

    fn jump(model: &mut Model, next: bool) {
        let history = Self::of(model);
        let mut headings = Vec::new();
        for section in &history.sections {
            if !section.heading.is_empty() {
                headings.push(ReadingPosition {
                    section: section.key.clone(),
                    source: None,
                });
            }
            for block in section
                .text
                .blocks()
                .iter()
                .filter(|block| block.heading > 0)
            {
                headings.push(ReadingPosition {
                    section: section.key.clone(),
                    source: Some(block.anchor),
                });
            }
        }
        let current = model.request_reading.borrow().anchor.clone();
        let at = current.as_ref().and_then(|position| {
            headings.iter().rposition(|heading| {
                history
                    .sections
                    .iter()
                    .position(|s| s.key == heading.section)
                    < history
                        .sections
                        .iter()
                        .position(|s| s.key == position.section)
                    || heading.section == position.section && heading.source <= position.source
            })
        });
        let index = if next {
            at.map_or(0, |at| (at + 1).min(headings.len().saturating_sub(1)))
        } else {
            at.unwrap_or(headings.len()).saturating_sub(1)
        };
        if let Some(position) = headings.get(index) {
            let mut state = model.request_reading.borrow_mut();
            state.anchor = Some(position.clone());
            state.native_inset = 0;
            model.roadmap_request_scroll = 0;
        }
    }

    pub fn latest(model: &mut Model) {
        model.roadmap_request_scroll = usize::MAX;
        let mut state = model.request_reading.borrow_mut();
        state.anchor = None;
        state.native_inset = 0;
        state.focused_link = None;
    }

    pub fn activate(model: &mut Model, id: WidgetId) -> Option<Vec<Cmd>> {
        if model.tab() != super::StageTab::Roadmap || !model.roadmap_requests_open() {
            return None;
        }
        if let Some(index) = Widget::request_tool_index(id) {
            if let Some(tool) = Self::of(model).tools.get(index) {
                model.inspect(super::Selected::Tool(tool.clone()));
                model.roadmap_requests_open = false;
                model.pane = Pane::Inspector;
            }
            return Some(Vec::new());
        }
        if id == Widget::request_jump(0) || id == Widget::request_jump(1) {
            Self::jump(model, id == Widget::request_jump(1));
            return Some(Vec::new());
        }
        if id == Widget::request_copy() {
            return Some(vec![Cmd::Clipboard(Self::of(model).original().into())]);
        }
        if let Some(index) = Widget::request_link_index(id) {
            return Some(
                Self::of(model)
                    .links
                    .get(index)
                    .map(|(_, address)| vec![Cmd::Clipboard(address.clone())])
                    .unwrap_or_default(),
            );
        }
        None
    }

    #[must_use]
    pub fn is_focused_link(model: &Model, id: WidgetId) -> bool {
        Widget::request_link_index(id)
            .is_some_and(|at| model.request_reading.borrow().focused_link == Some(at))
    }

    pub fn key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
        if model.tab() != super::StageTab::Roadmap
            || !model.roadmap_requests_open()
            || model.pane() != Pane::Inspector
            || !key.mods.is_empty()
        {
            return None;
        }
        match key.code {
            KeyCode::Char('c') if model.prompt().is_empty() => {
                Self::activate(model, Widget::request_copy())
            }
            KeyCode::Char('[' | ']') if model.prompt().is_empty() => {
                Self::jump(model, key.code == KeyCode::Char(']'));
                Some(Vec::new())
            }
            KeyCode::Tab => {
                let count = Self::of(model).links.len();
                if count > 0 {
                    let mut state = model.request_reading.borrow_mut();
                    if state.focused_link.is_some_and(|at| at + 1 >= count) {
                        state.focused_link = None;
                        return None;
                    }
                    state.focused_link = Some(state.focused_link.map_or(0, |at| at + 1));
                    state.anchor = Some(ReadingPosition {
                        section: "links".into(),
                        source: None,
                    });
                    model.roadmap_request_scroll = usize::MAX - 1;
                }
                Some(Vec::new())
            }
            KeyCode::Enter if model.request_reading.borrow().focused_link.is_some() => {
                let at = model.request_reading.borrow().focused_link.unwrap_or(0);
                Self::activate(model, Widget::request_link(at))
            }
            _ => None,
        }
    }

    pub fn report_native_positions(model: &mut Model, positions: Vec<(ReadingPosition, usize)>) {
        let mut state = model.request_reading.borrow_mut();
        if let Some(anchor) = &state.anchor
            && let Some((_, y)) = positions
                .iter()
                .filter(|(p, _)| p.section == anchor.section)
                .rfind(|(p, _)| match (p.source, anchor.source) {
                    (at, None) => at.is_none(),
                    (Some(at), Some(source)) => {
                        at.line < source.line || at.line == source.line && at.byte <= source.byte
                    }
                    _ => false,
                })
        {
            model.roadmap_request_scroll = y.saturating_add(state.native_inset);
        }
        state.native_positions = positions;
    }
}

#[cfg(test)]
#[path = "request_history.tests.rs"]
mod tests;
