//! The Birth sub-state: a project being born, moment by moment.

use super::birth_cmd::BirthCmd;
use super::birth_moment::BirthMoment;
use super::born_data::BornData;
use super::ground::Ground;
use super::ground_field::GroundField;
use super::interview::Interview;
use super::motion::Motion;
use super::prompt::Prompt;
use super::proposal::Proposal;
use super::survey_report::SurveyReport;

/// A project being born as the model keeps it: which moment the flow
/// is at, the ground chosen, what the survey found and when it landed,
/// the interview, the proposal and when it was charted, and what
/// accepting it wrote. Native-only state in the shared model, like the
/// [`Galaxy`](super::Galaxy): a terminal has no Birth and never reads it.
///
/// Every motion here is a pure function of the tick it is read at —
/// the survey's rows land one by one from the tick the report arrived,
/// the radar's phase is the tick modulo its period, the charting's
/// stars are born one by one from the tick the proposal arrived — so
/// reduced motion collapses each to its end frame by never advancing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Birth {
    moment: BirthMoment,
    ground: Option<Ground>,
    armed: bool,
    field: Option<(GroundField, Prompt)>,
    survey: Option<(SurveyReport, u64)>,
    surveying: bool,
    interview: Option<Interview>,
    proposal: Option<Proposal>,
    charted_at: Option<u64>,
    charting: bool,
    born: Option<BornData>,
    editing: Option<(usize, Prompt)>,
    drag: Option<usize>,
    started_at: Option<u64>,
}

impl Birth {
    /// Ticks in one five-second revolution of the survey sweep.
    /// This differs from the breathing period to avoid synchronized pulses.
    pub const RADAR_STEPS: u64 = 48;
    /// Ticks the survey holds after its last row lands before the
    /// interview opens — one beat, so the last receipt is read.
    pub const SURVEY_BEAT: u64 = Motion::RISE_STAGGER * 3;
    /// Ticks between two stars being born on the charting — the rise's
    /// own stagger, so the proposal is dealt the way the Brief is.
    pub const BORN_STAGGER: u64 = Motion::RISE_STAGGER;

    /// Open the flow on the ground, `armed` when a Finder drag is over
    /// the window. Opening again holds where the flow is; only a
    /// flow that was never started (or stood down) lands on F1.
    pub(super) fn open(&mut self, armed: bool) {
        self.armed = armed;
        if self.moment == BirthMoment::Ground {
            self.field = None;
        }
    }

    /// A Finder drag over the window, or gone from it.
    pub(super) fn arm(&mut self, armed: bool) {
        self.armed = armed;
    }

    /// Open one of the ground card's fields for typing.
    pub(super) fn type_into(&mut self, field: GroundField) {
        self.field = Some((field, Prompt::new()));
    }

    /// Type into the open field.
    pub(super) fn field_char(&mut self, c: char) -> bool {
        if let Some((_, prompt)) = &mut self.field {
            prompt.insert(&c.to_string());
            return true;
        }
        false
    }

    /// Erase one character from the open field.
    pub(super) fn field_backspace(&mut self) -> bool {
        if let Some((_, prompt)) = &mut self.field {
            prompt.backspace();
            return true;
        }
        false
    }

    /// Close the open field without choosing; `true` when one was open.
    pub(super) fn close_field(&mut self) -> bool {
        self.field.take().is_some()
    }

    /// The ground the open field names, when it names one — `beside`
    /// is the window's own folder, which a clone lands next to.
    #[must_use]
    pub fn field_ground(&self, beside: &str) -> Option<Ground> {
        let (field, prompt) = self.field.as_ref()?;
        let text = prompt.text().trim();
        if text.is_empty() {
            return None;
        }
        Some(match field {
            GroundField::Browse => Ground::Folder(text.to_owned()),
            GroundField::Empty => Ground::Empty(text.to_owned()),
            GroundField::Clone => Ground::Clone {
                url: text.to_owned(),
                into: Ground::clone_into(text, beside),
            },
        })
    }

    /// Choose the ground at tick `now`: the flow moves to the survey
    /// and asks the entry point to read it, and the sweep starts its
    /// turn from the top. A survey already kept for the same ground is
    /// not re-read.
    pub(super) fn choose(&mut self, ground: Ground, now: u64) -> Option<BirthCmd> {
        self.field = None;
        self.armed = false;
        self.started_at = Some(now);
        let same = self.ground.as_ref() == Some(&ground);
        self.moment = BirthMoment::Survey;
        if same && self.survey.is_some() {
            return None;
        }
        self.ground = Some(ground.clone());
        self.survey = None;
        self.interview = None;
        self.proposal = None;
        self.charted_at = None;
        self.born = None;
        self.surveying = true;
        Some(BirthCmd::Survey { ground })
    }

