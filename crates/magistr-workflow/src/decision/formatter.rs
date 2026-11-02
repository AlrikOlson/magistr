//! Console / Markdown / JSON formatters for reasoning notes.

use owo_colors::{OwoColorize, Style};

use crate::decision::domain::{Log, NextAction, Note, Thread};

#[derive(Debug, Clone, Copy)]
pub struct Formatter {
    color_enabled: bool,
}

impl Formatter {
    pub fn new(color_enabled: bool) -> Self {
        Self { color_enabled }
    }

    fn paint(&self, text: &str, style: Style) -> String {
        if self.color_enabled {
            text.style(style).to_string()
        } else {
            text.to_string()
        }
    }

    fn purpose_style(purpose: &str) -> Style {
        match purpose.to_ascii_lowercase().as_str() {
            "analysis" => Style::new().blue(),
            "action" => Style::new().green(),
            "reflection" => Style::new().yellow(),
            "decision" => Style::new().magenta(),
            "summary" => Style::new().cyan(),
            "validation" => Style::new().bright_green(),
            "exploration" => Style::new().bright_yellow(),
            "hypothesis" => Style::new().bright_blue(),
            "correction" => Style::new().bright_red(),
            "planning" => Style::new().bright_cyan(),
            _ => Style::new().white(),
        }
    }

    fn format_confidence(&self, confidence: Option<f64>) -> String {
        let Some(c) = confidence else {
            return String::new();
        };
        let percentage = (c * 100.0).round() as i32;
        let (symbol, style) = if c < 0.3 {
            ('○', Style::new().red())
        } else if c < 0.7 {
            ('◐', Style::new().yellow())
        } else {
            ('●', Style::new().green())
        };
        if self.color_enabled {
            format!(" {}", format!(" {symbol} {percentage}%").style(style))
        } else {
            format!(" [{percentage}%]")
        }
    }

    fn format_next_action(action: &NextAction) -> String {
        match action {
            NextAction::Text(s) => s.clone(),
            NextAction::Structured(a) => {
                let mut out = a.action.clone();
                if let Some(tool) = &a.tool {
                    out = format!("[{tool}] {out}");
                }
                if let Some(params) = &a.parameters {
                    if !params.is_empty() {
                        if let Ok(json) = serde_json::to_string(params) {
                            out.push_str(&format!(" ({json})"));
                        }
                    }
                }
                out
            }
        }
    }

    pub fn format_note_console(&self, note: &Note) -> String {
        let purpose_style = Self::purpose_style(&note.purpose);
        let purpose_text = note.purpose.to_ascii_uppercase();
        let mut header = self.paint(
            &format!(
                "[Note {}/{}] {}",
                note.note, note.estimated_total, purpose_text
            ),
            purpose_style,
        );
        header.push_str(&self.format_confidence(note.confidence));

        if let Some(revises) = note.revises_note {
            header.push_str(&self.paint(&format!(" ↻ Revises #{revises}"), Style::new().yellow()));
        }
        if let Some(from) = note.thread_from {
            header
                .push_str(&self.paint(&format!(" ⟿ Thread from #{from}"), Style::new().magenta()));
            if let Some(name) = &note.thread_name {
                header.push_str(&self.paint(&format!(" ({name})"), Style::new().bright_black()));
            }
        }

        let mut lines = vec![header];

        let gray = Style::new().bright_black();
        let white = Style::new().white();
        let yellow = Style::new().yellow();

        if !note.context.is_empty() {
            lines.push(format!("{} {}", self.paint("Context:", gray), note.context));
        }

        lines.push(format!(
            "{} {}",
            self.paint("Thought:", white),
            note.thought
        ));

        if let Some(notes) = &note.uncertainty_notes {
            lines.push(format!("{} {}", self.paint("Uncertainty:", yellow), notes));
        }
        if let Some(reason) = &note.revision_reason {
            lines.push(format!(
                "{} {}",
                self.paint("Revision Reason:", yellow),
                reason
            ));
        }

        lines.push(format!("{} {}", self.paint("Outcome:", gray), note.outcome));

        let next_action = Self::format_next_action(&note.next_action);
        lines.push(format!(
            "{} {} - {}",
            self.paint("Next:", gray),
            next_action,
            note.rationale
        ));

        if let Some(tools) = &note.tools_used {
            if !tools.is_empty() {
                lines.push(format!(
                    "{} {}",
                    self.paint("Tools Used:", gray),
                    tools.join(", ")
                ));
            }
        }

        if let Some(deps) = &note.dependencies {
            if !deps.is_empty() {
                let joined = deps
                    .iter()
                    .map(|e| match e.relation() {
                        Some(rel) => format!("{}({})", e.note(), rel),
                        None => e.note().to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!(
                    "{} Notes {}",
                    self.paint("Depends On:", gray),
                    joined
                ));
            }
        }

        if let Some(exec_ref) = &note.run_ref {
            lines.push(format!(
                "{} {}",
                self.paint("Execution Ref:", gray),
                exec_ref
            ));
        }

        lines.push(self.paint(&"─".repeat(60), gray));
        lines.join("\n")
    }

    pub fn format_note_markdown(&self, note: &Note) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!(
            "### Note {}/{}: {}",
            note.note,
            note.estimated_total,
            note.purpose.to_ascii_uppercase()
        ));

