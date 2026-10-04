//! A selected record's sections, links and source position form one reading contract.
//! All surfaces consume this derived document; only surfaces choose geometry and paint.
use super::{Model, Selected, Widget};
use crate::pixel::Icon;
use crate::turn::{DocumentAnchor, TextDocument};
use crate::widget_id::WidgetId;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

#[path = "inspection_records.rs"]
mod records;

/// A resolved record, a copyable address, or a reference that no longer exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordTarget {
    Record(Selected),
    Address(String),
    Unavailable,
}

/// A named destination. Original references are retained only for resolving spans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordLink {
    pub label: String,
    pub reference: String,
    pub target: RecordTarget,
    pub widget: WidgetId,
}

/// A stable named section with authored text and optional secondary details.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordSection {
    pub key: String,
    pub heading: String,
    pub text: TextDocument,
    pub secondary: bool,
    pub links: Vec<RecordLink>,
    pub actions: Vec<(&'static str, WidgetId)>,
}

/// Where reading stands in source, rather than at a surface's row number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadingPosition {
    pub section: String,
    pub source: Option<DocumentAnchor>,
}

/// The complete transient presentation of a selected record.
#[derive(Clone, Debug)]
pub struct Inspection {
    pub title: String,
    pub status: String,
    pub icon: Icon,
    pub metadata: String,
    pub actions: Vec<(&'static str, WidgetId)>,
    pub sections: Vec<RecordSection>,
    pub viewer: bool,
    layout: std::cell::RefCell<Option<(u64, Rc<Vec<ReadingRow>>)>>,
}

/// One measured reading row, with a stable source location and an optional control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadingRow {
    pub row: crate::turn::Row,
    pub position: ReadingPosition,
    pub target: Option<WidgetId>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct InspectionState {
    cache: Option<(u64, Rc<Inspection>)>,
    saved: HashMap<Selected, ReadingBookmark>,
    pub expanded: HashSet<String>,
    pub anchor: Option<ReadingPosition>,
    pub focused_link: Option<usize>,
    pub native_positions: Vec<(ReadingPosition, usize)>,
    #[cfg(test)]
    pub builds: usize,
}

/// Source reading belongs to record identity; measured surface positions do not.
#[derive(Clone, Debug, Default)]
struct ReadingBookmark {
    expanded: HashSet<String>,
    anchor: Option<ReadingPosition>,
    focused_link: Option<usize>,
    offset: usize,
}

impl InspectionState {
    pub fn select(&mut self, previous: Option<&Selected>, next: &Selected, offset: &mut usize) {
        let leaving = ReadingBookmark {
            expanded: std::mem::take(&mut self.expanded),
            anchor: self.anchor.take(),
            focused_link: self.focused_link.take(),
            offset: std::mem::take(offset),
        };
        if let Some(previous) = previous {
            self.saved.insert(previous.clone(), leaving);
        }
        let returning = self.saved.remove(next).unwrap_or_default();
        self.expanded = returning.expanded;
        self.anchor = returning.anchor;
        self.focused_link = returning.focused_link;
        *offset = returning.offset;
        self.native_positions.clear();
    }
}

impl Inspection {
    /// Cache semantic parsing by the selected source and its named relationships.
    /// Frame ticks, pointer motion and viewport geometry are not content revisions.
    #[must_use]
    pub fn of(model: &Model) -> Option<Rc<Self>> {
        let key = records::fingerprint(model).or_else(|| {
            let selected = model.selected()?;
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            (
                "unavailable",
                selected,
                format!("{:?}", model.theme().mode()),
            )
                .hash(&mut hash);
            Some(hash.finish())
        })?;
        let mut state = model.inspection_state.borrow_mut();
        if let Some((cached, card)) = &state.cache
            && *cached == key
            && card.links().iter().all(|link| {
                let resolved = records::link(model, &link.reference, &link.label, 0);
                resolved.target == link.target
                    && (resolved.label == link.label
                        || records::with_track_label(model, resolved).label == link.label)
            })
        {
            return Some(Rc::clone(card));
        }
        let card = Rc::new(records::build(model).unwrap_or_else(|| {
            let mut card = Self::blank();
            card.title = "Item unavailable".into();
            card.status = "Unavailable".into();
            card.section(
                model,
                "Unavailable",
                "This item is no longer available. Choose another item to continue.",
                false,
            );
            card
        }));
        state.cache = Some((key, Rc::clone(&card)));
        #[cfg(test)]
        {
            state.builds += 1;
        }
        Some(card)
    }

