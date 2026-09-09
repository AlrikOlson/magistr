//! The deliverables shelf of one shipped chunk.

use std::path::PathBuf;

use super::copy::Copy;
use super::shelf_still::ShelfStill;
use super::still_kind::StillKind;

/// Artifacts and receipts recorded for a completed chunk.
///
/// The entry point gathers visuals, commit details, gate results, cost,
/// and workflow references off the loop thread, then sends one
/// [`JobOutput::Shelf`](crate::job::JobOutput::Shelf). The node gallery and
/// export bundle read this same record. Missing receipts remain absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shelf {
    /// The shipped chunk's id.
    pub chunk: String,
    /// When it shipped, in seconds since the Unix epoch, as the store's
    /// `updated_at` stamp carries it.
    pub shipped_secs: Option<i64>,
    /// The commit that shipped it: the short hash and the subject line.
    pub commit: Option<(String, String)>,
    /// The gates the store still holds for it, in recorded order: the
    /// gate's name and whether it passed. Empty when the execution
    /// trace has moved on to a later objective — the store keeps one
    /// objective's tasks, and an older chunk's gates are not invented.
    pub gates: Vec<(String, bool)>,
    /// What the sessions that shipped it cost, in USD cents; `None`
    /// when the session database has no session covering the ship.
    pub cost_cents: Option<u64>,
    /// The tokens those sessions spent; `None` with the cost.
    pub tokens: Option<u64>,
    /// How long the session recording runs, in seconds — the fourth
    /// deliverable kind; `None`
    /// while no recording was kept, and shown as `not recorded`.
    pub recording_secs: Option<u64>,
    /// The reasoning steps the chunk names (`think:N`), ascending.
    pub think_refs: Vec<u32>,
    /// The ship tasks the chunk names (`task:X`), in the chunk's order.
    pub task_refs: Vec<String>,
    /// The pictures, stills before filmstrips, each in its suite's order.
    pub stills: Vec<ShelfStill>,
    /// Where the bundle was last written, once \[Export\] ran.
    pub bundle: Option<PathBuf>,
    /// Where the share page was last written, once \[Share page\] ran.
    pub page: Option<PathBuf>,
}

impl Shelf {
    /// The bundle's receipts document — what
    /// [`shelf::write_bundle`](crate::shelf::write_bundle) writes
    /// [`Self::receipts_markdown`] to.
    pub const RECEIPTS_FILE: &'static str = "receipts.md";

    /// An empty shelf for `chunk`, its refs read off the chunk's own
    /// cross-refs — the `think:N` and `task:X` edges `roadmap_link`
    /// wrote. Anything else on the list is another family's business.
    #[must_use]
    pub fn new(chunk: impl Into<String>, cross_refs: &[String]) -> Self {
        let mut think_refs: Vec<u32> = cross_refs
            .iter()
            .filter_map(|r| r.strip_prefix("think:")?.parse().ok())
            .collect();
        think_refs.sort_unstable();
        think_refs.dedup();
        let task_refs = cross_refs
            .iter()
            .filter_map(|r| r.strip_prefix("task:").map(str::to_owned))
            .collect();
        Self {
            chunk: chunk.into(),
            think_refs,
            task_refs,
            ..Self::default()
        }
    }

    /// How many pictures of `kind` the shelf holds.
    #[must_use]
    pub fn count(&self, kind: StillKind) -> usize {
        self.stills.iter().filter(|s| s.kind == kind).count()
    }

    /// The commit as one receipt value — `a3f9c21 · subject` — or the
    /// words for none.
    #[must_use]
    pub fn commit_text(&self) -> String {
        self.commit.as_ref().map_or_else(
            || "none found".to_owned(),
            |(hash, subject)| format!("{hash} \u{b7} {subject}"),
        )
    }

    /// The cost as the copy law spells money — `2.07 USD` — or the words
    /// for none.
    #[must_use]
    pub fn cost_text(&self) -> String {
        self.cost_cents
            .map_or_else(|| "not recorded".to_owned(), Copy::usd)
    }

    /// The tokens as the copy law spells a count — `41k tokens`,
    /// `412 tokens` — when the sessions were found.
    #[must_use]
    pub fn tokens_text(&self) -> Option<String> {
        let tokens = self.tokens?;
        Some(if tokens < 1_000 {
            format!("{tokens} tokens")
        } else if tokens < 10_000 {
            let tenths = (tokens + 50) / 100;
            if tenths % 10 == 0 {
                format!("{}k tokens", tenths / 10)
            } else {
                format!("{}.{}k tokens", tenths / 10, tenths % 10)
            }
        } else {
            format!("{}k tokens", (tokens + 500) / 1_000)
        })
    }