    /// The survey's report arrived at tick `now`.
    pub(super) fn surveyed(&mut self, report: SurveyReport, now: u64) {
        self.surveying = false;
        self.survey = Some((report, now));
    }

    /// The survey job failed: the flow stands down to the ground with
    /// nothing kept, so it can be chosen again.
    pub(super) fn survey_failed(&mut self) {
        self.surveying = false;
        self.survey = None;
        self.ground = None;
        self.moment = BirthMoment::Ground;
    }

    /// Advance the flow where the tick took it: once every survey row
    /// has landed and held its beat, the interview opens. `true` when
    /// the moment changed.
    pub(super) fn settle(&mut self, now: f32, animate: bool) -> bool {
        if self.moment != BirthMoment::Survey {
            return false;
        }
        let Some((report, _)) = &self.survey else {
            return false;
        };
        if !self.survey_done(now, animate) {
            return false;
        }
        self.interview = Some(Interview::of(report));
        self.moment = BirthMoment::Interview;
        true
    }

    /// Pick or unpick a chip on the current question.
    pub(super) fn toggle_chip(&mut self, index: usize) -> bool {
        self.interview
            .as_mut()
            .is_some_and(|interview| interview.toggle(index))
    }

    /// Type into the current question's own-answer field.
    pub(super) fn answer_char(&mut self, c: char) -> bool {
        match &mut self.interview {
            Some(interview) if !interview.is_done() => {
                interview.type_char(c);
                true
            }
            _ => false,
        }
    }

    /// Erase one character from the current question's own answer.
    pub(super) fn answer_backspace(&mut self) -> bool {
        match &mut self.interview {
            Some(interview) if !interview.is_done() => {
                interview.backspace();
                true
            }
            _ => false,
        }
    }

    /// Continue: commit the current answer. When it was the last, the
    /// flow moves to the charting and asks the entry point to draft
    /// the proposal around whatever is pinned.
    pub(super) fn advance(&mut self) -> Option<BirthCmd> {
        let interview = self.interview.as_mut()?;
        if !interview.advance() {
            return None;
        }
        if !interview.is_done() {
            return None;
        }
        self.chart()
    }

    /// Ask for the proposal: the charting moment, with a draft in
    /// flight. A redraw asks again with the current pins.
    fn chart(&mut self) -> Option<BirthCmd> {
        let answers = self.interview.as_ref()?.answers()?;
        let pinned = self
            .proposal
            .as_ref()
            .map(Proposal::pinned)
            .unwrap_or_default();
        self.moment = BirthMoment::Charting;
        self.charting = true;
        Some(BirthCmd::Chart {
            dir: self.dir().to_owned(),
            name: self.name().to_owned(),
            answers,
            pinned,
        })
    }

    /// `Redraw`: draft again, keeping the pinned stars as constraints.
    pub(super) fn redraw(&mut self) -> Option<BirthCmd> {
        if self.moment != BirthMoment::Charting {
            return None;
        }
        self.editing = None;
        self.chart()
    }

    /// The proposal arrived at tick `now`: the stars are born from
    /// here.
    pub(super) fn charted(&mut self, proposal: Proposal, now: u64) {
        self.charting = false;
        self.proposal = Some(proposal);
        self.charted_at = Some(now);
        self.editing = None;
        self.drag = None;
    }

    /// A drag began on the list row at `index`.
    pub(super) fn drag_from(&mut self, index: usize) {
        self.drag = Some(index);
    }

    /// The drag ended over the row at `to`: the row it started on
    /// moves there. `false` when nothing was dragged or nothing moved.
    pub(super) fn drop_on(&mut self, to: usize) -> bool {
        let Some(from) = self.drag.take() else {
            return false;
        };
        self.proposal
            .as_mut()
            .is_some_and(|proposal| proposal.reorder(from, to))
    }

    /// Open the row at `index` for renaming — a right-click, or `Edit
    /// chunks…` on the first row.
    pub(super) fn edit(&mut self, index: usize) -> bool {
        let Some(chunk) = self.proposal.as_ref().and_then(|p| p.chunks.get(index)) else {
            return false;
        };
        let mut prompt = Prompt::new();
        prompt.insert(&chunk.name);
        self.editing = Some((index, prompt));
        true
    }

