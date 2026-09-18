//! Permission and confirmation interaction — the one owner of how an
//! ask from the agent is answered from the screen.
//!
//! Four things live here because they encode one contract and change
//! together: [`Decision`], the feature's state (the conditional-approval
//! register and whether it is open, the card's scroll, the seen witness,
//! the ask a Moment was stood down for); [`DecisionIntent`], the
//! semantic actions a surface can raise; the reducer ([`apply`]) that
//! folds an intent into the model and answers with commands; and the
//! selectors on [`Model`] (`moment`, `review`, `ask`, `permissions_pending`,
//! `permissions_seen`, `condition`, `condition_open`) that every painter
//! reads. Keyboard ([`key`]), pointer ([`click`]) and the semantic
//! message route (`Msg::Decision`) all become an intent first, so the
//! three cannot disagree about what a gesture means, and a permission
//! is answered from exactly one place ([`answer`]).
//!
//! The authorities stay distinct. This module mints `Cmd::Permission`
//! and `Cmd::Constraint` — the provider's ask and the policy owner's
//! rule — and the merge that rides beside a review's approval. It
//! never asks the workspace (`Cmd::Workbench`) or the engine
//! (`Cmd::Engine`): a workspace confirmation and a workflow proposal
//! are other cards with other owners, and an answer here cannot stand
//! in for either.

use magistr_domain::{policy::RiskTier, workflow::CheckpointReason};

use super::charter_rule::CharterRule;
use super::checkpoint_cmd::CheckpointCmd;
use super::cmd::Cmd;
use super::copy::Copy;
use super::dock_item::DockItem;
use super::model::Model;
use super::permission::Permission;
use super::place::Place;
use super::prompt::Prompt;
use super::rule_mode::RuleMode;
use super::widget::Widget;
use crate::agent::ToolTitle;
use crate::input::{KeyCode, KeyEvent, Modifiers};
use crate::widget_id::WidgetId;
use magistr_application::sessions::{AskPlan, ImageBlock, PermissionChoice, PermissionKind};

/// Rows a page key moves the Moment's card by.
const PAGE: isize = 8;

/// The decision feature's state: what the screen holds about the ask
/// in front, beyond the ask itself (which waits on the dock).
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    /// The Moment's conditional-approval register — the line the
    /// reader types a standing constraint into ("allow, stop if other
    /// rows change").
    ///
    /// Its own buffer rather than the prompt's: the prompt is a turn
    /// waiting to be sent to the agent, and a condition is an answer to
    /// the question on screen. Sharing one would make Enter mean two
    /// things at once.
    condition: Prompt,
    /// Whether keys are being entered as a standing constraint. Tab
    /// opens the register explicitly, allowing text that starts with an
    /// answer's letter; any other key that answers nothing opens it
    /// too. Once open, every key is text.
    condition_open: bool,
    /// How far the front card's text is scrolled.
    scroll: usize,
    /// Permission requests that reached the cockpit so far — the
    /// witness that a session-wide *Always* kept the repeats away.
    seen: u64,
    /// The permission a Moment was stood down for (Esc on a window):
    /// it waits on the dock's strip unanswered instead of floating as
    /// the card, until it is answered from the strip.
    stood_down: Option<u64>,
    /// Answers retained until this turn settles, so failed delivery can be retried.
    submitted: Vec<Permission>,
}

impl Default for Decision {
    fn default() -> Self {
        Self {
            condition: Prompt::new(),
            condition_open: false,
            scroll: 0,
            seen: 0,
            stood_down: None,
            submitted: Vec::new(),
        }
    }
}

impl Decision {
    /// The conditional-approval register.
    #[must_use]
    pub const fn condition(&self) -> &Prompt {
        &self.condition
    }

    /// Whether keyboard input is being entered as a standing constraint.
    #[must_use]
    pub const fn condition_open(&self) -> bool {
        self.condition_open
    }

    /// How far the front card's text is scrolled.
    #[must_use]
    pub const fn scroll(&self) -> usize {
        self.scroll
    }

    /// Permission requests that reached the cockpit so far.
    #[must_use]
    pub const fn seen(&self) -> u64 {
        self.seen
    }

    /// Whether the ask `id` was stood down to the strip.
    #[must_use]
    pub const fn is_stood_down(&self, id: u64) -> bool {
        matches!(self.stood_down, Some(stood) if stood == id)
    }

