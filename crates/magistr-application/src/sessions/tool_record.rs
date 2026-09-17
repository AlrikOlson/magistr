//! What a tool call carried besides its title, status and diff.

use alloc::{string::String, vec::Vec};

use super::tool_payload::ToolPayload;

use super::tool_verb::ToolVerb;

/// Everything an ACP tool call said about itself past the row it makes
/// — the body a card opens into: the kind the wire named, the raw input
/// the agent sent, the raw output the tool returned, its text content
/// blocks, the files it touched and the terminal it embeds.
///
/// Bounded: a record is data on the turn for as long as the turn lives,
/// so one past [`ToolRecord::CAP`] bytes is cut down to it — input
/// first, then output, then the text blocks, each taking what the
/// earlier ones left — and remembers how big it was, so a detail can say
/// so rather than show a silently short body.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolRecord {
    /// The kind the wire named, when it named one — an update may
    /// re-kind a call it announced as something else.
    pub kind: Option<ToolVerb>,
    /// The raw input the agent sent the tool.
    pub input: Option<ToolPayload>,
    /// The raw output the tool returned.
    pub output: Option<ToolPayload>,
    /// The text content blocks, in wire order.
    pub text: Vec<String>,
    /// The files the call touched, each with the line when the wire
    /// gave one.
    pub locations: Vec<(String, Option<u32>)>,
    /// The terminal the call embeds, by id.
    pub terminal: Option<String>,
    /// How many bytes the record measured before it was cut to the
    /// cap, when it was.
    pub truncated_from: Option<usize>,
}

impl ToolRecord {
    /// The most one call keeps, in bytes of input, output and text.
    pub const CAP: usize = 64 * 1024;

    /// Whether the record carries anything at all — a call that said
    /// nothing past its title leaves the line's record untouched.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kind.is_none()
            && self.input.is_none()
            && self.output.is_none()
            && self.text.is_empty()
            && self.locations.is_empty()
            && self.terminal.is_none()
    }

    /// The bytes the record's input, output and text take.
    #[must_use]
    pub fn size(&self) -> usize {
        self.input.as_ref().map_or(0, value_len)
            + self.output.as_ref().map_or(0, value_len)
            + self.text.iter().map(String::len).sum::<usize>()
    }

    /// The record cut to [`ToolRecord::CAP`], its size before the cut
    /// noted; a record inside the cap comes back as it was.
    #[must_use]
    pub fn bounded(mut self) -> Self {
        let size = self.size();
        if size <= Self::CAP {
            return self;
        }
        self.truncated_from = Some(self.truncated_from.map_or(size, |prior| prior.max(size)));
        let mut budget = Self::CAP;
        self.input = self.input.take().map(|value| cut_value(value, &mut budget));
        self.output = self
            .output
            .take()
            .map(|value| cut_value(value, &mut budget));
        for text in &mut self.text {
            cut_text(text, &mut budget);
        }
        self
    }

    /// Fold an update's record into this one the way the wire means
    /// it: a kind, input, output or terminal the update carries
    /// replaces the one held, a content or locations collection it
    /// carries replaces the collection, and what it left out stays.
    /// The result is bounded again.
    pub fn absorb(&mut self, update: Self) {
        if update.kind.is_some() {
            self.kind = update.kind;
        }
        if update.input.is_some() {
            self.input = update.input;
        }
        if update.output.is_some() {
            self.output = update.output;
        }
        if !update.text.is_empty() {
            self.text = update.text;
        }
        if !update.locations.is_empty() {
            self.locations = update.locations;
        }
        if update.terminal.is_some() {
            self.terminal = update.terminal;
        }
        self.truncated_from = match (self.truncated_from, update.truncated_from) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        *self = core::mem::take(self).bounded();
    }
}

/// The bytes a JSON value takes: a string's own, anything else its
/// serialized form.
fn value_len(value: &ToolPayload) -> usize {
    value.content().len()
}

/// `value` cut to `budget`: a string keeps its head, anything else
/// becomes the head of its serialized form — still readable, no longer
/// parseable, which the noted size says.
fn cut_value(value: ToolPayload, budget: &mut usize) -> ToolPayload {
    if value.content().len() <= *budget {
        *budget -= value.content().len();
        return value;
    }
    let mut text = match value {
        ToolPayload::Text(text) | ToolPayload::Json(text) => text,
    };
    cut_text(&mut text, budget);
    ToolPayload::Text(text)
}

/// `text` cut to `budget` on a char boundary, the budget reduced by
/// what it keeps.
fn cut_text(text: &mut String, budget: &mut usize) {
    if text.len() <= *budget {
        *budget -= text.len();
        return;
    }
    let mut end = *budget;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    *budget = 0;
}

#[cfg(test)]
#[path = "tool_record.tests.rs"]
mod tests;