        let mut badges: Vec<String> = Vec::new();
        if let Some(c) = note.confidence {
            let pct = (c * 100.0).round() as i32;
            badges.push(format!(
                "![Confidence](https://img.shields.io/badge/confidence-{pct}%25-blue)"
            ));
        }
        if let Some(r) = note.revises_note {
            badges.push(format!(
                "![Revises](https://img.shields.io/badge/revises-note%20{r}-yellow)"
            ));
        }
        if let Some(b) = note.thread_from {
            badges.push(format!(
                "![Thread](https://img.shields.io/badge/thread-from%20{b}-purple)"
            ));
        }
        if !badges.is_empty() {
            lines.push(badges.join(" "));
        }

        lines.push(String::new());
        lines.push(format!("**Context:** {}", note.context));
        lines.push(String::new());
        lines.push(format!("**Thought:** {}", note.thought));

        if let Some(notes) = &note.uncertainty_notes {
            lines.push(String::new());
            lines.push(format!("> ⚠️ **Uncertainty:** {notes}"));
        }
        if let Some(reason) = &note.revision_reason {
            lines.push(String::new());
            lines.push(format!("> 🔄 **Revision Reason:** {reason}"));
        }

        lines.push(String::new());
        lines.push(format!("**Outcome:** {}", note.outcome));
        lines.push(String::new());

        let next_action = Self::format_next_action(&note.next_action);
        lines.push(format!("**Next Action:** {next_action}"));
        lines.push(format!("- *Rationale:* {}", note.rationale));

        if let Some(tools) = &note.tools_used {
            if !tools.is_empty() {
                lines.push(String::new());
                lines.push(format!("**Tools Used:** {}", tools.join(", ")));
            }
        }

        if let Some(exec_ref) = &note.run_ref {
            lines.push(String::new());
            lines.push(format!("**Execution Ref:** `{exec_ref}`"));
        }

        lines.push(String::new());
        lines.push("---".to_string());
        lines.join("\n")
    }

    pub fn format_note_json(&self, note: &Note) -> String {
        serde_json::to_string_pretty(note).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn format_thread_tree(&self, history: &Log) -> String {
        let mut lines = vec!["Thread Structure:".to_string()];
        let mut threads_by_note: std::collections::HashMap<u32, Vec<&Thread>> =
            std::collections::HashMap::new();
        if let Some(threads) = &history.threads {
            for b in threads {
                threads_by_note.entry(b.from_note).or_default().push(b);
            }
        }
        lines.push("Main:".to_string());
        for note in history.notes.iter().filter(|s| s.thread_id.is_none()) {
            lines.push(format!("  └─ Note {}: {}", note.note, note.purpose));
            if let Some(bs) = threads_by_note.get(&note.note) {
                for thread in bs {
                    lines.push(format!("     └─ Thread: {}", thread.name));
                    for bnote in &thread.notes {
                        lines.push(format!("        └─ Note {}: {}", bnote.note, bnote.purpose));
                    }
                }
            }
        }
        lines.join("\n")
    }
}