    /// Move the scroll by `delta`, never below the top and never past
    /// `maximum` — the card's own end where the surface measured one,
    /// `usize::MAX` where it did not.
    pub(super) fn scroll_by(&mut self, delta: isize, maximum: usize) {
        self.scroll = self.scroll.saturating_add_signed(delta).min(maximum);
    }

    /// The ask `id` waits on the strip rather than as the card.
    pub(super) const fn stand_down(&mut self, id: u64) {
        self.stood_down = Some(id);
    }

    /// Whatever was stood down floats again — a stuck chunk opened
    /// from the sky wants its card back.
    pub(super) const fn recall(&mut self) {
        self.stood_down = None;
    }

    /// A settled turn can no longer retry its submitted answers.
    pub(super) fn settle(&mut self) {
        self.submitted.clear();
    }

    /// The ask left the dock and cannot be restored by a late failure.
    fn forget(&mut self, id: u64) {
        self.submitted.retain(|permission| permission.id() != id);
        if self.is_stood_down(id) {
            self.stood_down = None;
        }
    }

    /// Clear the register and close it. `true` when it was open — the
    /// Esc that clears is spent, the Esc on an empty register is not.
    pub(super) fn clear_condition(&mut self) -> bool {
        let was_open = self.condition_open;
        self.condition.take();
        self.condition_open = false;
        was_open
    }
}

/// A semantic action on the decision in front. Keyboard, pointer and
/// the desktop's actions all raise one of these; the reducer is the
/// only reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionIntent {
    /// Answer the front ask with `kind` — an accelerator, Enter or Esc,
    /// a button, a spoken action. A kind the ask does not offer is not
    /// an answer and changes nothing.
    Answer(PermissionKind),
    /// Put the pictures the asking call carries on the feed.
    ShowImages,
    /// Open the Moment's conditional register for text.
    OpenCondition,
    /// Clear the register and close it, keeping the decision.
    ClearCondition,
    /// Type `char` into the register, opening it if it was closed.
    Type(char),
    /// Delete the character before the register's cursor.
    Backspace,
    /// Move the register's cursor left.
    Left,
    /// Move the register's cursor right.
    Right,
    /// Scroll the Moment's card by `rows`, clamped to the card.
    Scroll(isize),
}

impl Model {
    /// The decision feature's state.
    #[must_use]
    pub const fn decision(&self) -> &Decision {
        &self.decision
    }

    /// The decision on screen, when one is: the front dock card, when
    /// it is a permission that carries a plan.
    ///
    /// The Moment is a *decision*, not every tool call — an ask with
    /// no plan stays the Needs-you dock's one-line strip, which is what
    /// leaves every surface the cockpit already draws exactly as it was.
    #[must_use]
    pub fn moment(&self) -> Option<&Permission> {
        match self.dock.front() {
            Some(DockItem::Permission(p)) if p.is_plan() && !self.is_stood_down(p) => Some(p),
            _ => None,
        }
    }

    /// Whether `p` is the ask a Moment was stood down for: it waits on
    /// the dock's strip, not as the card.
    fn is_stood_down(&self, p: &Permission) -> bool {
        self.decision.is_stood_down(p.id())
    }

    /// The review on screen, when one is: the front dock card, when it
    /// is an **edit** permission and the surface is a window.
    ///
    /// "Review a change" — the other Moment. Two halves to the
    /// predicate and each does a job the other cannot. *Edit* is what
    /// makes the decision a review rather than an ask: a plan approval
    /// is [`Model::moment`]'s card, and an edit is a diff somebody has
    /// to read. *Native* is what leaves the terminal alone — a cell
    /// cockpit has a Session view, a tab bar and the Needs-you strip, so
    /// it is already steered at this altitude and its cell diff is
    /// untouched; a window has none of those and the review is the
    /// surface. Because the predicate itself is surface-gated, "every
    /// terminal golden is byte-identical" is a property of the wiring
    /// rather than of goldens agreeing.
    #[must_use]
    pub fn review(&self) -> Option<&Permission> {
        if !self.surface.is_native() {
            return None;
        }
        match self.dock.front() {
            Some(DockItem::Permission(p)) if p.is_edit() && !self.is_stood_down(p) => Some(p),
            _ => None,
        }
    }

