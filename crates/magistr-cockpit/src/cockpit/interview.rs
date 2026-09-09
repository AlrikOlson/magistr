//! The three questions a project is born with.

use super::prompt::Prompt;
use super::survey_report::SurveyReport;

/// The interview (Birth F3): three questions, not thirty — what the
/// project should become, what must never break, and how free the
/// agent is to move. Each is a card; the answered ones stack above the
/// current one. Chips the survey inferred come pre-offered; the
/// reader's own words always win — an answer typed is the answer,
/// whatever was picked.
///
/// The answers seed what follows: question one is the charting's
/// goal, question two's picks become required gates and protected
/// paths, and question three sets the charter dial's starting notch
/// ([`Interview::freedom`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Interview {
    current: usize,
    questions: Vec<Question>,
}

/// One question: its prompt, the short name the `Next:` cap uses, the
/// chips offered, which are picked, the reader's own words, and the
/// answer once it was given.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    prompt: String,
    short: &'static str,
    chips: Vec<String>,
    picked: Vec<bool>,
    own: Prompt,
    answer: Option<String>,
    single: bool,
}

impl Interview {
    /// The charter's starting notches, question three's chips — from
    /// the tightest hand to the freest, each the standing order the
    /// notch becomes.
    pub const FREEDOM: [&'static str; 3] = [
        "ask before every write",
        "edits allowed, commands ask",
        "free rein with checkpoints",
    ];

    /// The interview for the project `report` surveyed.
    #[must_use]
    pub fn of(report: &SurveyReport) -> Self {
        let name = if report.name.is_empty() {
            "this project"
        } else {
            report.name.as_str()
        };
        Self {
            current: 0,
            questions: vec![
                Question::new(
                    format!("What should {name} become?"),
                    "the goal",
                    report.becomes.clone(),
                    false,
                ),
                Question::new(
                    "What must never break?",
                    "what must never break",
                    report.never_break.clone(),
                    false,
                ),
                Question::new(
                    "How free is the agent to move?",
                    "agent permissions",
                    Self::FREEDOM.iter().map(|s| (*s).to_owned()).collect(),
                    true,
                ),
            ],
        }
    }

    /// Which question is being asked, counting from zero.
    #[must_use]
    pub const fn current(&self) -> usize {
        self.current
    }

    /// How many questions there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.questions.len()
    }

    /// Whether there are no questions at all — never, for an interview
    /// built by [`Interview::of`], but the pair `len`/`is_empty` is the
    /// reading a collection owes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.questions.is_empty()
    }

    /// Whether every question has been answered.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.current >= self.questions.len()
    }

    /// The question at `index`.
    #[must_use]
    pub fn question(&self, index: usize) -> Option<&Question> {
        self.questions.get(index)
    }

    /// The question being asked, while one is.
    #[must_use]
    pub fn asking(&self) -> Option<&Question> {
        self.questions.get(self.current)
    }

    /// The questions answered so far, in order — the cards stacked
    /// above the current one.
    #[must_use]
    pub fn answered(&self) -> Vec<&Question> {
        self.questions
            .iter()
            .take(self.current)
            .filter(|q| q.answer.is_some())
            .collect()
    }

    /// How many questions remain after the current one.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.questions.len().saturating_sub(self.current + 1)
    }

    /// The short name of the question after the current one — what
    /// `Next:` says — or `None` on the last.
    #[must_use]
    pub fn next_short(&self) -> Option<&'static str> {
        self.questions.get(self.current + 1).map(|q| q.short)
    }

    /// Pick or unpick chip `index` on the current question. A
    /// single-select question (the charter notch) keeps one pick.
    pub(super) fn toggle(&mut self, index: usize) -> bool {
        let Some(question) = self.questions.get_mut(self.current) else {
            return false;
        };
        question.toggle(index)
    }

    /// Type into the current question's own-answer field.
    pub(super) fn type_char(&mut self, c: char) {
        if let Some(question) = self.questions.get_mut(self.current) {
            question.own.insert(&c.to_string());
        }
    }

    /// Erase one character from the current question's own answer.
    pub(super) fn backspace(&mut self) {
        if let Some(question) = self.questions.get_mut(self.current) {
            question.own.backspace();
        }
    }

    /// Commit the current answer and advance — `false` when nothing
    /// was picked or typed, and the question stays.
    pub(super) fn advance(&mut self) -> bool {
        let Some(question) = self.questions.get_mut(self.current) else {
            return false;
        };
        if !question.commit() {
            return false;
        }
        self.current += 1;
        true
    }

    /// The three answers, once every question has one.
    #[must_use]
    pub fn answers(&self) -> Option<[String; 3]> {
        if !self.is_done() {
            return None;
        }
        let mut out = [String::new(), String::new(), String::new()];
        for (slot, question) in out.iter_mut().zip(&self.questions) {
            *slot = question.answer.clone()?;
        }
        Some(out)
    }

    /// The picks on "what must never break" — each becomes a required
    /// gate or a protected path, whichever the entry point reads it as.
    #[must_use]
    pub fn never_break(&self) -> Vec<String> {
        self.questions
            .get(1)
            .map(Question::picks)
            .unwrap_or_default()
    }

    /// The charter dial's starting notch, `0` the tightest — question
    /// three's pick, or `None` when the reader wrote their own order.
    #[must_use]
    pub fn freedom(&self) -> Option<usize> {
        self.questions
            .get(2)
            .and_then(|q| q.picked.iter().position(|p| *p))
    }
}

impl Question {
    fn new(
        prompt: impl Into<String>,
        short: &'static str,
        chips: Vec<String>,
        single: bool,
    ) -> Self {
        let picked = vec![false; chips.len()];
        Self {
            prompt: prompt.into(),
            short,
            chips,
            picked,
            own: Prompt::new(),
            answer: None,
            single,
        }
    }

    /// The question, as the card asks it.
    #[must_use]
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// The chips offered, each with whether it is picked.
    #[must_use]
    pub fn chips(&self) -> Vec<(&str, bool)> {
        self.chips
            .iter()
            .map(String::as_str)
            .zip(self.picked.iter().copied())
            .collect()
    }

    /// The reader's own words so far.
    #[must_use]
    pub fn own(&self) -> &str {
        self.own.text()
    }

    /// The answer, once given.
    #[must_use]
    pub fn answer(&self) -> Option<&str> {
        self.answer.as_deref()
    }

    /// The picked chips' words.
    fn picks(&self) -> Vec<String> {
        self.chips
            .iter()
            .zip(&self.picked)
            .filter(|(_, picked)| **picked)
            .map(|(chip, _)| chip.clone())
            .collect()
    }

    fn toggle(&mut self, index: usize) -> bool {
        let Some(slot) = self.picked.get(index).copied() else {
            return false;
        };
        if self.single {
            for pick in &mut self.picked {
                *pick = false;
            }
        }
        self.picked[index] = !slot;
        true
    }

    /// The answer is the reader's own words when there are any, the
    /// picks otherwise; nothing at all refuses to advance.
    fn commit(&mut self) -> bool {
        let own = self.own.text().trim().to_owned();
        let answer = if own.is_empty() {
            self.picks().join(" \u{b7} ")
        } else {
            own
        };
        if answer.is_empty() {
            return false;
        }
        self.answer = Some(answer);
        true
    }
}

#[cfg(test)]
#[path = "interview.tests.rs"]
mod tests;
