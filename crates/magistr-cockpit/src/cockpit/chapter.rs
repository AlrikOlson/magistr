//! The chapter of one shipped chunk — the report's document.

use super::roadmap::Roadmap;
use super::roadmap_chunk::RoadmapChunk;
use super::shelf::Shelf;

/// The written report of a shipped chunk: the title the display face sets, the prose, when
/// it shipped and under which workstream, and what it unlocked. The
/// *same* document three ways — the Report view composes it, the share
/// page publishes it and the export bundle carries it — so the three
/// cannot say different things.
///
/// Derived from the roadmap the cockpit already holds, never stored:
/// [`Chapter::of`] reads the chunk's record and the chunks that stand
/// on it. Data only; the page and the bundle are written by
/// [`shelf::write_page`](crate::shelf::write_page) and
/// [`shelf::write_bundle`](crate::shelf::write_bundle), effects the
/// entry point's job runs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Chapter {
    /// The shipped chunk's id.
    pub chunk: String,
    /// The chapter's title — the chunk's claim, or its name while it
    /// has none.
    pub title: String,
    /// The chapter's prose — the chunk's description as the store
    /// holds it. Empty when nothing was written, and shown as such.
    pub prose: String,
    /// The workstream, when grouped.
    pub group: Option<String>,
    /// When it shipped, in seconds since the Unix epoch, as the store's
    /// `updated_at` stamp carries it.
    pub shipped_secs: Option<i64>,
    /// The chunks that stand on this one and are still to come, each
    /// with the state word its star wears (`Ready`, `Pending`, …), in
    /// store order.
    pub unlocked: Vec<(String, String)>,
}

impl Chapter {
    /// The bundle's chapter document — what the bundle writes
    /// [`Self::markdown`] to, beside the receipts.
    pub const FILE: &'static str = "chapter.md";
    /// The share page — what [`Self::html`] is written to.
    pub const PAGE: &'static str = "chapter.html";

    /// The words for a chunk whose record carries no prose.
    pub const NO_PROSE: &'static str = "No description was written.";
    /// The words for a chunk nothing stands on.
    pub const NOTHING_UNLOCKED: &'static str = "nothing waits on it";

    /// The chapter of `chunk`, with what it unlocked read off
    /// `roadmap`: every active chunk not yet done whose deps name it,
    /// wearing the state its star wears.
    #[must_use]
    pub fn of(chunk: &RoadmapChunk, roadmap: &Roadmap) -> Self {
        let title = if chunk.title.trim().is_empty() {
            chunk.name.clone()
        } else {
            chunk.title.clone()
        };
        let unlocked = roadmap
            .unlocked_by(&chunk.id)
            .into_iter()
            .map(|dependent| {
                let state = roadmap
                    .star_state(&dependent.id)
                    .map_or("Pending", |state| state.label());
                (dependent.id.clone(), state.to_owned())
            })
            .collect();
        Self {
            chunk: chunk.id.clone(),
            title,
            prose: chunk.description.trim().to_owned(),
            group: chunk.group.clone(),
            shipped_secs: chunk.updated_secs,
            unlocked,
        }
    }

    /// The ship date as the cap spells it — `Aug 30` — when the record
    /// has a stamp.
    #[must_use]
    pub fn shipped_day(&self) -> Option<String> {
        self.shipped_secs
            .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
            .map(|stamp| stamp.format("%b %-d").to_string())
    }

    /// The chapter's cap — `Report · Shipped Aug 30 · desktop-gui`:
    /// the word, the ship day when there is a stamp, the workstream
    /// when there is one.
    #[must_use]
    pub fn cap(&self) -> String {
        let mut parts = vec!["Report".to_owned()];
        parts.push(match self.shipped_day() {
            Some(day) => format!("Shipped {day}"),
            None => "Shipped".to_owned(),
        });
        if let Some(group) = &self.group {
            parts.push(group.clone());
        }
        parts.join(" \u{b7} ")
    }