    /// The ask on screen, when one is: the front dock card, when it is
    /// a **plain** permission — neither a plan nor an edit — the
    /// surface is a window and the camera is at the Session.
    ///
    /// The third Moment. The terminal shows a plain ask on its Needs-you
    /// strip; the window lays that strip out nowhere, so without this
    /// an ask raised mid-loop had no surface at all and sat until the
    /// transport's timeout. Gated to the Session because that is the
    /// loop's stage and where the answer keys already land; at the
    /// Galaxy and the Sky the ask is the project's asking count and the
    /// star's *Stuck* — view 01's capture carries a plan-less ask over a
    /// fully lit room, and a scrim there would move it.
    #[must_use]
    pub fn ask(&self) -> Option<&Permission> {
        if !self.surface.is_native() || self.place != Place::Session {
            return None;
        }
        match self.dock.front() {
            Some(DockItem::Permission(p))
                if !p.is_plan() && !p.is_edit() && !self.is_stood_down(p) =>
            {
                Some(p)
            }
            _ => None,
        }
    }

    /// How many permission requests wait for an answer.
    #[must_use]
    pub fn permissions_pending(&self) -> usize {
        self.dock.permissions_pending()
    }

    /// Permission requests that reached the cockpit so far — the witness
    /// that a session-wide *Always* kept the repeats away.
    #[must_use]
    pub const fn permissions_seen(&self) -> u64 {
        self.decision.seen()
    }

    /// The Moment's conditional-approval register.
    #[must_use]
    pub const fn condition(&self) -> &Prompt {
        self.decision.condition()
    }

    /// Whether keyboard input is being entered as a standing constraint.
    #[must_use]
    pub const fn condition_open(&self) -> bool {
        self.decision.condition_open()
    }

    /// The front card, when it is a permission.
    fn front_permission(&self) -> Option<&Permission> {
        match self.dock.front() {
            Some(DockItem::Permission(p)) => Some(p),
            _ => None,
        }
    }
}

/// A key for the ask in front. A permission takes every key — the
/// agent is blocked on it — so this answers `Some` whenever the dock's
/// front is a permission, and `None` when it is not the decision's to
/// take.
///
/// At a Moment two régimes hold, and which one is in force is decided
/// by the conditional register alone: while it is empty the design's
/// single letters answer (`A` allow, `⇧A` always, `D` deny, `⇧D` never,
/// `space` show the pictures); once it holds anything the letters are
/// *text* and the answer is Enter. That is the rule the dock's proposal
/// cards already follow — the writing surface owns its letters — and it
/// is what lets one card be both a one-keystroke exit and a place to
/// write "allow, stop if other rows change". Quit and cancel are
/// answered before this (`quit_or_cancel`), so a wedged card can always
/// be left.
///
/// On the strip, an accelerator, Enter (allow) or Esc (deny) answers;
/// anything else is swallowed. SHIFT is allowed through because it is
/// what *carries* the session-wide answers: the decoder delivers ⇧A as
/// `Char('A')` with SHIFT set, and the accelerator table is keyed on
/// the char the key actually produced.
pub(super) fn key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    let plain = key.mods.is_empty() || key.mods == Modifiers::SHIFT;
    if model.moment().is_some() {
        let intent = moment_intent(model, key, plain);
        return Some(intent.map_or_else(Vec::new, |intent| apply(model, intent)));
    }
    model.front_permission()?;
    let intent = match key.code {
        KeyCode::Enter => Some(DecisionIntent::Answer(PermissionKind::AllowOnce)),
        KeyCode::Esc => Some(DecisionIntent::Answer(PermissionKind::RejectOnce)),
        KeyCode::Char(c) if plain => {
            PermissionKind::from_accelerator(c).map(DecisionIntent::Answer)
        }
        _ => None,
    };
    Some(intent.map_or_else(Vec::new, |intent| apply(model, intent)))
}

