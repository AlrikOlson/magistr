use magistr_cockpit::cockpit::Term;
use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::match_line::MatchLine;
use crate::progress_strip::ProgressStrip;
use crate::term_pane::TermPane;
use crate::voice::Voice;

/// The body a feed card opens into: one anatomy — head · body · receipt line — and only
/// the body knows the verb. Run → the command and its output pane; Read → path · range
/// and the excerpt; Search → the query and its hits with the query marked; Fetch → the
/// url and what came back; Think → the thought; an MCP tool → server · tool with its
/// input and output; Check → the gate's command and its own diagnostic rows (error ·
/// file:line:column · help), never a summary line.
///
/// Every string is set in the data voice on the card's own ground; the caps over a
/// block are the receipt key's tracked uppercase, the way a receipt row names what it
/// holds. The card's head and its receipt line are the feed's ([`ToolDetail::receipt`]
/// draws the latter); this is the middle.
///
/// A call that runs on a pseudo-terminal carries its screen — the [`Term`] with the
/// model's tick and whether motion is reduced — and its output block is the
/// [`TermPane`] instead of the text pane: the program's own cells, the cursor, the
/// pill.
pub enum ToolDetail<'a> {
    /// A command and what it printed.
    Run {
        /// The command line the agent ran.
        command: &'a str,
        /// Its output, newest line last — shown when the call embeds
        /// no terminal.
        output: &'a str,
        /// The terminal the command streams to, with the tick the
        /// frame is drawn at and whether motion is reduced.
        term: Option<(&'a Term, u64, bool)>,
    },
    /// A file read.
    Read {
        /// The file.
        path: &'a str,
        /// The first line read and how many, when the call named them.
        range: Option<(u32, Option<u32>)>,
        /// What the read returned.
        excerpt: &'a str,
    },
    /// A search and its hits.
    Search {
        /// The query — marked wherever it appears in a hit.
        query: &'a str,
        /// The hit rows, one per line.
        hits: &'a str,
    },
    /// An external fetch.
    Fetch {
        /// The url.
        url: &'a str,
        /// What came back.
        body: &'a str,
    },
    /// The agent's own reasoning.
    Think {
        /// The thought.
        thought: &'a str,
    },
    /// Anything else — an MCP tool by its server and name.
    Mcp {
        /// The server, when the tool's name carried one.
        server: Option<&'a str>,
        /// The tool.
        tool: &'a str,
        /// The raw input the agent sent, spelled out.
        input: &'a str,
        /// The raw output the tool returned, spelled out.
        output: &'a str,
    },
    /// A quality gate and its diagnostics.
    Check {
        /// The gate's command, when the recorder kept it.
        command: Option<&'a str>,
        /// The gate's own rows — error · location · help — one per line.
        rows: &'a str,
        /// The terminal the gate streamed on, when it streamed — the
        /// screen stands in for the rows while it does.
        term: Option<(&'a Term, u64, bool)>,
    },
}

impl ToolDetail<'_> {
    /// The inset every row sits at — the card head's `padding:6px 12px`.
    const PAD_X: f32 = 12.0;
    /// The air over and under the body.
    const PAD_Y: f32 = 6.0;
    /// The air between a cap and its block, and between blocks.
    const GAP: f32 = 4.0;
    /// The most lines an output pane shows. A longer output keeps its
    /// tail — the lines a run ends on are the lines that say how it
    /// went — and counts what came before it on one line above.
    pub const PANE_LINES: usize = 12;

    /// The body on `ground` (the card's own fill), every row inset.
    #[must_use]
    pub fn el(&self, ground: Srgba, lex: &Lexicon) -> Element {
        let mut body = Element::surface()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .gap(Self::GAP)
            .align_start()
            .stretch();
        match self {
            Self::Run {
                command,
                output,
                term,
            } => {
                // The reader's kill, where the program's own progress report was.
                if term.is_some_and(|(term, ..)| term.is_killed()) {
                    body = body.child(killed(lex));
                }
                // The program's own progress report, under the head.
                if let Some(strip) =
                    term.and_then(|(term, ..)| ProgressStrip::el(term.progress(), lex))
                {
                    body = body.child(strip);
                }
                body = body
                    .child(cap("command", lex))
                    .child(line(command, lex.tx))
                    .child(cap("output", lex))
                    .child(match term {
                        Some((term, ticks, reduced)) => {
                            TermPane::el(term, *ticks, *reduced, ground, lex)
                        }
                        None => pane(output, ground, lex),
                    });
            }
            Self::Read {
                path,
                range,
                excerpt,
            } => {
                let subject = match range {
                    Some((from, Some(count))) => format!(
                        "{path} \u{b7} lines {from}\u{2013}{}",
                        from.saturating_add(count.saturating_sub(1))
                    ),
                    Some((from, None)) => format!("{path} \u{b7} from line {from}"),
                    None => (*path).to_owned(),
                };
                body = body
                    .child(cap("path", lex))
                    .child(line(&subject, lex.tx))
                    .child(cap("excerpt", lex))
                    .child(pane(excerpt, ground, lex));
            }
            Self::Search { query, hits } => {
                body = body
                    .child(cap("query", lex))
                    .child(line(query, lex.tx))
                    .child(cap("hits", lex));
                let mut rows = Element::surface().align_start().stretch();
                for hit in lines(hits) {
                    rows = rows.child(MatchLine::el(
                        hit,
                        &marks(hit, query),
                        Voice::VALUE,
                        lex.sb1,
                        lex,
                    ));
                }
                body = body.child(rows);
            }
            Self::Fetch { url, body: came } => {
                body = body
                    .child(cap("url", lex))
                    .child(line(url, lex.blue))
                    .child(cap("response", lex))
                    .child(pane(came, ground, lex));
            }
            Self::Think { thought } => {
                body = body.child(cap("thought", lex));
                for row in lines(thought) {
                    body = body
                        .child(Element::text(Voice::VALUE, lex.sb1, row).claim(Component::Value));
                }
            }
            Self::Mcp {
                server,
                tool,
                input,
                output,
            } => {
                let name = match server {
                    Some(server) => format!("{server} \u{b7} {tool}"),
                    None => (*tool).to_owned(),
                };
                body = body
                    .child(cap("tool", lex))
                    .child(line(&name, lex.tx))
                    .child(cap("input", lex))
                    .child(pane(input, ground, lex))
                    .child(cap("output", lex))
                    .child(pane(output, ground, lex));
            }
            Self::Check {
                command,
                rows,
                term,
            } => {
                if term.is_some_and(|(term, ..)| term.is_killed()) {
                    body = body.child(killed(lex));
                }
                if let Some(strip) =
                    term.and_then(|(term, ..)| ProgressStrip::el(term.progress(), lex))
                {
                    body = body.child(strip);
                }
                if let Some(command) = command {
                    body = body.child(cap("command", lex)).child(line(command, lex.tx));
                }
                if let Some((term, ticks, reduced)) = term {
                    body = body
                        .child(cap("output", lex))
                        .child(TermPane::el(term, *ticks, *reduced, ground, lex));
                } else {
                    body = body.child(cap("diagnostics", lex));
                    let mut diagnostics = Element::surface().align_start().stretch();
                    for row in lines(rows) {
                        diagnostics = diagnostics.child(line(row, diagnostic_ink(row, lex)));
                    }
                    body = body.child(diagnostics);
                }
            }
        }
        body
    }

    /// The receipt line under a body: its parts joined by the middle
    /// dot — `exit 0 · 1.2 s · receipt #238` — and, when the record
    /// was cut to its cap, how much was recorded, so a short body is
    /// never silently short. Over its own hairline on `ground`.
    #[must_use]
    pub fn receipt(
        parts: &[String],
        truncated_from: Option<usize>,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let mut words: Vec<String> = parts.iter().filter(|p| !p.is_empty()).cloned().collect();
        if let Some(from) = truncated_from {
            words.push(format!(
                "cut at {} \u{b7} {} recorded",
                kib(magistr_cockpit::agent::ToolRecord::CAP),
                kib(from)
            ));
        }
        if words.is_empty() {
            // Nothing to say: no line, no hairline — a card with no
            // receipt ends on its body.
            return Element::surface().height(0.0);
        }
        Element::surface()
            .stretch()
            .child(
                Element::surface()
                    .height(1.0)
                    .fill(Lexicon::composite(ground, lex.hairc)),
            )
            .child(
                Element::row().pad_x(Self::PAD_X).pad_y(Self::PAD_Y).child(
                    Element::text(Voice::LEDGER_META, lex.ov0, words.join(" \u{b7} "))
                        .claim(Component::ReceiptRow)
                        .nowrap(),
                ),
            )
    }
}