    /// Type into the row being renamed.
    pub(super) fn edit_char(&mut self, c: char) -> bool {
        if let Some((_, prompt)) = &mut self.editing {
            prompt.insert(&c.to_string());
            return true;
        }
        false
    }

    /// Erase one character from the row being renamed.
    pub(super) fn edit_backspace(&mut self) -> bool {
        if let Some((_, prompt)) = &mut self.editing {
            prompt.backspace();
            return true;
        }
        false
    }

    /// Commit the rename; `false` when nothing was being renamed.
    pub(super) fn commit_edit(&mut self) -> bool {
        let Some((index, prompt)) = self.editing.take() else {
            return false;
        };
        if let Some(proposal) = self.proposal.as_mut() {
            proposal.rename(index, prompt.text());
        }
        true
    }

    /// Drop the rename without committing; `true` when one was open.
    pub(super) fn cancel_edit(&mut self) -> bool {
        self.editing.take().is_some()
    }

    /// `Accept this roadmap`: ask the entry point to write it. Nothing
    /// is asked without a proposal to write.
    pub(super) fn accept(&mut self) -> Option<BirthCmd> {
        if self.moment != BirthMoment::Charting || self.charting {
            return None;
        }
        let proposal = self.proposal.clone()?;
        let interview = self.interview.as_ref()?;
        self.editing = None;
        Some(BirthCmd::Accept {
            dir: self.dir().to_owned(),
            name: self.name().to_owned(),
            proposal,
            never_break: interview.never_break(),
            freedom: interview.freedom(),
        })
    }

    /// The roadmap was written: first light.
    pub(super) fn born(&mut self, data: BornData) {
        self.born = Some(data);
        self.moment = BirthMoment::FirstLight;
    }

    /// Esc on a later moment: the flow stands down to the ground with
    /// what the survey found kept, so choosing the same folder again
    /// does not read it twice. `false` on the ground itself, whose Esc
    /// rises instead.
    pub(super) fn stand_down(&mut self) -> bool {
        if self.moment == BirthMoment::Ground {
            return false;
        }
        self.moment = BirthMoment::Ground;
        self.field = None;
        self.editing = None;
        self.drag = None;
        true
    }

    /// Which moment the flow is at.
    #[must_use]
    pub const fn moment(&self) -> BirthMoment {
        self.moment
    }

    /// Whether a Finder drag is over the window — the slot is armed.
    #[must_use]
    pub const fn is_armed(&self) -> bool {
        self.armed
    }

    /// The field being typed into, and its text.
    #[must_use]
    pub fn field(&self) -> Option<(GroundField, &str)> {
        self.field
            .as_ref()
            .map(|(field, prompt)| (*field, prompt.text()))
    }

    /// The ground chosen.
    #[must_use]
    pub const fn ground(&self) -> Option<&Ground> {
        self.ground.as_ref()
    }

    /// The project's folder — the ground's, or nothing before one is
    /// chosen.
    #[must_use]
    pub fn dir(&self) -> &str {
        self.ground.as_ref().map_or("", Ground::dir)
    }

    /// The project's name: what the survey named it, or the folder's
    /// own name until then.
    #[must_use]
    pub fn name(&self) -> &str {
        match &self.survey {
            Some((report, _)) if !report.name.is_empty() => report.name.as_str(),
            _ => self.ground.as_ref().map_or("", Ground::name),
        }
    }

    /// The survey's report, once it landed.
    #[must_use]
    pub fn survey(&self) -> Option<&SurveyReport> {
        self.survey.as_ref().map(|(report, _)| report)
    }

    /// Whether the survey is still being read — the `Scanning…` chip.
    #[must_use]
    pub fn is_scanning(&self, now: f32, animate: bool) -> bool {
        self.moment == BirthMoment::Survey && (self.surveying || !self.survey_done(now, animate))
    }

    /// How many of the survey's rows have landed at `now` — the facts
    /// first, then the receipts, one every [`Motion::RISE_STAGGER`]
    /// from the tick the report arrived; every row at once when the
    /// motion is not running.
    #[must_use]
    pub fn rows_landed(&self, now: f32, animate: bool) -> usize {
        let Some((report, at)) = &self.survey else {
            return 0;
        };
        if !animate {
            return report.rows();
        }
        let elapsed = now - *at as f32;
        if elapsed < 0.0 {
            return 0;
        }
        let landed = (elapsed / Motion::RISE_STAGGER as f32).floor() as usize + 1;
        landed.min(report.rows())
    }