/// What `key` means at the Moment, or nothing when it is swallowed.
fn moment_intent(model: &Model, key: KeyEvent, plain: bool) -> Option<DecisionIntent> {
    let typing = model.decision.condition_open;
    Some(match key.code {
        KeyCode::PageDown => DecisionIntent::Scroll(PAGE),
        KeyCode::PageUp => DecisionIntent::Scroll(-PAGE),
        // `⇥` is the way in for a sentence that *starts* with one of
        // the answers, which the documented example does.
        KeyCode::Tab if !typing => DecisionIntent::OpenCondition,
        // The lit exit, whichever régime is in force: with an empty
        // register Enter is plain Allow, with a written one it is the
        // conditional approval.
        KeyCode::Enter => DecisionIntent::Answer(PermissionKind::AllowOnce),
        // Esc clears what was written before it denies — a reader who
        // typed a condition and thought better of it should lose the
        // sentence, not the decision.
        KeyCode::Esc if typing => DecisionIntent::ClearCondition,
        KeyCode::Esc => DecisionIntent::Answer(PermissionKind::RejectOnce),
        KeyCode::Backspace if typing => DecisionIntent::Backspace,
        KeyCode::Left if typing => DecisionIntent::Left,
        KeyCode::Right if typing => DecisionIntent::Right,
        KeyCode::Char(c) if plain => {
            if !typing {
                if c == ' ' && hidden_images(model) {
                    return Some(DecisionIntent::ShowImages);
                }
                // A space with nothing to show is the first character
                // of a condition, not a dead key.
                if c != ' '
                    && let Some(kind) = PermissionKind::from_accelerator(c)
                {
                    return Some(DecisionIntent::Answer(kind));
                }
            }
            // Any key that answers nothing is the first character of a
            // condition rather than a dead key.
            DecisionIntent::Type(c)
        }
        _ => return None,
    })
}

/// A click on an answer button or on the *Show images* control, on the
/// strip or on the Moment's card alike — the target is the id the
/// strip's own button carries, so nothing downstream can tell the two
/// apart. `None` for any other target, including a click inside the
/// card, which the pointer fold turns into
/// [`DecisionIntent::OpenCondition`] itself.
pub(super) fn click(model: &mut Model, target: Option<WidgetId>) -> Option<Vec<Cmd>> {
    if target == Some(Widget::ShowImages.id()) {
        return Some(apply(model, DecisionIntent::ShowImages));
    }
    let kind = target.and_then(Widget::from_id).and_then(Widget::answers)?;
    Some(apply(model, DecisionIntent::Answer(kind)))
}

/// Fold `intent` into the model. A Moment's answer travels through
/// [`moment_answer`], the strip's through [`answer`]; the register's
/// intents mean nothing without a Moment, and a scroll clamps to the
/// card the surface measured.
pub(super) fn apply(model: &mut Model, intent: DecisionIntent) -> Vec<Cmd> {
    match intent {
        DecisionIntent::Answer(kind) => {
            if model.moment().is_some() {
                return moment_answer(model, kind);
            }
            let option = model
                .front_permission()
                .and_then(|p| p.choice(kind))
                .map(str::to_owned);
            answer(model, option)
        }
        DecisionIntent::ShowImages => show_images(model),
        DecisionIntent::OpenCondition => {
            if model.moment().is_some() {
                model.decision.condition_open = true;
            }
            Vec::new()
        }
        DecisionIntent::ClearCondition => {
            if model.moment().is_some() {
                model.decision.clear_condition();
            }
            Vec::new()
        }
        DecisionIntent::Type(c) => {
            if model.moment().is_some() {
                model.decision.condition_open = true;
                model.decision.condition.insert(c.encode_utf8(&mut [0; 4]));
            }
            Vec::new()
        }
        DecisionIntent::Backspace => {
            if model.moment().is_some() && model.decision.condition_open {
                model.decision.condition.backspace();
            }
            Vec::new()
        }
        DecisionIntent::Left => {
            if model.moment().is_some() && model.decision.condition_open {
                model.decision.condition.left();
            }
            Vec::new()
        }
        DecisionIntent::Right => {
            if model.moment().is_some() && model.decision.condition_open {
                model.decision.condition.right();
            }
            Vec::new()
        }
        DecisionIntent::Scroll(rows) => {
            if model.uses_flight_recorder() {
                super::view::scroll_permission(model, rows);
            }
            Vec::new()
        }
    }
}

/// Whether the Moment still has pictures the reader has not asked for.
fn hidden_images(model: &Model) -> bool {
    model
        .moment()
        .is_some_and(|p| !p.hidden_images().is_empty())
}