    /// Authored names keep formatting; tool labels and paths keep their literal bytes.
    #[must_use]
    pub fn title_document(&self, model: &Model) -> TextDocument {
        if matches!(
            model.selected(),
            Some(super::Selected::Tool(_) | super::Selected::Change(_) | super::Selected::Run(_))
        ) {
            TextDocument::code(&self.title, "", model.theme().mode())
        } else {
            TextDocument::markdown(&self.title, model.theme().mode())
        }
    }

    pub(super) fn blank() -> Self {
        Self {
            title: String::new(),
            status: String::new(),
            icon: Icon::Pending,
            metadata: String::new(),
            actions: Vec::new(),
            sections: Vec::new(),
            viewer: false,
            layout: std::cell::RefCell::default(),
        }
    }

    pub(super) fn section(&mut self, model: &Model, heading: &str, source: &str, secondary: bool) {
        if source.trim().is_empty() {
            return;
        }
        let text = TextDocument::markdown(source, model.theme().mode());
        self.add_section(model, heading, text, secondary);
    }

    pub(super) fn code(
        &mut self,
        model: &Model,
        heading: &str,
        source: &str,
        language: &str,
        secondary: bool,
    ) {
        if source.is_empty() {
            return;
        }
        let text = TextDocument::code(source, language, model.theme().mode());
        self.add_section(model, heading, text, secondary);
    }

    fn add_section(&mut self, model: &Model, heading: &str, text: TextDocument, secondary: bool) {
        let key = format!(
            "{}:{}",
            heading,
            self.sections
                .iter()
                .filter(|s| s.heading == heading)
                .count()
        );
        let mut section = RecordSection {
            key,
            heading: heading.to_owned(),
            text,
            secondary,
            links: Vec::new(),
            actions: Vec::new(),
        };
        for block in section.text.blocks() {
            for row in std::iter::once(&block.row)
                .chain(block.cell_rows().flat_map(|(label, value)| [label, value]))
            {
                for span in &row.spans {
                    if let crate::turn::SpanStyle::Link(reference) = &span.style
                        && !section
                            .links
                            .iter()
                            .any(|link| link.reference == *reference)
                    {
                        let index = self.link_count() + section.links.len();
                        section.links.push(records::link(
                            model,
                            reference,
                            &row.text[span.start..span.end],
                            index,
                        ));
                    }
                }
            }
        }
        self.sections.push(section);
    }

    pub(super) fn references(&mut self, model: &Model, heading: &str, references: &[String]) {
        if references.is_empty() {
            return;
        }
        let mut seen = HashSet::new();
        let mut links = Vec::new();
        for reference in references {
            let mut link = records::link(model, reference, "", self.link_count() + links.len());
            if matches!(heading, "Depends on" | "Needed by") {
                link = records::with_track_label(model, link);
            }
            if matches!(&link.target, RecordTarget::Record(selected) if Some(selected) == model.selected())
            {
                continue;
            }
            let identity = match &link.target {
                RecordTarget::Record(selected) => format!("{selected:?}"),
                _ => link.reference.clone(),
            };
            if seen.insert(identity) {
                links.push(link);
            }
        }
        if links.is_empty() {
            return;
        }
        self.sections.push(RecordSection {
            key: format!(
                "{heading}:{}",
                self.sections
                    .iter()
                    .filter(|s| s.heading == heading)
                    .count()
            ),
            heading: heading.into(),
            text: TextDocument::markdown("", model.theme().mode()),
            secondary: false,
            links,
            actions: Vec::new(),
        });
    }