    /// The cost and the tokens as one value — `2.07 USD · 41k tokens`
    /// — the receipt the Report's cost row wears.
    #[must_use]
    pub fn spend_text(&self) -> String {
        match self.tokens_text() {
            Some(tokens) => format!("{} \u{b7} {tokens}", self.cost_text()),
            None => self.cost_text(),
        }
    }

    /// The gates as a count — `3 passed`, `2 of 3 passed` — or the
    /// words for none kept.
    #[must_use]
    pub fn gates_text(&self) -> String {
        if self.gates.is_empty() {
            return "none kept".to_owned();
        }
        let passed = self.gates.iter().filter(|(_, ok)| *ok).count();
        if passed == self.gates.len() {
            format!("{passed} passed")
        } else {
            format!("{passed} of {} passed", self.gates.len())
        }
    }

    /// The recording's length — `12 min`, `45 s` — or the words for
    /// none kept.
    #[must_use]
    pub fn recording_text(&self) -> String {
        match self.recording_secs {
            None => "not recorded".to_owned(),
            Some(secs) if secs < 60 => format!("{secs} s"),
            Some(secs) => format!("{} min", secs / 60),
        }
    }

    /// The reasoning refs as one value — `think 490 \u{b7} 491` — or
    /// the words for none.
    #[must_use]
    pub fn trace_text(&self) -> String {
        if self.think_refs.is_empty() {
            return "none linked".to_owned();
        }
        let numbers: Vec<String> = self.think_refs.iter().map(u32::to_string).collect();
        format!("think {}", numbers.join(" \u{b7} "))
    }

    /// The chunk's reasoning references, separated by middle dots, or the
    /// empty-state label when none were recorded.
    #[must_use]
    pub fn trace_refs_text(&self) -> String {
        if self.think_refs.is_empty() {
            return "none linked".to_owned();
        }
        self.think_refs
            .iter()
            .map(|n| format!("think:{n}"))
            .collect::<Vec<_>>()
            .join(" \u{b7} ")
    }

    /// The ship refs as one value — `task:verify` — or the words for
    /// none.
    #[must_use]
    pub fn ship_text(&self) -> String {
        if self.task_refs.is_empty() {
            return "none linked".to_owned();
        }
        self.task_refs
            .iter()
            .map(|task| format!("task:{task}"))
            .collect::<Vec<_>>()
            .join(" \u{b7} ")
    }

    /// When the chunk shipped, as `2026-09-02 09:25 UTC`, or the words
    /// for a record with no stamp.
    #[must_use]
    pub fn shipped_text(&self) -> String {
        self.shipped_secs
            .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
            .map_or_else(
                || "no stamp".to_owned(),
                |stamp| stamp.format("%Y-%m-%d %H:%M UTC").to_string(),
            )
    }

    /// The receipts, one markdown bullet each — what the receipts
    /// document and the chapter's receipts section both list.
    #[must_use]
    pub fn receipt_lines(&self) -> Vec<String> {
        let gates = if self.gates.is_empty() {
            "none kept".to_owned()
        } else {
            self.gates
                .iter()
                .map(|(name, passed)| {
                    format!("{name} {}", if *passed { "passed" } else { "failed" })
                })
                .collect::<Vec<_>>()
                .join(" \u{b7} ")
        };
        vec![
            format!("- Shipped: {}", self.shipped_text()),
            format!("- Commit: {}", self.commit_text()),
            format!("- Gates: {gates}"),
            format!("- Cost: {}", self.spend_text()),
            format!("- Recording: {}", self.recording_text()),
            format!("- Trace: {}", self.trace_text()),
            format!("- Ship: {}", self.ship_text()),
        ]
    }

    /// The receipts as the bundle writes them: one markdown document,
    /// STE, every receipt on its own line.
    #[must_use]
    pub fn receipts_markdown(&self) -> String {
        let mut out = format!("# {}\n\n{}\n", self.chunk, self.receipt_lines().join("\n"));
        for kind in [StillKind::Still, StillKind::Filmstrip] {
            let names: Vec<&str> = self
                .stills
                .iter()
                .filter(|s| s.kind == kind)
                .map(|s| s.name.as_str())
                .collect();
            if names.is_empty() {
                continue;
            }
            out.push_str(&format!("\n## {}\n\n", kind.dir_name()));
            for name in names {
                out.push_str(&format!("- {}/{name}\n", kind.dir_name()));
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "shelf.tests.rs"]
mod tests;
