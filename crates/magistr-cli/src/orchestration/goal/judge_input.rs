//! What the judge is asked.

use std::fmt::Write as _;

use super::gate_evidence::GateEvidence;
use super::verdict::Verdict;

/// Everything the judge is given: the chunk's acceptance list, entire
/// and unabridged — there is no 500-character cap here, which is one of
/// the three reasons the native judge leads — the real gate
/// evidence, and the tail of what the agent said.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct JudgeInput {
    /// The chunk's id.
    pub chunk: String,
    /// The chunk's title — the claim in one sentence.
    pub title: String,
    /// Every acceptance criterion, verbatim.
    pub acceptance: Vec<String>,
    /// The gates, as they really ran.
    pub gates: Vec<GateEvidence>,
    /// The last of the agent's own text this iteration.
    pub transcript_tail: String,
    /// Which iteration this is, of how many.
    pub iteration: (u32, u32),
}

impl JudgeInput {
    /// How much of the transcript's tail the prompt carries, in bytes.
    pub const TAIL_BYTES: usize = 12_000;

    /// The prompt, in full. Its load-bearing sentence is the evaluator's
    /// own: without clear evidence, the answer is *not yet*.
    #[must_use]
    pub fn prompt(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "You are judging whether a roadmap chunk is complete after iteration {} of {}.",
            self.iteration.0, self.iteration.1
        );
        let _ = writeln!(
            out,
            "Judge the acceptance criteria below against the evidence. The gate results \
             are the real exit codes of the project's quality gates — they are facts, not \
             the agent's account. If the evidence does not clearly show a criterion is met, \
             it is not met. If the transcript does not contain clear evidence that the \
             acceptance is satisfied, answer {{\"ok\": false, \"reason\": \"insufficient evidence\"}}."
        );
        let _ = writeln!(out, "\n## Chunk `{}`\n{}", self.chunk, self.title);
        let _ = writeln!(out, "\n## Acceptance criteria");
        if self.acceptance.is_empty() {
            let _ = writeln!(
                out,
                "(the chunk lists none: judge the title as the criterion)"
            );
        }
        for (n, criterion) in self.acceptance.iter().enumerate() {
            let _ = writeln!(out, "{}. {criterion}", n + 1);
        }
        let _ = writeln!(out, "\n## Quality gates (real exit codes)");
        if self.gates.is_empty() {
            let _ = writeln!(out, "(no gates are configured)");
        }
        for gate in &self.gates {
            let _ = writeln!(out, "{}", gate.line());
        }
        let _ = writeln!(out, "\n## The agent's last words this iteration");
        let _ = writeln!(out, "{}", tail(&self.transcript_tail, Self::TAIL_BYTES));
        let _ = writeln!(
            out,
            "\n## Answer\nReply with one JSON object and nothing else, matching this schema:\n{}\n\
             Use {{\"ok\": true}} only when every criterion is met and every gate passed; \
             {{\"ok\": false}} to iterate again; {{\"ok\": false, \"impossible\": true}} when the \
             acceptance cannot be met as written and a human must decide.",
            Verdict::SCHEMA
        );
        out
    }
}

/// The last `max` bytes of `text`, cut at a character boundary.
fn tail(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

#[cfg(test)]
#[path = "judge_input.tests.rs"]
mod tests;