    /// Cached terminal rows. The caller supplies its cell or embedded-font ruler.
    pub fn rows(
        &self,
        model: &Model,
        width: f32,
        geometry: u64,
        measure: &mut impl FnMut(&str, crate::layout::TextRole) -> f32,
    ) -> Rc<Vec<ReadingRow>> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        width.to_bits().hash(&mut hash);
        geometry.hash(&mut hash);
        let mut expanded = model
            .inspection_state
            .borrow()
            .expanded
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        expanded.sort();
        expanded.hash(&mut hash);
        let key = hash.finish();
        if let Some((cached, rows)) = &*self.layout.borrow()
            && *cached == key
        {
            return Rc::clone(rows);
        }
        let mut out = Vec::new();
        for (index, section) in self.sections.iter().enumerate() {
            let open = self.is_expanded(model, section);
            let position = ReadingPosition {
                section: section.key.clone(),
                source: None,
            };
            if !out.is_empty() {
                out.push(ReadingRow {
                    row: crate::turn::Row::new(
                        crate::turn::RowKind::Prose(crate::turn::Voice::Agent),
                        "",
                    ),
                    position: position.clone(),
                    target: None,
                });
            }
            out.push(ReadingRow {
                row: crate::turn::Row::new(
                    crate::turn::RowKind::Heading,
                    format!(
                        "{}{}",
                        section.heading,
                        if section.secondary {
                            if open { " · Hide" } else { " · Show" }
                        } else {
                            ""
                        }
                    ),
                ),
                position: position.clone(),
                target: section.secondary.then(|| Widget::detail_section(index)),
            });
            if !open {
                continue;
            }
            for row in section.text.rows(width, measure) {
                out.push(ReadingRow {
                    row: row.row,
                    position: ReadingPosition {
                        section: section.key.clone(),
                        source: Some(row.anchor),
                    },
                    target: None,
                });
            }
            for link in &section.links {
                let text = match &link.target {
                    RecordTarget::Record(_) => format!("{} →", link.label),
                    RecordTarget::Address(address) => {
                        format!("{} · {address} · Copy link", link.label)
                    }
                    RecordTarget::Unavailable => "This record is no longer available.".into(),
                };
                let doc = TextDocument::markdown(text, model.theme().mode());
                for row in doc.rows(width, measure) {
                    out.push(ReadingRow {
                        row: row.row,
                        position: position.clone(),
                        target: (link.target != RecordTarget::Unavailable).then_some(link.widget),
                    });
                }
            }
            for (label, target) in &section.actions {
                out.push(ReadingRow {
                    row: crate::turn::Row::new(
                        crate::turn::RowKind::Prose(crate::turn::Voice::Agent),
                        *label,
                    ),
                    position: position.clone(),
                    target: Some(*target),
                });
            }
        }
        let rows = Rc::new(out);
        *self.layout.borrow_mut() = Some((key, Rc::clone(&rows)));
        rows
    }

    #[must_use]
    pub fn row_of(rows: &[ReadingRow], position: &ReadingPosition) -> usize {
        let start = rows
            .iter()
            .position(|row| row.position.section == position.section)
            .unwrap_or(0);
        let Some(anchor) = position.source else {
            return start;
        };
        rows.iter()
            .enumerate()
            .filter(|(_, row)| row.position.section == position.section)
            .filter_map(|(index, row)| row.position.source.map(|source| (index, source)))
            .filter(|(_, source)| {
                source.line < anchor.line
                    || source.line == anchor.line && source.byte <= anchor.byte
            })
            .map(|(index, _)| index)
            .next_back()
            .unwrap_or(start)
    }

    fn link_count(&self) -> usize {
        self.sections.iter().map(|s| s.links.len()).sum()
    }

    #[must_use]
    pub fn links(&self) -> Vec<&RecordLink> {
        self.sections.iter().flat_map(|s| &s.links).collect()
    }

    #[must_use]
    pub fn is_expanded(&self, model: &Model, section: &RecordSection) -> bool {
        !section.secondary
            || model
                .inspection_state
                .borrow()
                .expanded
                .contains(&section.key)
    }

    /// Copy the original prose/output, never the wrapped or shortened screen text.
    #[must_use]
    pub fn original(&self) -> String {
        let sections = self
            .sections
            .iter()
            .map(|s| {
                let mut text = format!("## {}\n\n{}", s.heading, s.text.source());
                for link in &s.links {
                    if s.text.source().is_empty() {
                        text.push_str(&format!("\n[{}]({})", link.label, link.reference));
                    }
                }
                text
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!("# {}\n\n{sections}", self.title)
    }
}

// Parsing and geometry caches are derived; model equality compares reading state.
impl PartialEq for InspectionState {
    fn eq(&self, other: &Self) -> bool {
        self.expanded == other.expanded
            && self.anchor == other.anchor
            && self.focused_link == other.focused_link
            && self.native_positions == other.native_positions
    }
}

#[cfg(test)]
#[path = "inspection.tests.rs"]
mod tests;