/// *Show images*: the pictures the asking call carries go onto the feed
/// through the one rendering route a picture has ever taken
/// (`session::picture`), so a screenshot on a plan card is the same
/// line, the same wrap and the same compositor placement as a
/// screenshot in a tool result. The card keeps its place; only its
/// button leaves.
fn show_images(model: &mut Model) -> Vec<Cmd> {
    if model.moment().is_none() {
        return Vec::new();
    }
    let images: Vec<ImageBlock> = match model.dock.front_mut() {
        Some(DockItem::Permission(p)) => p.reveal_images(),
        _ => Vec::new(),
    };
    for block in &images {
        super::session::picture(model, block);
    }
    Vec::new()
}

/// Answer the Moment with `kind`, carrying whatever the register holds.
///
/// The answer itself is [`answer`] — the very command the dock strip
/// and a click on the card's button both produce, so there is one way
/// a permission is ever answered. What a Moment adds is the second
/// half of conditional approval: a written condition, or the scope a
/// `⇧A` covers, becomes a charter row the reader can read back, and the
/// written words go to whoever owns the policy as a [`Cmd::Constraint`]
/// *ahead of* the answer, while it still holds the request they are
/// about. A `⇧A` sends no words: its scope is the request's own, read
/// off the answer by the policy.
fn moment_answer(model: &mut Model, kind: PermissionKind) -> Vec<Cmd> {
    let Some(permission) = model.moment() else {
        return Vec::new();
    };
    // A card offers only the choices the agent listed: a key naming one
    // it did not offer is not an answer, and must not clear the
    // register or take the card away.
    let Some(option) = permission.choice(kind).map(str::to_owned) else {
        return Vec::new();
    };
    let scope = permission.scope().map(str::to_owned);
    let id = permission.id();
    let condition = model.decision.condition.take().trim().to_owned();
    model.decision.condition_open = false;
    let mut cmds = Vec::new();
    if !condition.is_empty() {
        cmds.push(Cmd::Constraint {
            id,
            condition: condition.clone(),
        });
    }
    cmds.extend(answer(model, Some(option)));
    model.decision.scroll = 0;
    if let Some((subject, mode)) = constraint(&condition, kind, scope.as_deref()) {
        model.rail.note_charter(CharterRule::new(&subject, mode));
    }
    cmds
}

/// The standing constraint an answer leaves behind, if any: the written
/// condition when there is one, else the scope a session-wide answer
/// covers. A plain once-only answer leaves nothing — the whole point of
/// "once" is that it is not a rule.
fn constraint(
    condition: &str,
    kind: PermissionKind,
    scope: Option<&str>,
) -> Option<(String, RuleMode)> {
    if !condition.is_empty() {
        // A condition is written as a *reason to come back*, so the
        // rule it records asks: the reader said what would make them
        // want to be interrupted again.
        return Some((condition.to_owned(), RuleMode::Ask));
    }
    let (verb, mode) = match kind {
        PermissionKind::AllowAlways => ("always allow", RuleMode::Auto),
        PermissionKind::RejectAlways => ("never allow", RuleMode::Off),
        PermissionKind::AllowOnce | PermissionKind::RejectOnce => return None,
    };
    Some((
        scope.map_or_else(|| verb.to_owned(), |scope| format!("{verb} · {scope}")),
        mode,
    ))
}

/// Answer the front permission with `option_id`; without one (a key that
/// answers nothing, a click elsewhere) the request stays.
fn answer(model: &mut Model, option_id: Option<String>) -> Vec<Cmd> {
    let Some(option_id) = option_id else {
        return Vec::new();
    };
    // Read before the card is popped: whether this answer is a review's
    // *Approve & merge* is a fact about the surface the answer came
    // from, and popping the card takes it away.
    let reviewing = model.review().is_some();
    let since = model.review_since().map(str::to_owned);
    let Some(permission) = model.dock.pop_permission() else {
        return Vec::new();
    };
    let approved = [PermissionKind::AllowOnce, PermissionKind::AllowAlways]
        .into_iter()
        .any(|kind| permission.choice(kind) == Some(option_id.as_str()));
    let mut cmds = vec![Cmd::Permission {
        id: permission.id(),
        option_id,
    }];
    // Approve merges the worktree. It rides *beside* the answer rather
    // than replacing it, and it goes out from the one place a
    // permission is ever answered, so the click on the button and the
    // key that presses it cannot disagree about whether the work came
    // home. A review scoped to the journal names no checkpoint and
    // therefore asks for no merge — there is no branch a fork opened to
    // bring back (`review-merge-needs-a-fork`).
    if reviewing
        && approved
        && let Some(id) = since
    {
        cmds.push(Cmd::Checkpoint(CheckpointCmd::Merge { id }));
    }
    model.decision.submitted.push(permission);
    cmds
}