/// A block's cap — the receipt key's tracked uppercase.
fn cap(text: &str, lex: &Lexicon) -> Element {
    Element::text(Voice::RECEIPT_KEY, lex.ov0, text)
        .claim(Component::Cap)
        .nowrap()
}

/// The strip's word for a program the reader ended: `killed by you` in the failure hue
/// and the cap voice, where the progress strip stood.
fn killed(lex: &Lexicon) -> Element {
    Element::text(Voice::RECEIPT_KEY, lex.red, "killed by you")
        .claim(Component::Cap)
        .nowrap()
}

/// One row in the data voice.
fn line(text: &str, ink: Srgba) -> Element {
    Element::text(Voice::VALUE, ink, text)
        .claim(Component::Value)
        .nowrap()
}

/// The lines of `text`, trailing blank ones dropped — an output that
/// ends on a newline is not an output with an empty last line.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.trim_end_matches(['\n', '\r']).lines()
}

/// A bounded output pane: the last [`ToolDetail::PANE_LINES`] lines in
/// a well, what came before counted on one line above them. An empty
/// output says so rather than drawing an empty well.
fn pane(output: &str, ground: Srgba, lex: &Lexicon) -> Element {
    let all: Vec<&str> = lines(output).collect();
    let well = Srgba::new(lex.base.r, lex.base.g, lex.base.b, TermPane::WELL);
    let mut pane = Element::surface()
        .stretch()
        .align_start()
        .pad(TermPane::PAD)
        .radius(TermPane::RADIUS)
        .fill(Lexicon::composite(ground, well));
    if all.is_empty() {
        return pane.child(
            Element::text(Voice::LEDGER_META, lex.ov0, "no output")
                .claim(Component::Value)
                .nowrap(),
        );
    }
    let earlier = all.len().saturating_sub(ToolDetail::PANE_LINES);
    if earlier > 0 {
        pane = pane.child(
            Element::text(
                Voice::LEDGER_META,
                lex.ov0,
                format!("\u{2026} {earlier} earlier lines"),
            )
            .claim(Component::Counter)
            .nowrap(),
        );
    }
    for row in &all[earlier..] {
        pane = pane.child(line(row, lex.sb1));
    }
    pane
}