    /// How far the survey row at `index` has risen, `0..1` — `None`
    /// once it has landed, and `None` before its turn.
    #[must_use]
    pub fn row_rise(&self, index: usize, now: f32, animate: bool) -> Option<f32> {
        if !animate {
            return None;
        }
        let (_, at) = self.survey.as_ref()?;
        let dealt = *at as f32 + (index as u64 * Motion::RISE_STAGGER) as f32;
        let elapsed = now - dealt;
        if elapsed < 0.0 {
            return None;
        }
        (elapsed < Motion::ARRIVAL_STEPS as f32).then_some(elapsed / Motion::ARRIVAL_STEPS as f32)
    }

    /// The scan meter: facts landed over facts to find.
    #[must_use]
    pub fn scan(&self, now: f32, animate: bool) -> (usize, usize) {
        let Some((report, _)) = &self.survey else {
            return (0, 0);
        };
        let total = report.detected.len();
        (self.rows_landed(now, animate).min(total), total)
    }

    /// Whether every row has landed and held its beat.
    fn survey_done(&self, now: f32, animate: bool) -> bool {
        let Some((report, at)) = &self.survey else {
            return false;
        };
        if !animate {
            return true;
        }
        let last = *at + (report.rows() as u64) * Motion::RISE_STAGGER + Self::SURVEY_BEAT;
        now >= last as f32
    }

    /// Where the sweep points, `0..1` of a turn at `now` — from the
    /// top at the tick the ground was chosen, so the survey always
    /// opens with the sweep starting.
    #[must_use]
    pub fn radar_phase(&self, now: f32) -> f32 {
        let since = now - self.started_at.unwrap_or(0) as f32;
        (since / Self::RADAR_STEPS as f32).rem_euclid(1.0)
    }

    /// The interview, once the survey opened it.
    #[must_use]
    pub const fn interview(&self) -> Option<&Interview> {
        self.interview.as_ref()
    }

    /// The proposal, once the charting delivered it.
    #[must_use]
    pub const fn proposal(&self) -> Option<&Proposal> {
        self.proposal.as_ref()
    }

    /// Whether a draft is in flight — the field is empty and waits.
    #[must_use]
    pub const fn is_charting(&self) -> bool {
        self.charting
    }

    /// How far the proposal's star at `index` has been born, `0..1`:
    /// `0` before its turn, `1` once it is there. Each star comes
    /// [`Birth::BORN_STAGGER`] after the last, fading in over the
    /// arrival; every star at once when the motion is not running.
    #[must_use]
    pub fn born_alpha(&self, index: usize, now: f32, animate: bool) -> f32 {
        if !animate {
            return 1.0;
        }
        let Some(at) = self.charted_at else {
            return 0.0;
        };
        let dealt = at as f32 + (index as u64 * Self::BORN_STAGGER) as f32;
        let elapsed = now - dealt;
        if elapsed < 0.0 {
            return 0.0;
        }
        (elapsed / Motion::ARRIVAL_STEPS as f32).min(1.0)
    }

    /// Whether the charting is still being drawn at `now`.
    #[must_use]
    pub fn is_being_born(&self, now: f32) -> bool {
        match (&self.proposal, self.charted_at) {
            (Some(proposal), Some(_)) => {
                let last = proposal.chunks.len().saturating_sub(1);
                self.born_alpha(last, now, true) < 1.0
            }
            _ => false,
        }
    }

    /// The row being renamed, and its text so far.
    #[must_use]
    pub fn editing(&self) -> Option<(usize, &str)> {
        self.editing
            .as_ref()
            .map(|(index, prompt)| (*index, prompt.text()))
    }

    /// The row a drag started on.
    #[must_use]
    pub const fn dragging(&self) -> Option<usize> {
        self.drag
    }

    /// What was written, once the proposal was accepted.
    #[must_use]
    pub const fn born_data(&self) -> Option<&BornData> {
        self.born.as_ref()
    }

    /// Whether anything on the flow moves at `now`: the survey landing
    /// its rows under the sweep, or the stars being born.
    #[must_use]
    pub fn is_moving(&self, now: f32) -> bool {
        match self.moment {
            BirthMoment::Survey => true,
            BirthMoment::Charting => self.is_being_born(now),
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "birth.tests.rs"]
mod tests;