/// \[Approve\] on the Changes tab for `path`: when a policy held the
/// edit for review and put its diff-approval card on the Needs-you
/// dock, that card is answered with its plain allow, so the human
/// reviews the diff once rather than twice. No card for the file means
/// nothing — the approval is the reviewed-mark the Changes tab already
/// made, and a plain ask that merely names the path is the provider's
/// own question, never answered by a workspace review.
pub(super) fn approve_edit(model: &mut Model, path: &str) -> Vec<Cmd> {
    let Some(permission) = model.dock.take_edit(path) else {
        return Vec::new();
    };
    let Some(option_id) = permission.allow().map(str::to_owned) else {
        return Vec::new();
    };
    let id = permission.id();
    model.decision.submitted.push(permission);
    vec![Cmd::Permission { id, option_id }]
}

/// A permission request arrived: the card goes onto the dock's queue
/// ahead of its proposals and blockers, the screen-reader channel says
/// so, and a risky allow names the state it can be undone to.
///
/// The checkpoint's name is minted here, in the same fold that raises
/// the card, because the card has to print it — see `Spine::open`.
/// Only `High`: a checkpoint before every ask would put a chain of
/// indistinguishable states on a rail whose whole value is that a state
/// on it is worth going back to.
pub(super) fn request(
    model: &mut Model,
    id: u64,
    title: &str,
    choices: Vec<PermissionChoice>,
    edit: bool,
    risk: RiskTier,
    plan: Option<AskPlan>,
) -> Vec<Cmd> {
    model.decision.seen += 1;
    if model.moment().is_none() {
        model.decision.scroll = 0;
    }
    let title = ToolTitle::display(title);
    let announce = Cmd::Announce(format!("needs you: {title}"));
    let mut permission = Permission::new(id, title, choices);
    if let Some(plan) = plan {
        permission = permission.about(plan);
    }
    if edit {
        permission = permission.edit();
    }
    let mut cmds = vec![announce];
    if risk == RiskTier::High {
        let label = permission.title().to_owned();
        let cp = model
            .rail
            .spine_mut()
            .open(CheckpointReason::Risk, label.clone());
        permission = permission.at_checkpoint(&cp);
        cmds.push(Cmd::Checkpoint(CheckpointCmd::Save {
            id: cp,
            reason: CheckpointReason::Risk,
            label,
        }));
    }
    model.dock.push_permission(permission);
    cmds
}

/// The transport denied an ask nobody answered (fail closed): the card
/// comes down wherever it waited, and the denial is said where the wait
/// was named. Nothing when the ask was not on the dock.
pub(super) fn lapsed(model: &mut Model, id: u64, title: &str, after_secs: u64) -> Vec<Cmd> {
    if model.dock.remove_permission(id).is_none() {
        return Vec::new();
    }
    model.decision.forget(id);
    let line = format!(
        "denied after {}: nobody answered {}",
        Copy::duration(after_secs),
        ToolTitle::display(title)
    );
    model.session.turn.push_note(&line);
    vec![Cmd::Announce(line)]
}

/// A failed answer returns its actual question, never a new permission.
pub(super) fn delivery_failed(model: &mut Model, id: u64) {
    if let Some(index) = model
        .decision
        .submitted
        .iter()
        .position(|permission| permission.id() == id)
    {
        let permission = model.decision.submitted.remove(index);
        model.dock.push_permission(permission);
        model.decision.recall();
    }
}

/// The turn the ask belonged to was cancelled: the card leaves without
/// claiming a timeout denial.
pub(super) fn cancelled(model: &mut Model, id: u64) {
    model.dock.remove_permission(id);
    model.decision.forget(id);
}

#[cfg(test)]
#[path = "decision.tests.rs"]
mod tests;