    /// The prose, or the words for none.
    #[must_use]
    pub fn prose_text(&self) -> &str {
        if self.prose.is_empty() {
            Self::NO_PROSE
        } else {
            &self.prose
        }
    }

    /// The unlocked list as one line — ` · Ready` — or the
    /// words for none.
    #[must_use]
    pub fn unlocked_text(&self) -> String {
        if self.unlocked.is_empty() {
            return Self::NOTHING_UNLOCKED.to_owned();
        }
        self.unlocked
            .iter()
            .map(|(id, state)| format!("{id} \u{b7} {state}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The document as markdown: the title, the cap, the prose, the
    /// unlocked list, then the shelf's receipts — STE, one thing per
    /// line.
    #[must_use]
    pub fn markdown(&self, shelf: &Shelf) -> String {
        let mut out = format!(
            "# {}\n\n{}\n\n{}\n\n## Unlocked\n\n",
            self.title,
            self.cap(),
            self.prose_text()
        );
        if self.unlocked.is_empty() {
            out.push_str(&format!("- {}\n", Self::NOTHING_UNLOCKED));
        }
        for (id, state) in &self.unlocked {
            out.push_str(&format!("- {id} \u{b7} {state}\n"));
        }
        out.push_str("\n## Receipts\n\n");
        out.push_str(&shelf.receipt_lines().join("\n"));
        out.push('\n');
        out
    }

    /// The document as one self-contained HTML page — the share page:
    /// the same words as [`Self::markdown`], the title in a serif and
    /// the receipts in a mono, no script, no remote asset.
    #[must_use]
    pub fn html(&self, shelf: &Shelf) -> String {
        let mut unlocked = String::new();
        if self.unlocked.is_empty() {
            unlocked.push_str(&format!("<li>{}</li>", escape(Self::NOTHING_UNLOCKED)));
        }
        for (id, state) in &self.unlocked {
            unlocked.push_str(&format!(
                "<li><code>{}</code> \u{b7} {}</li>",
                escape(id),
                escape(state)
            ));
        }
        let receipts: String = shelf
            .receipt_lines()
            .iter()
            .map(|line| format!("<li>{}</li>", escape(line.trim_start_matches("- "))))
            .collect();
        format!(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
             <title>{title}</title>\n\
             <style>\n\
             body{{margin:0;padding:48px 24px;background:#1e1e2e;color:#cdd6f4;\
             font-family:'Iowan Old Style',Palatino,Charter,Georgia,serif;line-height:1.7}}\n\
             main{{max-width:520px;margin:0 auto}}\n\
             .cap{{font-family:ui-monospace,Menlo,monospace;font-size:9.5px;letter-spacing:.12em;\
             text-transform:uppercase;color:#6c7086;margin:0 0 8px}}\n\
             h1{{font-size:24px;font-weight:400;letter-spacing:-.01em;margin:0 0 12px}}\n\
             p{{font-size:14px;color:#bac2de;margin:0 0 20px}}\n\
             ul{{list-style:none;padding:0;margin:0 0 20px;font-family:ui-monospace,Menlo,monospace;\
             font-size:11px;color:#a6adc8}}\n\
             li{{padding:6px 0;border-bottom:1px solid rgba(69,71,90,.6)}}\n\
             code{{font-family:inherit}}\n\
             </style>\n</head>\n<body>\n<main>\n\
             <p class=\"cap\">{cap}</p>\n<h1>{title}</h1>\n<p>{prose}</p>\n\
             <p class=\"cap\">Unlocked</p>\n<ul>{unlocked}</ul>\n\
             <p class=\"cap\">Receipts</p>\n<ul>{receipts}</ul>\n\
             </main>\n</body>\n</html>\n",
            title = escape(&self.title),
            cap = escape(&self.cap()),
            prose = escape(self.prose_text()),
        )
    }
}

/// `text` with HTML's five reserved characters escaped.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[path = "chapter.tests.rs"]
mod tests;