/// The byte ranges of `query` in `hit`, matched without regard to
/// ASCII case — ASCII lowering keeps every byte offset, which is what
/// lets the ranges be cut from the original.
fn marks(hit: &str, query: &str) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    let hay = hit.to_ascii_lowercase();
    let needle = query.to_ascii_lowercase();
    hay.match_indices(&needle)
        .map(|(at, found)| (at, at + found.len()))
        .collect()
}

/// A diagnostic row's ink by what it is: an error red, a warning
/// yellow, help and notes blue, a location (`--> src/x.rs:3:9`) the
/// subdued ink, anything else the text ink.
fn diagnostic_ink(row: &str, lex: &Lexicon) -> Srgba {
    let word = row.trim_start();
    if word.starts_with("error") {
        lex.red
    } else if word.starts_with("warning") {
        lex.yellow
    } else if word.starts_with("help") || word.starts_with("note") {
        lex.blue
    } else if word.starts_with("-->") || word.starts_with("::: ") || is_location(word) {
        lex.sb0
    } else {
        lex.tx
    }
}

/// Whether `word` reads as `path:line:column` — two colon-separated
/// numbers at its end.
fn is_location(word: &str) -> bool {
    let mut parts = word.rsplitn(3, ':');
    let column = parts
        .next()
        .is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    let line = parts
        .next()
        .is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    column && line && parts.next().is_some_and(|p| !p.is_empty())
}

/// `bytes` in KiB, whole.
fn kib(bytes: usize) -> String {
    format!("{} KiB", bytes.div_ceil(1024))
}

#[cfg(test)]
#[path = "tool_detail.tests.rs"]
mod tests;
