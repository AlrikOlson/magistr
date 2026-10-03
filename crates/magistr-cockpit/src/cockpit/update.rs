//! Folding one message into the model.

use magistr_domain::workflow::CheckpointReason;

use magistr_workflow_contracts::{FeedStatus, MagistrFrame, NativeEvent};

use super::bad_day::BadDay;
use super::binding::Binding;
use super::birth_moment::BirthMoment;
use super::changes;
use super::charter_rule::CharterRule;
use super::checkpoint_cmd::CheckpointCmd;
use super::checkpoint_event::CheckpointEvent;
use super::cmd::Cmd;
use super::decision::{self, DecisionIntent};
use super::density::Density;
use super::dock_item::DockItem;
use super::engine_cmd::EngineCmd;
use super::ground::Ground;
use super::ground_field::GroundField;
use super::inspector::Inspector;
use super::launch_action::LaunchAction;
use super::loop_cmd::LoopCmd;
use super::loop_event::LoopEvent;
use super::loop_status::LoopStatus;
use super::loop_verdict::LoopVerdict;
use super::model::Model;
use super::msg::Msg;
use super::nav;
use super::palette::Palette;
use super::palette_row::{PaletteGo, PaletteRow};
use super::pane::Pane;
use super::phase::DisplayPhase;
use super::place::Place;
use super::request_phase::RequestPhase;
use super::rule_mode::RuleMode;
use super::search::Search;
use super::search_hit::{SearchGo, SearchHit};
use super::search_scope::SearchScope;
use super::selected::Selected;
use super::session;
use super::settings::Settings;
use super::settings_group::SettingsGroup;
use super::shelf_cmd::ShelfCmd;
use super::stage_tab::StageTab;
use super::star_state::StarState;
use super::term_hold::TermHold;
use super::theme_choice::ThemeChoice;
use super::turn_state::TurnState;
use super::widget::Widget;
use super::work::{self, open_chunk};
use super::workbench_cmd::WorkbenchCmd;
use crate::engine::EngineEvent;
use crate::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use crate::job::JobOutput;
use crate::theme::MotionMode;
use crate::turn::{Viewport, Voice};
use crate::widget_id::WidgetId;
use magistr_application::sessions::{AgentEvent, TurnEnd};

/// Apply `msg` to `model`; the returned commands are for the runtime.
///
/// The background reply moves a detected theme to dark or light (a
/// forced one stays) and re-highlights the turn's code. A resize
/// re-wraps the turn. Tab and Shift-Tab cycle terminal panes; Ctrl-Tab
/// cycles views. Other keys go by state: a pending permission on the dock's front takes the
/// rest first (its accelerators, Enter for allow, Esc for deny answer it
/// with [`Cmd::Permission`]); a proposal or blocker card on the front
/// answers only on an empty prompt (`y` accept / `n` reject with
/// [`EngineCmd::ResolveProposal`], `c` clear with
/// [`EngineCmd::ClearBlocker`]) — with text
/// in the prompt the letters stay the prompt's; ctrl-D on an
/// empty prompt asks to quit; ctrl-C cancels a streaming
/// turn and quits otherwise; Enter with text and no turn streaming sends
/// the prompt ([`Cmd::Prompt`]) and echoes it on the stage — on an empty
/// prompt it expands or collapses the most recent tool call, as a left
/// click on a stage row does for the call sitting there; everything
/// else edits the prompt — printable keys and pastes insert,
/// Alt/Shift-Enter breaks the line and the box grows to show the lines,
/// Backspace/Delete/Left/Right/ctrl-A/E/U/K/W as in a shell, Up/Down
/// move across the prompt's lines and walk the history past its edges,
/// Home/End move the cursor when there is text and scroll the stage
/// when there is none, PageUp/PageDown and the wheel always scroll. A
/// typed `@` (anywhere) or `/` (at a line's start) opens the completion
/// popup — the workspace's files or the command set — which re-filters
/// on every edit and closes when nothing matches; a click on a popup
/// row accepts that candidate. A click on a tab switches to it; a click on a front card's
/// button answers the card. Every key,
/// mouse report and paste is counted; presses, clicks, wheel and pastes
/// are also remembered — releases, repeats and pointer motion are not.
/// Agent events are counted and folded into the turn (text, thought and
/// the echoed prompt as prose, tool calls as lines, the end of the turn
/// as [`TurnState::AwaitingInput`] with a note when it ended early, a
/// transport error as an error line, a permission request onto the
/// dock's queue ahead of its proposals and blockers); engine frames are
/// counted, the feed's status kept,
/// each frame folded into the rail (the ship plan, the roadmap's
/// position, the pending signals) and into the dock (open proposals and
/// awaiting-human blockers appear and clear as the frames say), a seed
/// replacing the rail's state
/// whole, and the phase a frame implies taken — never backwards within
/// an iteration (see [`DisplayPhase::from_frame`]); a click on the rail's
/// toggle collapses or expands it and a drag on its divider resizes it,
/// both rewrapping the turn to the stage's new width; a job result is kept with the frame it
/// landed on and settles the gate LED running as that job. Loop events
/// fold into the loop status: `Started` turns the stage to the new
/// iteration (the transcript stays, the phase starts over), `Elapsed`
/// and `GateStarted` update the strip, `Note`,
/// `Failed`, `Paused` and `Ended` also leave a line on the stage. While a
/// loop runs, Space on an empty prompt pauses it ([`LoopCmd::Pause`]) or
/// resumes it, ahead of the dock; while paused, `s` skips
/// the next iteration, Esc aborts the loop, and other keys are swallowed
/// (ctrl-D still quits, a pending permission still takes its accelerators).
/// Whatever else it does, `update` compares the model's
/// [`MotionKey`](super::MotionKey) across the fold and hands the
/// changes to the motion layer — a progress change eases, a tab switch
/// or a dock arrival transitions — and watches
/// the model's declared [`Animations`](super::Animations) across it —
/// the set of things that move, each with its own period, reconciled to
/// the one interval the runtime owns. Whenever that interval changes it
/// re-declares with [`Cmd::Animate`], and when the set empties it takes
/// the tick back with [`Cmd::Still`]. That is what keeps an idle
/// cockpit at zero frames a second, and what lets the clocks ask for a
/// second while the motion layer asks for sixty milliseconds.
pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    let was = model.animations().period();
    let before = model.motion_key();
    // Where the window sits in the turn's content, read before anything
    // can move its rows (stage-scroll-anchor-and-rate): a card opening
    // above the window, an open card's detail growing, a re-wrap for a
    // new width all shift row indices, and a window pinned to a row
    // would show other content. Held to the line it was showing
    // instead, and re-pinned on that line's row once the fold has moved
    // it — every fold, so no re-wrap anyone adds can forget to.
    let anchor = model.stage_anchor();
    let layout = model.session.turn.layout();
    let mut cmds = fold(model, msg);
    model.sync_flight_wrap();
    if let Some(anchor) = anchor
        && model.session.turn.layout() != layout
    {
        model.hold_stage_anchor(anchor);
    }
    let after = model.motion_key();
    let animate = model.theme.motion().is_full();
    model.motion.observe(before, after, model.now(), animate);
    let now = model.animations().period();
    if now != was {
        cmds.push(now.map_or(Cmd::Still, Cmd::Animate));
    }
    cmds
}

/// The fold proper; [`update`] wraps it with the animation declaration.
fn fold(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    if matches!(
        msg,
        Msg::Key(_) | Msg::Mouse { .. } | Msg::Paste(_) | Msg::PromptReplace(_) | Msg::Decision(_)
    ) {
        model.inputs += 1;
        // The tick rest is measured from.
        // Pointer motion counts: moving over the sky *is* intent, and a
        // room that stayed empty under a moving cursor would be reading
        // keystrokes rather than attention.
        model.last_input = model.ticks;
    }
    match msg {
        Msg::Resize(size) => {
            model.size = size;
            model.session.turn.set_cell_size(model.cell_size());
            model.session.turn.set_row_cap(model.image_row_cap());
            model.session.turn.rewrap(model.turn_width());
        }
        Msg::FrameDue => {
            model.frames += 1;
            model.roll_activity();
        }
        Msg::Tick => {
            model.ticks += 1;
            birth_settle(model);
            return hold_step(model);
        }
        // A folder from the desktop: wherever the
        // camera was, Birth opens on that ground. A drag hovering
        // opens the ground card with its slot armed; a drag that
        // leaves disarms it and the card stays.
        Msg::Dropped(path) => return birth_drop(model, path),
        Msg::DragHover(on) => {
            if model.surface().is_native() {
                birth_open(model, on);
                model.birth.arm(on);
            }
        }
        // A phase is a *sampling* parameter, not news: it moves where
        // on its curve every transient is read, and changes nothing the
        // motion key watches. Clamped rather than trusted, so a slow
        // frame cannot run the clock past the tick that has not
        // happened yet and skip a stamp's window.
        Msg::Phase(phase) => model.tick_phase = phase.clamp(0.0, 1.0),
        Msg::Focus(focused) => {
            model.focused = focused;
            if !focused {
                model.flight_panels.cancel();
                model.pressed = None;
            }
        }
        Msg::Background(colour) => {
            model.background = Some(colour);
            model.theme = model.theme.observe(colour);
            model.session.turn.set_mode(model.theme.mode());
        }
        Msg::Mode(mode) => {
            let entering_pixels = !model.pixels && mode.has_pixels();
            model.banner = mode.banner();
            model.pixels = mode.has_pixels();
            if entering_pixels && model.uses_flight_recorder() {
                if !model.tab_explicit {
                    model.tab = StageTab::Turn;
                }
                if model.tab == StageTab::Roadmap {
                    model.show(StageTab::Roadmap);
                } else if model.selected.is_none()
                    && let Some(step) = model.trace.steps().last()
                {
                    model.inspect(Selected::Step(step.number));
                    model.pane = Pane::Stage;
                }
            }
            model.pointer_shapes = mode.has_pointer_shapes();
            // The pixel arrangement gives the stage up a gutter on each
            // side, so the turn wraps to the width it actually has —
            // the same rewrap a resize does. Without it a word that fit
            // the wider measure sits in a column the panel no longer
            // shows, and is never painted.
            model.session.turn.rewrap(model.turn_width());
        }
        Msg::TextSizing(sizing) => model.text_sizing = sizing,
        Msg::KeyReleases(reports) => model.key_releases = reports,
        Msg::CellSize(cell) => model.report_cell_size(cell),
        Msg::Key(key) => {
            if key.is_press() {
                model.remember(Msg::Key(key));
                return key_press(model, key);
            }
            // A release is news to one thing only: the key holding a
            // kill. Every other release, and every repeat, is
            // nothing.
            if key.is_release() {
                hold_release(model, key);
            }
        }
        Msg::Mouse { event, target } => {
            let cmds = mouse(model, event, target);
            if !matches!(event.kind, MouseKind::Move | MouseKind::Drag(_)) {
                model.remember(Msg::Mouse { event, target });
            }
            return cmds;
        }
        Msg::PromptCursor(at) => {
            if model.active_panel().is_none() {
                model.session.prompt.set_cursor(at);
                model.session.completion = None;
                model.pane = Pane::Stage;
            }
        }
        Msg::PromptReplace(text) => {
            session::replace_prompt(model, &text);
            model.remember(Msg::PromptReplace(text));
        }
        Msg::Paste(text) => {
            session::paste(model, &text);
            model.remember(Msg::Paste(text));
        }
        Msg::Decision(intent) => return decision::apply(model, intent),
        Msg::RoadmapRequestQueued => {
            if let Some(request) = model
                .roadmap_requests
                .last_mut()
                .filter(|request| request.phase == RequestPhase::Waiting)
            {
                request.phase = RequestPhase::Queued;
                return vec![Cmd::Announce(
                    "Roadmap request queued. It will start after the current work finishes."
                        .to_owned(),
                )];
            }
        }
        Msg::RoadmapRequestStarted => {
            if let Some(request) = model.roadmap_requests.last_mut().filter(|request| {
                matches!(request.phase, RequestPhase::Waiting | RequestPhase::Queued)
            }) {
                model
                    .session
                    .turn
                    .push_text(Voice::User, &format!("{}\n", request.text));
                request.source_start = Some(model.session.turn.lines().len());
                request.phase = RequestPhase::Working;
                model.session.state = TurnState::Streaming;
                model.session.notice = None;
                model.agent_activity.begin("Updating roadmap");
                return vec![Cmd::Announce(
                    "Updating roadmap. Waiting for the agent to respond.".to_owned(),
                )];
            }
        }
        Msg::RoadmapRequestCancelled => {
            let running = model.roadmap_request_running();
            if let Some(request) = model
                .roadmap_requests
                .last_mut()
                .filter(|request| request.phase.is_active())
            {
                request.phase = RequestPhase::Stopped;
                request.source_end = Some(model.session.turn.lines().len());
                if model.session.prompt.is_empty() {
                    model.session.prompt.insert(&request.text);
                }
            }
            if running {
                model.session.state = TurnState::AwaitingInput;
                model
                    .agent_activity
                    .observe(&AgentEvent::TurnEnded(TurnEnd::Cancelled));
            }
        }
        Msg::CommandFailed { cmd, error } => return cmd.failed(model, error),
        Msg::Agent(event) => {
            if matches!(
                event,
                AgentEvent::TurnEnded(_) | AgentEvent::Error(_) | AgentEvent::Initialized { .. }
            ) {
                model.decision.settle();
            }
            model.agent_events += 1;
            model.note_activity();
            return session::agent(model, event);
        }
        Msg::Engine(EngineEvent::Feed(status)) => {
            // The loss is stamped on the loop's clock, so the bad day's
            // connection card can age it without a clock read.
            model.feed_lost_at = match status {
                FeedStatus::Disconnected => model.loop_status.as_ref().map(LoopStatus::elapsed),
                _ => None,
            };
            model.feed = Some(status);
            model.projection.feed(status);
        }
        Msg::Engine(EngineEvent::Event { event, delivery }) => {
            // A frame delivered twice is folded once.
            if !model.projection.delivered(delivery) {
                return Vec::new();
            }
            // A running gate's bytes are kept by the gate's name, not
            // shown: the pane paints them from the store, and nothing
            // else in the cockpit — the rail, the trace, the phase — has
            // a reading of them.
            if let NativeEvent::Magistr(MagistrFrame::GateOutput(output)) = &event {
                model.note_activity();
                let tick = model.ticks;
                model.terminals.push(&output.gate, &output.bytes, tick);
                session::sync_strip(model, &output.gate);
                return Vec::new();
            }
            model.engine_frames += 1;
            model.note_activity();
            model.rail.apply(&event);
            let docked = model.dock.len();
            model.dock.apply(&event);
            model.trace.apply(&event);
            let animate = model.theme.motion().is_full();
            model.roadmap.apply(&event, model.now(), animate);
            session::turn_event(model, &event);
            // The screen-reader channel: each card the event queued and
            // the phase it moved, as plain lines for the runtime to speak.
            let icons = model.theme().icons();
            let mut cmds: Vec<Cmd> = model
                .dock
                .cards()
                .skip(docked)
                .map(|card| Cmd::Announce(format!("needs you: {}", card.text(icons))))
                .collect();
            if let Some(phase) = DisplayPhase::from_event(&event)
                && advance_phase(model, phase)
            {
                cmds.push(Cmd::Announce(format!("phase {phase}")));
            }
            return cmds;
        }
        Msg::Engine(EngineEvent::Seed(snapshot)) => model.seed(&snapshot),
        // The store stands where the last seed left it: the projection
        // is current again and no view is touched — the open detail,
        // the rail and the trace stay exactly as painted.
        Msg::Engine(EngineEvent::Confirmed { .. }) => model.projection.confirmed(),
        Msg::Engine(EngineEvent::Pinned {
            step_number,
            pinned,
        }) => model.trace.set_pinned(step_number, pinned),
        Msg::Engine(EngineEvent::Checkpoint { lines }) => {
            model.changes.close_viewer();
            model.trace.open_viewer(lines);
            model.show(StageTab::Session);
            model.pane = Pane::Inspector;
            model.inspector_open = true;
            model.inspector_scroll = 0;
        }
        Msg::Engine(EngineEvent::PortFailed { cmd, what, error }) => {
            model.dock.restore_answer(&cmd);
            model
                .session
                .turn
                .push_error(&format!("engine {what}: {error}"));
        }
        Msg::Job { id, output } => {
            model.jobs_done += 1;
            if let Some(status) = &mut model.loop_status {
                status.settle_gate(id, !output.is_failed());
            }
            if let JobOutput::Launcher(data) = &output {
                model.launcher = Some((**data).clone());
            }
            if let JobOutput::Galaxy(data) = &output {
                model.galaxy.seed((**data).clone());
            }
            if let JobOutput::Shelf(shelf) = &output {
                model.shelf = Some((**shelf).clone());
            }
            birth_job(model, &output);
            model.last_job = Some((id, output));
            model.last_job_frame = Some(model.frames);
        }
        Msg::Loop(event) => return loop_event(model, event),
        Msg::Workbench(event) => changes::fold(model, event),
        Msg::Checkpoint(event) => checkpoint(model, event),
        Msg::Rehosted(next) => return super::project_switch::rehosted(model, *next),
    }
    Vec::new()
}

/// Fold the checkpoint port's answer: a save lands on the row the card
/// already named, a restore, a fork and a merge leave a line on the
/// stage, and a refusal marks its row rather than erasing it — the card
/// that named the checkpoint is still on screen.
fn checkpoint(model: &mut Model, event: CheckpointEvent) {
    let spine = model.rail.spine_mut();
    match event {
        CheckpointEvent::Saved { id, head, verified } => spine.saved(&id, head, verified),
        CheckpointEvent::Restored {
            id,
            iteration,
            severed,
        } => {
            let mut note = format!("restored {id} · iteration {iteration}");
            if !severed.is_empty() {
                note.push_str(&format!(" · left behind: {}", severed.join(", ")));
            }
            model.session.turn.push_note(&note);
        }
        CheckpointEvent::Forked { id, branch, path } => {
            model
                .session
                .turn
                .push_note(&format!("forked {id} · {branch} at {path}"));
        }
        CheckpointEvent::Merged { id, branch, head } => {
            model
                .session
                .turn
                .push_note(&format!("merged {id} · {branch} at {head}"));
        }
        CheckpointEvent::Failed { id, what, error } => {
            if let Some(id) = &id {
                spine.failed(id, &error);
            }
            model
                .session
                .turn
                .push_error(&format!("checkpoint {what}: {error}"));
        }
    }
}

/// Take `phase` unless the iteration is already past it; once done, the
/// next frame starts over (a new objective on a plain `run`). Says
/// whether the shown phase changed — what the runtime announces.
fn advance_phase(model: &mut Model, phase: DisplayPhase) -> bool {
    let forward = model
        .phase
        .is_none_or(|current| current == DisplayPhase::Done || phase > current);
    let changed = model.phase != Some(phase);
    if forward {
        model.phase = Some(phase);
    }
    forward && changed
}

fn loop_event(model: &mut Model, event: LoopEvent) -> Vec<Cmd> {
    let event = match event {
        LoopEvent::Started { iteration, max } => {
            model.next_iteration();
            // The verdict count is the loop's, not the iteration's.
            let status = LoopStatus::new(iteration, max).carrying(model.loop_status.as_ref());
            model.loop_status = Some(status);
            return Vec::new();
        }
        LoopEvent::Boundary { iteration, gates } => {
            // The cockpit mints the row, as it does for a risky ask, so
            // one counter names every state on the chain; the driver —
            // what survives between iterations — takes the state under
            // that name and answers on the same row.
            let spine = model.rail.spine_mut();
            let id = spine.open_boundary(iteration, gates);
            let label = spine
                .get(&id)
                .map_or_else(String::new, |cp| cp.label().to_owned());
            return vec![Cmd::Checkpoint(CheckpointCmd::Save {
                id,
                reason: CheckpointReason::Iteration,
                label,
            })];
        }
        LoopEvent::Note(text) => {
            model.session.turn.push_note(&text);
            return Vec::new();
        }
        LoopEvent::Failed(text) => {
            model.session.notice = Some("Work failed");
            model.session.turn.push_error(&format!("error: {text}"));
            return Vec::new();
        }
        LoopEvent::Ended(why) => {
            model.session.turn.finish();
            model.agent_activity.active = false;
            model.agent_activity.status = "Stopped";
            if let Some(status) = &mut model.loop_status {
                status.end(why.as_str());
            }
            model.session.state = TurnState::AwaitingInput;
            if model.session.notice != Some("Work failed") {
                model.session.notice = Some("Stopped");
            }
            // A failure is said once: when the reason the work stopped is the
            // error already on the screen, the closing note does not repeat it.
            let shown = format!("error: {why}");
            let repeats = model
                .session
                .turn
                .lines()
                .iter()
                .rev()
                .find(|line| matches!(line.kind(), crate::turn::LineKind::Error))
                .is_some_and(|line| line.text() == shown);
            model.session.turn.push_note(&if repeats {
                "Work stopped.".to_owned()
            } else {
                format!("Work stopped: {why}")
            });
            return Vec::new();
        }
        other => other,
    };
    let Some(status) = &mut model.loop_status else {
        return Vec::new();
    };
    match event {
        LoopEvent::Elapsed(elapsed) => status.set_elapsed(elapsed),
        LoopEvent::Stage(stage) => status.set_stage(stage),
        LoopEvent::GateStarted { name, job } => status.start_gate(name, job),
        LoopEvent::Paused(why) => {
            status.set_paused(true);
            model.session.turn.push_note(&format!("paused: {why}"));
        }
        LoopEvent::Verdict {
            iteration,
            ok,
            impossible,
            reason,
        } => {
            let verdict = LoopVerdict {
                ok,
                impossible,
                reason,
            };
            model
                .session
                .turn
                .push_note(&format!("judge, iteration {iteration}: {}", verdict.text()));
            status.record_verdict(verdict);
        }
        LoopEvent::Started { .. }
        | LoopEvent::Boundary { .. }
        | LoopEvent::Note(_)
        | LoopEvent::Ended(_)
        | LoopEvent::Failed(_) => {}
    }
    Vec::new()
}

fn key_press(model: &mut Model, key: KeyEvent) -> Vec<Cmd> {
    if super::flight_panels::FlightPanels::key(model, key) {
        return Vec::new();
    }
    if let Some(cmd) = session::quit_or_cancel(model, key) {
        return vec![cmd];
    }
    if let Some(cmds) = settings_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = search_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = palette_key(model, key) {
        return cmds;
    }
    // Birth's keys: a field or an answer being typed
    // takes every plain key ahead of the sheet's `?` and the prompt.
    if let Some(cmds) = birth_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = sheet_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = session::completion_key(model, key) {
        return cmds;
    }
    if key.code == KeyCode::Tab {
        // A Moment is modal: the stage behind it is not navigable, and
        // `⇥` is the way into its conditional register.
        if model.moment().is_some() {
            return decision::apply(model, DecisionIntent::OpenCondition);
        }
        // View navigation has one meaning; each surface owns focus geometry.
        if key.mods.contains(Modifiers::CTRL) {
            let tab = if key.mods.contains(Modifiers::SHIFT) {
                model.tab.prev()
            } else {
                model.tab.next()
            };
            model.show(tab);
            model.pane = Pane::Stage;
        } else {
            model.pane = model.pane.cycle(key.mods == Modifiers::SHIFT);
        }
        return Vec::new();
    }
    // Esc on a window is navigation and is
    // folded once, ahead of the dock and the paused loop, so a window's
    // Esc never answers a permission and never aborts a loop; the fold
    // answers `None` on a terminal, whose Esc keeps every meaning it
    // had.
    if key.code == KeyCode::Esc
        && let Some(cmds) = nav::esc(model)
    {
        return cmds;
    }
    // The terminal dock is the topmost layer on either
    // surface: its chords fold ahead of the prompt's Enter, and a
    // terminal's Esc lowers it before anything under it answers.
    if let Some(cmds) = term_dock_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = pause_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = dock_key(model, key) {
        return cmds;
    }
    if model.tab == StageTab::Roadmap
        && key.mods == Modifiers::ALT
        && key.code == KeyCode::Char('m')
    {
        model.roadmap_requests_open = !model.roadmap_requests_open;
        model.pane = Pane::Inspector;
        let label = if model.roadmap_requests_open {
            "Roadmap requests"
        } else {
            "Roadmap item details"
        };
        return vec![Cmd::Announce(label.to_owned())];
    }
    if let Some(cmds) = paused_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = kill_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = copy_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = follow_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = galaxy_key(model, key) {
        return cmds;
    }
    if !model.surface().is_native()
        && model.tab == StageTab::Roadmap
        && model.session.prompt.is_empty()
        && key.mods == Modifiers::ALT
    {
        let handled = match key.code {
            KeyCode::Char('f') => {
                model.roadmap.cycle_filter();
                true
            }
            KeyCode::Char('s') => {
                model.roadmap.sort_by(model.roadmap.sort().next());
                true
            }
            KeyCode::Char('r') => {
                model.roadmap.sort_by(model.roadmap.sort());
                true
            }
            _ => false,
        };
        if handled {
            model.roadmap_viewport = Viewport::at_top();
            model.pane = Pane::Stage;
            return Vec::new();
        }
    }
    if let Some(cmds) = inspector_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = launcher_key(model, key) {
        return cmds;
    }
    if let Some(cmds) = native_trace_key(model, key).or_else(|| native_changes_key(model, key)) {
        return cmds;
    }
    if let Some(cmds) = work::roadmap_key(model, key) {
        return cmds;
    }
    // A terminal's Esc closes the viewer pane on the Changes or Trace
    // tab (a window's took the fold above).
    if key.code == KeyCode::Esc && model.tab == StageTab::Session && model.changes.close_viewer() {
        return Vec::new();
    }
    if key.code == KeyCode::Esc && model.tab == StageTab::Session && model.trace.close_viewer() {
        return Vec::new();
    }
    // The draft's own keys; a key the draft has no meaning for scrolls
    // the stage.
    if let Some(cmds) = session::draft_key(model, key) {
        return cmds;
    }
    scroll_key(model, key.code);
    Vec::new()
}

/// A click on one of the Brief's controls. Every one of them puts the
/// overlay away — a catch-up card is read once — and the three verbs
/// then *dive to the thing*, which is own word for what
/// a card's body does: Answer and View reports frame their star on the
/// constellation the Brief was raised over, Review climbs to the
/// journal.
///
/// The dive is a `focus`, not a second copy of the sky's arithmetic:
/// the id the card carries is the id the constellation draws, so the
/// two cannot point at different chunks — and it goes through the one
/// open-chunk fold, so Answer lands on the stuck chunk's decision and
/// View reports on the shipped chunk's card exactly as ⌘K or a star
/// would land there.
fn brief_click(model: &mut Model, widget: Widget) -> Vec<Cmd> {
    let target = match widget {
        Widget::BriefAnswer => model
            .roadmap
            .rows()
            .into_iter()
            .find(|chunk| StarState::of(chunk.state, false).alerts())
            .map(|chunk| chunk.id.clone()),
        Widget::BriefReports => model.roadmap.shipped().first().cloned(),
        _ => None,
    };
    model.dismiss_brief();
    if widget == Widget::BriefReview {
        model.show_changes();
        return Vec::new();
    }
    if let Some(id) = target {
        open_chunk(model, &id);
        return if widget == Widget::BriefReports {
            work::open_report(model)
        } else {
            work::shelf_collect(model)
        };
    }
    Vec::new()
}

/// A click on one of the bad day's verbs. A
/// fault's card is never dismissed — it leaves when the fact behind it
/// does — so every verb here *does the one thing*: `Install agent…`
/// announces the install line and leaves it on the stage as the
/// receipt; `Details` puts every card's receipt trail on the stage;
/// `Increase limit` opens settings at Budget & limits; `View receipts`
/// climbs to the Changes journal; `Reconnect` asks the runtime to
/// redial its transport.
fn fault_click(model: &mut Model, widget: Widget) -> Vec<Cmd> {
    match widget {
        Widget::FaultInstall => {
            let line = format!("install the agent: {}", BadDay::INSTALL);
            model.session.turn.push_note(&line);
            vec![Cmd::Announce(line)]
        }
        Widget::FaultDetails => {
            let details: Vec<String> = model
                .bad_day()
                .map(|day| {
                    day.faults()
                        .iter()
                        .filter_map(|fault| fault.detail().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            for detail in details {
                model.session.turn.push_note(&detail);
            }
            model.show(StageTab::Turn);
            Vec::new()
        }
        Widget::FaultIncreaseLimit => {
            raise_budget(model);
            Vec::new()
        }
        Widget::FaultReceipts => {
            model.show_changes();
            Vec::new()
        }
        Widget::FaultReconnect => vec![Cmd::Reconnect],
        _ => Vec::new(),
    }
}

/// Open settings at Budget & limits — the one place the breaker is
/// raised, whether the ask came from the bad day's `Increase limit`
/// after it tripped or the meter's `Raise` on the rail before it did.
fn raise_budget(model: &mut Model) {
    let mut settings = Settings::default();
    settings.set_group(SettingsGroup::Budget);
    model.settings = Some(settings);
}

/// A checkpoint row's Restore or Fork, by the row's
/// index on the spine — the same command the terminal's `/restore`
/// and `/fork` send, and nothing for a row the spine does not hold:
/// the port is never asked about a state that was never on the rail.
fn checkpoint_row_verb(model: &Model, index: usize, fork: bool) -> Vec<Cmd> {
    let Some(cp) = model.rail.spine().entries().get(index) else {
        return Vec::new();
    };
    let id = cp.id().to_owned();
    let cmd = if fork {
        CheckpointCmd::Fork { id, branch: None }
    } else {
        CheckpointCmd::Restore { id }
    };
    vec![Cmd::Checkpoint(cmd)]
}

/// A key for the ⌘K palette — the whole
/// keyboard while it is up, the opening chord while it is not. `None`
/// when the palette is closed and the key was not ⌘K.
///
/// ⌘K opens only on a native surface, the Brief's rule: the palette is
/// one of the window's floats, and a terminal never reports `SUPER`.
fn palette_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    let chord = (model.surface().is_native()
        && key.code == KeyCode::Char('k')
        && key.mods == Modifiers::SUPER)
        || (key.code == KeyCode::Char('p')
            && key.mods == Modifiers::CTRL
            && !model.surface().is_native());
    if model.palette.is_none() {
        if chord && model.moment().is_none() {
            model.palette = Some(Palette::default());
            return Some(Vec::new());
        }
        return None;
    }
    if chord || key.code == KeyCode::Esc {
        model.palette = None;
        return Some(Vec::new());
    }
    let rows = Palette::rows(model);
    match key.code {
        KeyCode::Enter => {
            let index = model.palette.as_ref()?.chosen();
            model.palette = None;
            let row = rows.into_iter().nth(index)?;
            return Some(palette_confirm(model, row, key.mods == Modifiers::SUPER));
        }
        code => {
            let palette = model.palette.as_mut()?;
            match code {
                KeyCode::Up => palette.up(),
                KeyCode::Down => palette.down(rows.len()),
                KeyCode::Backspace => palette.backspace(),
                KeyCode::Char(c) if key.mods.is_empty() || key.mods == Modifiers::SHIFT => {
                    palette.type_char(c);
                }
                _ => {}
            }
        }
    }
    Some(Vec::new())
}

/// Confirm a palette row. ⌘↵ (`act`) puts the palette away and leaves
/// the reader exactly where they were — the row's pending act now owns
/// the keys the palette was capturing: a plan-carrying ask is already
/// floating as the Moment, and the dock front's single-letter answers
/// work from any tab. ↵ goes: a place navigates the breadcrumb, a
/// chunk opens through the one open-chunk fold, a binding dispatches
/// its canonical key through the same fold every key the terminal
/// sends takes.
fn palette_confirm(model: &mut Model, row: PaletteRow, act: bool) -> Vec<Cmd> {
    if act {
        return Vec::new();
    }
    match row.go {
        PaletteGo::ResizePanels => {
            super::flight_panels::FlightPanels::begin_keyboard(model);
            Vec::new()
        }
        PaletteGo::ResetPanels => {
            model.flight_panels.reset();
            Vec::new()
        }
        PaletteGo::Settings => {
            model.settings = Some(Settings::default());
            Vec::new()
        }
        PaletteGo::Search => {
            model.search = Some(Search::default());
            Vec::new()
        }
        PaletteGo::Help => {
            model.sheet = true;
            model.sheet_viewport = Viewport::at_top();
            Vec::new()
        }
        PaletteGo::ChooseLoop => choose_loop(model),
        PaletteGo::Launch(action) => launch(model, action),
        PaletteGo::Tab(tab) => {
            model.show(tab);
            Vec::new()
        }
        PaletteGo::Place(Place::Report) => work::open_report(model),
        PaletteGo::Place(place) => {
            model.show_place(place);
            if place == Place::Changes {
                vec![Cmd::Workbench(super::workbench_cmd::WorkbenchCmd::Journal)]
            } else {
                Vec::new()
            }
        }
        PaletteGo::Chunk(id) => {
            open_chunk(model, &id);
            work::shelf_collect(model)
        }
        PaletteGo::Session(index) => resume_session(model, index),
        PaletteGo::Project(index) => {
            model.show_place(Place::Galaxy);
            galaxy_dive(model, index)
        }
        PaletteGo::Key(code, mods) => fold(model, Msg::Key(KeyEvent::new(code).with(mods))),
        // The loop's verbs are the verb, not a keystroke: a row that
        // typed `s` into the prompt would not be a row that steps.
        PaletteGo::Loop(decision) => loop_verb(model, decision).unwrap_or_default(),
        PaletteGo::CheckpointNow => {
            vec![Cmd::Checkpoint(checkpoint_now(model, CHECKPOINT_NOW_LABEL))]
        }
        // The previewed row's Restore / Fork: the verb as the rail's
        // hover reveals it, already naming the checkpoint.
        PaletteGo::Checkpoint(cmd) => vec![Cmd::Checkpoint(cmd)],
        // The offer's Delegate, by the key the row named — the rail's
        // index for it is whatever the rail holds now.
        PaletteGo::Delegate(key) => model
            .rail
            .offers()
            .iter()
            .position(|offer| offer.key() == key)
            .map(|index| delegate_offer(model, index))
            .unwrap_or_default(),
    }
}

/// A key for the ⌘, settings — the whole
/// keyboard while the surface is up, the opening chord while it is
/// not. `None` when settings is closed and the key was not ⌘,.
///
/// Native-only, the palette's rule. Typing filters the sidebar's
/// groups, ↑↓ walk the visible ones, Esc or the chord closes; ↵ is
/// nothing — a pane is already open, there is nothing to confirm.
fn settings_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    let chord = key.code == KeyCode::Char(',') && key.mods == Modifiers::SUPER;
    if model.settings.is_none() {
        if chord && model.surface().is_native() {
            model.settings = Some(Settings::default());
            return Some(Vec::new());
        }
        return None;
    }
    if chord || key.code == KeyCode::Esc {
        model.settings = None;
        return Some(Vec::new());
    }
    let settings = model.settings.as_mut()?;
    match key.code {
        KeyCode::Up => settings.up(),
        KeyCode::Down => settings.down(),
        KeyCode::Backspace => settings.backspace(),
        KeyCode::Char(c) if key.mods.is_empty() || key.mods == Modifiers::SHIFT => {
            settings.type_char(c);
        }
        _ => {}
    }
    Some(Vec::new())
}

/// A click while ⌘, is up: the surface owns the whole window, so the
/// press is always spent here. A nav item opens its pane; a theme
/// card, a switch or a density segment edits the value in the model
/// at once; `Open file` opens the pane's source on the feed the way an
/// edit card's \[Open\] does, and the surface stands down for it;
/// `ESC · CLOSE` closes. Anything else holds — a surface with its own
/// close verb is not put away by a stray click.
fn settings_click(model: &mut Model, target: Option<WidgetId>) -> Vec<Cmd> {
    let Some(target) = target else {
        return Vec::new();
    };
    if let Some(group) = Widget::settings_nav_index(target).and_then(SettingsGroup::at) {
        if let Some(settings) = model.settings.as_mut() {
            settings.set_group(group);
        }
        return Vec::new();
    }
    if let Some(choice) = Widget::settings_theme_index(target).and_then(ThemeChoice::at) {
        model.choose_theme(choice);
        return Vec::new();
    }
    match Widget::settings_switch_index(target) {
        Some(0) => {
            model.settings_data.heatmap = !model.settings_data.heatmap;
            return Vec::new();
        }
        Some(1) => {
            model.settings_data.grain = !model.settings_data.grain;
            return Vec::new();
        }
        // The Motion pane's one switch: reduced motion, the reader's
        // own ask, kept on the theme where the tick reads it.
        Some(2) => {
            let theme = model.theme();
            let motion = if theme.motion().is_full() {
                MotionMode::Reduced
            } else {
                MotionMode::Full
            };
            model.set_theme(theme.with_motion(motion));
            return Vec::new();
        }
        _ => {}
    }
    if let Some(density) = Widget::settings_density_index(target).and_then(Density::at) {
        model.settings_data.density = density;
        return Vec::new();
    }
    match Widget::from_id(target) {
        Some(Widget::SettingsClose) => {
            model.settings = None;
        }
        Some(Widget::SettingsOpenFile) => {
            let path = model
                .settings
                .take()
                .map_or(SettingsGroup::default(), |s| s.group())
                .source()
                .to_owned();
            return vec![Cmd::Workbench(WorkbenchCmd::Open { path })];
        }
        _ => {}
    }
    Vec::new()
}

/// A key for the ⌘⇧F search — the whole
/// keyboard while it is up, the opening chord while it is not. `None`
/// when the search is closed and the key was not ⌘⇧F.
///
/// Native-only, the palette's rule: search is one of the window's
/// floats, and a terminal never reports `SUPER`. The chord arrives as
/// `F` (shift held) from a window and as `f` from a decoder that folds
/// case; both are the chord.
fn search_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    let chord = matches!(key.code, KeyCode::Char('f' | 'F'))
        && key.mods == (Modifiers::SUPER | Modifiers::SHIFT);
    if model.search.is_none() {
        if chord && model.surface().is_native() {
            model.search = Some(Search::default());
            return Some(Vec::new());
        }
        return None;
    }
    if chord || key.code == KeyCode::Esc {
        model.search = None;
        return Some(Vec::new());
    }
    let results = Search::results(model);
    match key.code {
        KeyCode::Enter => {
            let index = model.search.as_ref()?.chosen();
            model.search = None;
            let hit = results.rows.into_iter().nth(index)?;
            return Some(search_confirm(model, hit));
        }
        code => {
            let search = model.search.as_mut()?;
            match code {
                KeyCode::Up => search.up(),
                KeyCode::Down => search.down(results.rows.len()),
                KeyCode::Tab if key.mods == Modifiers::SHIFT => search.prev_scope(),
                KeyCode::Tab => search.next_scope(),
                KeyCode::Backspace => search.backspace(),
                KeyCode::Char(c) if key.mods.is_empty() || key.mods == Modifiers::SHIFT => {
                    search.type_char(c);
                }
                _ => {}
            }
        }
    }
    Some(Vec::new())
}

/// Open a hit at the altitude it lives at: a chunk through the one
/// open-chunk fold, a step on the Trace with that step expanded, a
/// receipt on the Trace where the ship timeline is, a session on the
/// launcher's row — exactly where the palette's ↵ lands the same
/// things.
fn search_confirm(model: &mut Model, hit: SearchHit) -> Vec<Cmd> {
    match hit.go {
        SearchGo::Chunk(id) => {
            open_chunk(model, &id);
            return work::shelf_collect(model);
        }
        SearchGo::Step(number) => nav::open_record(model, Selected::Step(number)),
        SearchGo::Receipt(identity) => nav::open_record(model, Selected::Receipt(identity)),
        SearchGo::Session(index) => return resume_session(model, index),
    }
    Vec::new()
}

/// A key while the accelerator sheet covers the stage: Esc or `?`
/// closes it, the scroll verbs move it, everything else is swallowed —
/// and `?` on an empty prompt opens it from any tab. `None` when the
/// key was not the sheet's to take.
///
/// The registry outgrew a small terminal's stage, so the sheet scrolls.
/// Its own keys are decided *before* the scroll
/// verbs, so Esc still closes the overlay rather than aborting a scroll,
/// and opening it rewinds the window: a help overlay reopened is a fresh
/// question, not the place the last one was left.
fn sheet_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    let plain = key.mods.is_empty() || key.mods == Modifiers::SHIFT;
    if model.sheet {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
            model.sheet = false;
        } else {
            scroll_key(model, key.code);
        }
        return Some(Vec::new());
    }
    if key.code == KeyCode::Char('?') && plain && model.session.prompt.is_empty() {
        model.sheet = true;
        model.sheet_viewport = Viewport::at_top();
        return Some(Vec::new());
    }
    None
}

/// Terminal object navigation. Modal and prompt input has already had priority.
fn inspector_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if model.uses_flight_recorder()
        && model.tab() == StageTab::Turn
        && model.pane() == Pane::Rail
        && key.code == KeyCode::Enter
        && key.mods.is_empty()
    {
        model.reasoning_expanded = !model.reasoning_expanded;
        return Some(Vec::new());
    }
    if (model.surface().is_native() && model.tab() != StageTab::Roadmap)
        || !model.session.prompt.is_empty()
        || !key.mods.is_empty()
    {
        return None;
    }
    if key.code == KeyCode::Esc
        && (model.trace.viewer().is_some() || model.changes.viewer().is_some())
    {
        model.trace.close_viewer();
        model.changes.close_viewer();
        model.inspector_scroll = 0;
        model.inspector_open = false;
        return Some(Vec::new());
    }
    if model.uses_flight_recorder() {
        if key.code == KeyCode::Esc && model.pane() != Pane::Stage && !model.inspector_open {
            model.pane = Pane::Stage;
            return Some(Vec::new());
        }
        if model.pane() == Pane::Rail
            && model.reasoning_expanded()
            && matches!(
                key.code,
                KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
            )
        {
            let count = model.trace.steps().len();
            if count > 0 {
                let index = model
                    .trace
                    .steps()
                    .iter()
                    .position(|s| model.selected() == Some(&Selected::Step(s.number)))
                    .unwrap_or(count - 1);
                let next = match key.code {
                    KeyCode::Home => 0,
                    KeyCode::End => count - 1,
                    KeyCode::Right | KeyCode::Down => (index + 1).min(count - 1),
                    _ => index.saturating_sub(1),
                };
                model.inspect(Selected::Step(model.trace.steps()[next].number));
                model.pane = Pane::Rail;
            }
            return Some(Vec::new());
        }
    }
    match key.code {
        KeyCode::Enter
            if model.selected.is_some()
                && (model.pane == Pane::Inspector
                    || model.pane == Pane::Stage
                        && (matches!(model.tab, StageTab::Roadmap | StageTab::Session)
                            || matches!(model.selected, Some(Selected::Tool(_))))) =>
        {
            model.inspector_open = !model.inspector_open;
            model.inspector_scroll = 0;
        }
        KeyCode::Esc if model.inspector_open => {
            model.inspector_open = false;
            model.inspector_scroll = 0;
        }
        KeyCode::Up | KeyCode::Down if model.pane == Pane::Rail => {}
        KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
            if model.pane == Pane::Inspector =>
        {
            let delta = match key.code {
                KeyCode::Home => isize::MIN,
                KeyCode::End => isize::MAX,
                KeyCode::PageUp => -(model.stage_rows().max(1) as isize),
                _ => model.stage_rows().max(1) as isize,
            };
            detail_scroll(model, delta);
        }
        KeyCode::Up | KeyCode::Down if model.pane == Pane::Inspector => {
            detail_scroll(model, if key.code == KeyCode::Down { 1 } else { -1 });
        }
        KeyCode::Up | KeyCode::Down if model.tab == StageTab::Roadmap => {
            let chunks = model.roadmap.rows();
            let offset = chunks.len().max(1);
            let mut selections: Vec<_> = chunks
                .iter()
                .enumerate()
                .map(|(i, chunk)| (Selected::Chunk(chunk.id.clone()), i))
                .collect();
            selections.extend(
                model
                    .roadmap
                    .signals()
                    .iter()
                    .enumerate()
                    .map(|(i, signal)| (Selected::Signal(signal.id.clone()), offset + i)),
            );
            let current = selections
                .iter()
                .position(|(selected, _)| Some(selected) == model.selected());
            let at = selection_index(current, selections.len(), key.code == KeyCode::Down)?;
            let (selected, row) = selections.swap_remove(at);
            model.inspect(selected);
            let (total, height) = model.roadmap_window();
            model.roadmap_viewport.hold(
                row.saturating_add(1).saturating_sub(height),
                total,
                height,
            );
        }
        KeyCode::Up | KeyCode::Down
            if model.tab == StageTab::Turn && matches!(model.selected, Some(Selected::Tool(_))) =>
        {
            let calls: Vec<_> = model
                .session
                .turn
                .lines()
                .iter()
                .filter_map(|line| {
                    session::tool_identity(line).map(|id| (id.to_owned(), line.row_start()))
                })
                .collect();
            let current = calls
                .iter()
                .position(|(id, _)| model.selected() == Some(&Selected::Tool(id.clone())));
            let index = selection_index(current, calls.len(), key.code == KeyCode::Down)?;
            let (id, row) = &calls[index];
            model.inspect(Selected::Tool(id.clone()));
            let (total, height) = model.conversation_window();
            let position = model.conversation_row_offset(row.saturating_add(1));
            model
                .session
                .viewport
                .hold(position.saturating_sub(height), total, height);
        }
        KeyCode::Up | KeyCode::Down if model.tab == StageTab::Session => {
            let rows = super::session_item::SessionItem::collect(&model.trace, &model.changes);
            let current = rows
                .iter()
                .position(|row| Some(&row.selection()) == model.selected());
            let at = selection_index(current, rows.len(), key.code == KeyCode::Down)?;
            let selected = rows[at].selection();
            model.inspect(selected);
            reveal_session_selection(model);
        }
        _ => return None,
    }
    Some(Vec::new())
}

fn selection_index(current: Option<usize>, len: usize, forward: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match current {
        Some(at) if forward => at.saturating_add(1).min(len - 1),
        Some(at) => at.saturating_sub(1),
        None => 0,
    })
}
/// Native step navigation keeps the desktop trace reading independent of terminal tabs.
fn native_trace_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if model.place() != Some(Place::Trace)
        || !model.session.prompt.is_empty()
        || !key.mods.is_empty()
        || model.trace.viewer().is_some()
        || model.trace.steps().is_empty()
    {
        return None;
    }
    match key.code {
        KeyCode::Up | KeyCode::Down => {
            model.trace.select_step(key.code == KeyCode::Down);
            if let Some(step) = model.trace.selected() {
                model.inspect(Selected::Step(step.number));
            }
            Some(Vec::new())
        }
        KeyCode::Enter if model.trace.selected().is_some() => {
            model.trace.toggle_detail();
            Some(Vec::new())
        }
        _ => None,
    }
}

/// Move through the native journal without changing the conversation or its draft.
fn native_changes_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if model.place() != Some(Place::Changes)
        || !key.mods.is_empty()
        || model.changes.viewer().is_some()
        || !matches!(key.code, KeyCode::Up | KeyCode::Down)
    {
        return None;
    }
    let rows = model.changes.entries();
    let current = rows
        .iter()
        .position(|row| model.selected() == Some(&Selected::Change(row.identity())));
    let at = selection_index(current, rows.len(), key.code == KeyCode::Down)?;
    model.inspect(Selected::Change(rows[at].identity()));
    model.pane = Pane::Stage;
    reveal_session_selection(model);
    Some(Vec::new())
}

/// Scroll the timeline so its selected item remains visible.
pub(super) fn reveal_session_selection(model: &mut Model) {
    let row = if model.place() == Some(Place::Changes) {
        model
            .changes
            .entries()
            .iter()
            .position(|entry| model.selected() == Some(&Selected::Change(entry.identity())))
    } else {
        super::session_item::SessionItem::collect(&model.trace, &model.changes)
            .iter()
            .position(|row| Some(&row.selection()) == model.selected())
    };
    let Some(row) = row else {
        return;
    };
    let (total, height) = model.session_window();
    let (now, animate) = glide(model);
    let top = model.session_top();
    let delta = if row < top {
        row as isize - top as isize
    } else if height > 0 && row >= top + height {
        (row + 1 - height) as isize - top as isize
    } else {
        0
    };
    if delta != 0 {
        model
            .session_viewport
            .scroll_by(delta, total, height, now, animate);
    }
}

/// Project navigation remains available while an unsent message is retained.
/// `↵` opens the selected cluster — the window's own project when nothing
/// is selected — the arrows walk the selection, `+`/`-`/`0` zoom the
/// field as the Sky's keys zoom the tech-tree. `None` anywhere else, so
/// a terminal (which has no Galaxy) and every other altitude keep the
/// keys' meanings.
fn galaxy_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if model.place() != Some(Place::Galaxy) || !key.mods.is_empty() {
        return None;
    }
    let handled = match key.code {
        KeyCode::Enter => {
            let target = model.galaxy.selected().or_else(|| model.galaxy.home())?;
            return Some(galaxy_dive(model, target));
        }
        KeyCode::Right | KeyCode::Down => model.galaxy.select_step(true),
        KeyCode::Left | KeyCode::Up => model.galaxy.select_step(false),
        KeyCode::Char('+' | '=') => model.galaxy.zoom_by(ZOOM_KEY_STEP),
        KeyCode::Char('-') => model.galaxy.zoom_by(1.0 / ZOOM_KEY_STEP),
        KeyCode::Char('0') => model.galaxy.reset_zoom(),
        _ => return None,
    };
    handled.then(Vec::new)
}

/// A dive into the cluster at `index` — `↵` on it, or a click. The
/// window's own project opens: the camera moves to its Sky through the
/// spine, the same move the palette's `Open` makes. Any other project
/// is *selected* and the camera holds: this window is rooted on one
/// project's engines, and a dive that showed this roadmap under another
/// project's name would be a lie. Where the host can admit another
/// project in place the dive asks it to
/// ([`project_switch`](super::project_switch)); the camera still holds
/// until the host answers.
fn galaxy_dive(model: &mut Model, index: usize) -> Vec<Cmd> {
    if !model.galaxy.select(index) {
        return Vec::new();
    }
    if model.galaxy.projects()[index].here {
        model.show(StageTab::Roadmap);
        return Vec::new();
    }
    super::project_switch::open(model, index)
}

/// Host command chords on the terminal, or native launcher accelerators: `n`
/// starts one iteration, `l` the loop, `g` the gates — each recorded
/// as the model's launch ask beside a quit, for the entry point to
/// dispatch — `m` turns the stage to the Roadmap tab, and the arrows
/// walk the sessions card's rows with Enter resuming the selected one.
/// `None` anywhere else; with text in the prompt the letters stay the
/// prompt's.
fn launcher_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if model.launcher().is_some() && key.mods == Modifiers::ALT && model.moment().is_none() {
        return match key.code {
            KeyCode::Char('n') => Some(launch(model, LaunchAction::Next)),
            KeyCode::Char('l') => Some(choose_loop(model)),
            KeyCode::Char('g') => Some(launch(model, LaunchAction::Gates)),
            _ => None,
        };
    }
    if model.hosted_work && key.mods == Modifiers::ALT && model.moment().is_none() {
        return match key.code {
            KeyCode::Char('n') => Some(launch(model, LaunchAction::Next)),
            KeyCode::Char('l') => Some(choose_loop(model)),
            _ => None,
        };
    }
    if !model.is_launcher() || !model.session.prompt.is_empty() || !key.mods.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Char('n') => Some(launch(model, LaunchAction::Next)),
        KeyCode::Char('l') => Some(choose_loop(model)),
        KeyCode::Char('g') => Some(launch(model, LaunchAction::Gates)),
        KeyCode::Char('m') => {
            model.show(StageTab::Roadmap);
            Some(Vec::new())
        }
        KeyCode::Down => select_session(model, true),
        KeyCode::Up => select_session(model, false),
        KeyCode::Enter => model.session_sel.map(|index| resume_session(model, index)),
        _ => None,
    }
}

/// Walk the sessions card's selection one row `down` (or up), opening
/// it at the first (or last) row when nothing is selected yet and
/// stopping at either end. `None` — so the key falls through to the
/// prompt's history — while the card lists no sessions.
fn select_session(model: &mut Model, down: bool) -> Option<Vec<Cmd>> {
    let last = model.launcher()?.sessions.len().checked_sub(1)?;
    model.session_sel = Some(match (model.session_sel, down) {
        (None, true) => 0,
        (None, false) => last,
        (Some(index), true) => index.saturating_add(1).min(last),
        (Some(index), false) => index.saturating_sub(1),
    });
    model.session_note = None;
    Some(Vec::new())
}

/// Resume the session row at `index`: the row's id becomes the
/// launcher's [`LaunchAction::Resume`] ask beside a quit, for the entry
/// point to hand the agent as its `session/load`. A row the entry point
/// could give no id — a session no longer on disk — never quits the
/// cockpit; it says so on the card instead.
fn resume_session(model: &mut Model, index: usize) -> Vec<Cmd> {
    let Some(session) = model
        .launcher()
        .and_then(|data| data.sessions.get(index))
        .cloned()
    else {
        return Vec::new();
    };
    model.session_sel = Some(index);
    match session.id {
        Some(id) => {
            model.session_note = None;
            launch(model, LaunchAction::Resume(id))
        }
        None => {
            model.session_note = Some(format!("{} is no longer resumable", session.when));
            Vec::new()
        }
    }
}

/// Open the track picker before giving a loop request to the host.
fn choose_loop(model: &mut Model) -> Vec<Cmd> {
    if model.loop_is_live() {
        model
            .session
            .turn
            .push_note("Work is already running. Pause or stop it first.");
    } else {
        model.palette = Some(Palette::tracks());
    }
    Vec::new()
}

/// Record the launcher's ask and quit — the entry point reads
/// [`Model::launch`] once the terminal is handed back and starts the
/// asked-for command.
fn launch(model: &mut Model, action: LaunchAction) -> Vec<Cmd> {
    // A hosted session starts work in place: its host takes the command
    // and the session stays. One run at a time — a start while work is
    // live says so and sends nothing.
    if model.hosted_work && matches!(action, LaunchAction::Next | LaunchAction::Loop(_)) {
        if model.loop_is_live() {
            model
                .session
                .turn
                .push_note("work is already running — pause or abort it first");
            return Vec::new();
        }
        return vec![Cmd::Launch(action)];
    }
    model.launch = Some(action);
    model.quit_requested = true;
    vec![Cmd::Quit]
}

/// A loop verb from anywhere — a key, a click on the ledger head, a ⌘K
/// row — through one fold: the strip's paused flag follows the decision
/// at once (the boundary the driver then holds is confirmed back as
/// [`LoopEvent::Paused`] only for a step, which pauses without a second
/// word from the user), the decision leaves its line on the stage, and
/// the driver gets the command. `None` when no live loop can take it.
fn loop_verb(model: &mut Model, decision: LoopCmd) -> Option<Vec<Cmd>> {
    let status = model.loop_status.as_mut().filter(|s| s.ended().is_none())?;
    status.set_paused(decision == LoopCmd::Pause);
    model.session.turn.push_note(decision_note(decision));
    Some(vec![Cmd::Loop(decision)])
}

/// Pause the live loop, or resume it when it holds a boundary. `None`
/// when no loop is live.
fn loop_toggle(model: &mut Model) -> Option<Vec<Cmd>> {
    let paused = model
        .loop_status
        .as_ref()
        .filter(|s| s.ended().is_none())?
        .is_paused();
    loop_verb(
        model,
        if paused {
            LoopCmd::Resume
        } else {
            LoopCmd::Pause
        },
    )
}

/// The label a checkpoint taken by the verb — the ledger head's
/// Checkpoint now, the paused `c`, the ⌘K row — wears when nobody typed
/// one.
const CHECKPOINT_NOW_LABEL: &str = "checkpoint now";

/// Take a checkpoint of the state as it stands, under `label`: the
/// spine mints the name — whoever prints it must be the one that chose
/// it — and the port is asked to save it under that name.
pub(super) fn checkpoint_now(model: &mut Model, label: &str) -> CheckpointCmd {
    let id = model
        .rail
        .spine_mut()
        .open(CheckpointReason::Manual, label.to_owned());
    CheckpointCmd::Save {
        id,
        reason: CheckpointReason::Manual,
        label: label.to_owned(),
    }
}

/// Space on an empty prompt while a loop runs: pause it, or resume it
/// when paused — ahead of the permission strip, so a loop can be paused
/// while a request waits. `⌘.` aborts a live loop from any state on a
/// window. `None` for any other key.
fn pause_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if key.code == Binding::ABORT_CHORD.code && key.mods == Binding::ABORT_CHORD.mods {
        return Some(loop_verb(model, LoopCmd::Abort).unwrap_or_default());
    }
    if key.code != KeyCode::Char(' ') || !key.mods.is_empty() || !model.session.prompt.is_empty() {
        return None;
    }
    loop_toggle(model)
}

/// A key while the loop is paused: `s` steps one iteration, `x` skips
/// the next, `c` takes a checkpoint of the held state, Esc aborts,
/// everything else is swallowed. `None` when the loop is not paused.
fn paused_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    model
        .loop_status
        .as_ref()
        .filter(|s| s.is_paused() && s.ended().is_none())?;
    let decision = match key.code {
        KeyCode::Char('s') if key.mods.is_empty() => LoopCmd::Step,
        KeyCode::Char('x') if key.mods.is_empty() => LoopCmd::Skip,
        KeyCode::Char('c') if key.mods.is_empty() => {
            return Some(vec![Cmd::Checkpoint(checkpoint_now(
                model,
                CHECKPOINT_NOW_LABEL,
            ))]);
        }
        KeyCode::Esc => LoopCmd::Abort,
        _ => return Some(Vec::new()),
    };
    loop_verb(model, decision)
}

/// `f` on an empty prompt while a card's terminal pane is scrolled up:
/// every pane back on its live tail — the jump pill's key. `None` for
/// any other key, and when no pane is scrolled, so the letter is still
/// the prompt's.
/// `k` on an empty prompt starts the held kill on the running card's program — a
/// window always, a terminal only once it reports releases, since a
/// hold nobody can let go of would complete on the timer. `None` with
/// nothing running on a pane, so the letter stays the prompt's; a
/// second press while one is held changes nothing.
fn kill_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if key.code != Binding::KILL.code || !key.mods.is_empty() || !model.session.prompt.is_empty() {
        return None;
    }
    if !model.reports_key_releases() {
        return None;
    }
    let target = model.kill_target()?;
    if model.hold.is_none() {
        model.hold = Some(TermHold::key(target, model.now()));
    }
    Some(Vec::new())
}

/// `y` on an empty prompt puts the running card's stripped transcript
/// on the clipboard — the scrollback and the
/// screen, through [`Cmd::Clipboard`]. `None` without a card that ran
/// on a terminal that wrote anything.
fn copy_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if key.code != Binding::COPY.code || !key.mods.is_empty() || !model.session.prompt.is_empty() {
        return None;
    }
    let id = model.session.turn.running_terminal()?;
    let text = model.terminals.term(id)?.transcript();
    Some(vec![Cmd::Clipboard(text)])
}

/// A press on the turn's absolute row `row`, when it is the head of the
/// card whose program a held kill would reach: the cap holds the kill
/// from this step.
fn arm_cap_hold(model: &mut Model, row: usize) {
    if model.hold.is_some() {
        return;
    }
    let Some(target) = model.kill_target() else {
        return;
    };
    let on_head = model
        .session
        .turn
        .card_at_row(row)
        .is_some_and(|(line, _)| line.record().terminal.as_deref() == Some(target.id()));
    if on_head {
        model.hold = Some(TermHold::cap(target, model.now()));
    }
}

/// The kill key came up: a key hold ends, with nothing done unless the
/// half second had already passed on a tick. A cap
/// hold is the button's to end, never the key's.
fn hold_release(model: &mut Model, key: KeyEvent) {
    if key.code == Binding::KILL.code && model.hold.as_ref().is_some_and(|hold| !hold.is_by_cap()) {
        model.hold = None;
    }
}

/// One tick under a held kill: the hold is dropped silently once its
/// program ended on its own; done, it kills the screen on the model,
/// keeps the strip row for the pane's word, and sends the kill to the
/// producer. Nothing otherwise — the fill is read off the ticks.
fn hold_step(model: &mut Model) -> Vec<Cmd> {
    let Some(hold) = model.hold.as_ref() else {
        return Vec::new();
    };
    if model.kill_target().as_ref() != Some(hold.target()) {
        model.hold = None;
        return Vec::new();
    }
    if !hold.is_done(model.now()) {
        return Vec::new();
    }
    let target = hold.target().clone();
    model.hold = None;
    let id = target.id();
    model.terminals.kill(id);
    model.session.turn.set_terminal_strip(id, true);
    vec![Cmd::Kill(target)]
}

fn follow_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if key.code != KeyCode::Char('f') || !key.mods.is_empty() || !model.session.prompt.is_empty() {
        return None;
    }
    model.terminals.follow_all().then(Vec::new)
}

/// The terminal dock's keys: `⌘↵` raises the running
/// card's screen into the dock or lowers it, `⌘⇧↵` maximizes it (the
/// rail stays) or brings it back to the dragged height — the window's
/// chords, [`Binding::DOCK_CHORD`] and [`Binding::DOCK_MAX_CHORD`];
/// [`Binding::DOCK`]'s `ctrl-t` raises or lowers on either surface. On
/// a terminal, Esc lowers a raised dock ahead of every other meaning
/// (a window's Esc took the nav fold, where the dock lowers before it
/// climbs). `None` for any other key, and for a chord a terminal never
/// reports, so the letter stays the prompt's.
fn term_dock_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    let native = model.surface().is_native();
    let is = |binding: Binding| key.code == binding.code && key.mods == binding.mods;
    if native && is(Binding::DOCK_MAX_CHORD) {
        model.maximize_term_dock();
        return Some(Vec::new());
    }
    if (native && is(Binding::DOCK_CHORD)) || is(Binding::DOCK) {
        if !model.lower_term_dock() {
            model.raise_term_dock();
        }
        return Some(Vec::new());
    }
    if !native && key.code == KeyCode::Esc && model.lower_term_dock() {
        return Some(Vec::new());
    }
    None
}

/// The line a loop decision leaves on the stage.
const fn decision_note(decision: LoopCmd) -> &'static str {
    match decision {
        LoopCmd::Pause => "pausing at the end of this iteration",
        LoopCmd::Resume => "resuming",
        LoopCmd::Step => "stepping one iteration, then pausing",
        LoopCmd::Skip => "skipping the next iteration",
        LoopCmd::Abort => "aborting the loop",
    }
}

/// A key for the dock's front card. A permission is the decision
/// feature's to answer ([`decision::key`]) and takes every key — the
/// agent is blocked on it. A proposal or blocker answers its
/// accelerator only on an empty prompt — the prompt owns its letters —
/// and lets every other key through. `None` when the dock is empty or
/// the key was not the dock's to take.
fn dock_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    decision::key(model, key).or_else(|| card_key(model, key))
}

/// The charter's offer at `index` accepted: the row
/// it becomes is noted on the charter at once, AUTO, and the scope's
/// key goes to whoever owns the policy to write the rule. Nothing for
/// an index the rail does not hold.
fn delegate_offer(model: &mut Model, index: usize) -> Vec<Cmd> {
    let Some(offer) = model.rail.take_offer(index) else {
        return Vec::new();
    };
    model
        .rail
        .note_charter(CharterRule::new(offer.subject(), RuleMode::Auto));
    vec![Cmd::Delegate(offer.key().to_owned())]
}

/// The empty-prompt accelerator of a front proposal (`y` accept, `n`
/// reject) or blocker (`c` clear): the answered card leaves the dock.
fn card_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if !key.mods.is_empty() || !model.session.prompt.is_empty() {
        return None;
    }
    let cmd = match (model.dock.front()?, key.code) {
        (DockItem::Proposal { chunk, kind, .. }, KeyCode::Char(c @ ('y' | 'n'))) => {
            Cmd::Engine(EngineCmd::ResolveProposal {
                chunk: chunk.clone(),
                kind: *kind,
                accept: c == 'y',
            })
        }
        (DockItem::Blocker { chunk, .. }, KeyCode::Char('c')) => {
            Cmd::Engine(EngineCmd::ClearBlocker {
                chunk: chunk.clone(),
            })
        }
        _ => return None,
    };
    if let Cmd::Engine(answer) = &cmd {
        model.dock.answer_front(answer);
    }
    Some(vec![cmd])
}

fn scroll_key(model: &mut Model, code: KeyCode) {
    let (total, height) = scroll_extent(model);
    let (now, animate) = glide(model);
    if model.uses_flight_recorder() && !model.is_sheet_open() && scrolled_tab(model).is_none() {
        let delta = match code {
            KeyCode::PageUp => -(height.saturating_sub(1).max(1) as isize),
            KeyCode::PageDown => height.saturating_sub(1).max(1) as isize,
            KeyCode::Home => isize::MIN,
            KeyCode::End => isize::MAX,
            _ => return,
        };
        let origin = model.conversation_scroll_origin();
        model
            .session
            .viewport
            .scroll_from(origin, delta, total, height, (now, animate));
        return;
    }
    let viewport = scrolled_viewport(model);
    match code {
        KeyCode::PageUp => viewport.page_up(total, height, now, animate),
        KeyCode::PageDown => viewport.page_down(total, height, now, animate),
        KeyCode::Home => viewport.home(total, height, now, animate),
        KeyCode::End => viewport.end(total, height, now, animate),
        _ => {}
    }
}

/// The animation step a scroll lands on, and whether its glide runs at
/// all — read before the viewport is borrowed, since the borrow takes
/// the whole model with it.
fn glide(model: &Model) -> (f32, bool) {
    (model.now(), model.theme.motion().is_full())
}

/// What the stage scrolls right now: the accelerator sheet while it
/// covers everything, else the Session and Roadmap tabs move
/// their own bodies under their pinned headings and every other tab
/// moves the turn.
fn scroll_extent(model: &Model) -> (usize, usize) {
    if model.is_sheet_open() {
        return model.sheet_window();
    }
    match scrolled_tab(model) {
        Some(StageTab::Session) => model.session_window(),
        Some(StageTab::Roadmap) => model.roadmap_window(),
        _ => model.conversation_window(),
    }
}

/// The viewport [`scroll_extent`] measured.
fn scrolled_viewport(model: &mut Model) -> &mut Viewport {
    if model.is_sheet_open() {
        return &mut model.sheet_viewport;
    }
    match scrolled_tab(model) {
        Some(StageTab::Session) => &mut model.session_viewport,
        Some(StageTab::Roadmap) => &mut model.roadmap_viewport,
        _ => &mut model.session.viewport,
    }
}

/// The tab whose list scrolls. The accelerator sheet takes precedence.
const fn scrolled_tab(model: &Model) -> Option<StageTab> {
    if model.is_sheet_open() {
        return None;
    }
    match model.shown_tab() {
        StageTab::Session => Some(StageTab::Session),
        StageTab::Roadmap => Some(StageTab::Roadmap),
        _ => None,
    }
}

/// The tab a mouse event landed on in the undocked third column, or
/// `None` when it landed anywhere else — through the rect the runtime
/// reported after layout, `None` too on a screen too narrow to undock
/// anything.
///
/// The wheel is the only way to move that column: the undocked tab is by
/// construction never the active one, so the scroll keys — which follow
/// the active tab — cannot reach it. Routing by the pointer rather than
/// by the focus is the rule every terminal multiplexer settled on.
///
/// Answered ahead of the accelerator sheet on purpose: the sheet covers
/// the stage and not this column, so a notch over rows still on screen
/// moves those rows.
fn wheeled_undock(model: &Model, event: &MouseEvent) -> Option<StageTab> {
    let rect = model.undock_rect()?;
    let tab = model.undocked_tab()?;
    rect.contains(event.col, event.row).then_some(tab)
}

/// What one wheel notch moves, given the column it landed in: the
/// undocked tab's own body there, [`scroll_extent`]'s answer anywhere
/// else. The undocked column's window *is* the stage's window for the
/// tab it holds — its padding and heading come to exactly the panel ring
/// and tab bar the stage wears, so the two cancel (pinned by
/// `the_undock_column_windows_by_the_same_rows_as_the_stage`).
fn wheel_extent(model: &Model, undocked: Option<StageTab>) -> (usize, usize) {
    match undocked {
        Some(_) => model.session_window(),
        None => scroll_extent(model),
    }
}

/// The viewport [`wheel_extent`] measured. The undocked column moves the
/// *tab's* own viewport rather than one minted for the column: one
/// viewport per surface, not per placement, so a feed scrolled beside
/// the stage is still where it was left when its tab comes back on
/// stage. Session uses the same viewport in both placements.
fn wheeled_viewport(model: &mut Model, undocked: Option<StageTab>) -> &mut Viewport {
    match undocked {
        Some(_) => &mut model.session_viewport,
        None => scrolled_viewport(model),
    }
}

/// The terminal whose pane row the pointer is over, when the stage is
/// showing the turn and `target` is one of its rows inside an open
/// card's pane — the row the wheel moves inside rather than past.
fn pane_under(
    model: &Model,
    target: Option<WidgetId>,
    undocked: Option<StageTab>,
) -> Option<String> {
    // The dock's screen is its own target, whatever the stage shows:
    // a notch over it moves the docked pane.
    if target == Some(Widget::TermDock.id()) {
        return model.term_dock.id().map(str::to_owned);
    }
    if undocked.is_some()
        || (!model.uses_flight_recorder() && model.shown_tab() != StageTab::Turn)
        || model.is_sheet_open()
    {
        return None;
    }
    let index = target.and_then(Widget::stage_row_index)?;
    let (id, _) = model
        .session
        .turn
        .pane_at_row(model.top_row().checked_add(index)?)?;
    Some(id.to_owned())
}

fn mouse(model: &mut Model, event: MouseEvent, target: Option<WidgetId>) -> Vec<Cmd> {
    if super::flight_panels::FlightPanels::mouse(model, event, target) {
        return Vec::new();
    }
    if model.uses_flight_recorder() && model.is_sheet_open() {
        if matches!(event.kind, MouseKind::ScrollUp | MouseKind::ScrollDown) {
            let (total, height) = model.sheet_window();
            let (now, animate) = glide(model);
            if event.kind == MouseKind::ScrollDown {
                model.sheet_viewport.wheel_down(total, height, now, animate);
            } else {
                model.sheet_viewport.wheel_up(total, height, now, animate);
            }
        }
        return Vec::new();
    }
    if model.uses_flight_recorder()
        && matches!(event.kind, MouseKind::ScrollUp | MouseKind::ScrollDown)
    {
        let down = event.kind == MouseKind::ScrollDown;
        if model.settings.is_some() {
            return Vec::new();
        }
        if model.search.is_some() {
            let count = Search::results(model).rows.len();
            if let Some(search) = model.search.as_mut() {
                if down {
                    search.down(count);
                } else {
                    search.up();
                }
            }
            return Vec::new();
        }
        if model.palette.is_some() {
            let count = Palette::rows(model).len();
            if let Some(palette) = model.palette.as_mut() {
                if down {
                    palette.down(count);
                } else {
                    palette.up();
                }
            }
            return Vec::new();
        }
    }
    if model.uses_flight_recorder() && model.moment().is_some() {
        match event.kind {
            MouseKind::ScrollUp => {
                return decision::apply(model, DecisionIntent::Scroll(-(Viewport::WHEEL as isize)));
            }
            MouseKind::ScrollDown => {
                return decision::apply(model, DecisionIntent::Scroll(Viewport::WHEEL as isize));
            }
            MouseKind::Down(Button::Left) => {
                model.pressed = target;
                // A button answers; a click anywhere else on the card
                // is the way into its conditional register.
                return decision::click(model, target)
                    .unwrap_or_else(|| decision::apply(model, DecisionIntent::OpenCondition));
            }
            _ => {}
        }
    }
    if model.uses_flight_recorder()
        && matches!(event.kind, MouseKind::ScrollUp | MouseKind::ScrollDown)
    {
        let down = event.kind == MouseKind::ScrollDown;
        let conversation = target == Some(Widget::Stage.id())
            || target.and_then(Widget::stage_row_index).is_some();
        if conversation && pane_under(model, target, None).is_none() {
            let (total, height) = model.conversation_window();
            let (now, animate) = glide(model);
            let distance = model.conversation_wheel();
            let origin = model.conversation_scroll_origin();
            model.session.viewport.scroll_from(
                origin,
                if down { distance } else { -distance },
                total,
                height,
                (now, animate),
            );
            return Vec::new();
        }
        if target == Some(Widget::Dock.id()) {
            // The strip's own scroll: a row at a time, unclamped — the
            // strip measures no card end the way the Moment does.
            model
                .decision
                .scroll_by(if down { 1 } else { -1 }, usize::MAX);
            return Vec::new();
        }
        let list = (model.tab() == StageTab::Roadmap
            && (target == Some(Widget::RoadmapTable.id())
                || target.and_then(Widget::roadmap_row_index).is_some()
                || target.and_then(Widget::signal_row_index).is_some()))
            || (model.pane() == Pane::Stage
                && model.tab() == StageTab::Session
                && (target.is_some_and(Inspector::contains_target)
                    || target.and_then(Widget::session_row_index).is_some()
                    || target.and_then(Widget::change_row_index).is_some()));
        if list {
            let (total, height) = scroll_extent(model);
            let (now, animate) = glide(model);
            let viewport = scrolled_viewport(model);
            if down {
                viewport.wheel_down(total, height, now, animate);
            } else {
                viewport.wheel_up(total, height, now, animate);
            }
            return Vec::new();
        }
    }
    if (!model.surface().is_native()
        || model.flight_panels.native_details.is_some()
        || model.flight_panels.native_requests.is_some())
        && target.is_some_and(|id| Inspector::contains_target(id) || id == Widget::RequestBody.id())
    {
        if matches!(event.kind, MouseKind::ScrollUp | MouseKind::ScrollDown) {
            detail_scroll(
                model,
                if event.kind == MouseKind::ScrollDown {
                    Viewport::WHEEL as isize
                } else {
                    -(Viewport::WHEEL as isize)
                },
            );
            return Vec::new();
        }
        if event.kind == MouseKind::Down(Button::Left) {
            model.pane = Pane::Inspector;
        }
    }
    // Which column the notch landed in, before anything borrows the
    // model: the undocked third one moves its own rows, everything else
    // moves what the stage is showing.
    let undocked = wheeled_undock(model, &event);
    let (total, height) = wheel_extent(model, undocked);
    let pane = match event.kind {
        MouseKind::ScrollUp | MouseKind::ScrollDown => pane_under(model, target, undocked),
        _ => None,
    };
    let (now, animate) = glide(model);
    // The held widget, before any arm returns early: a press is state
    // about the *gesture*, so it is taken and released here rather than
    // inside whichever of the twenty arms happens to handle the target.
    match event.kind {
        MouseKind::Down(Button::Left) => model.pressed = target,
        MouseKind::Up(_) => model.pressed = None,
        _ => {}
    }
    match event.kind {
        // A notch over the tech-tree zooms it about the pointer instead
        // of scrolling the list under it; in cell-only mode the runtime
        // reports no rect, so the wheel scrolls there as everywhere. The
        // tree sits inside the stage, so it can never be the column
        // `undocked` named.
        // A notch over the Galaxy's field zooms the field — the field is its own widget, so the
        // wheel needs no reported rect to find it.
        MouseKind::ScrollUp if target == Some(Widget::GalaxyField.id()) => {
            model.galaxy.zoom_by(ZOOM_NOTCH);
        }
        MouseKind::ScrollDown if target == Some(Widget::GalaxyField.id()) => {
            model.galaxy.zoom_by(1.0 / ZOOM_NOTCH);
        }
        // A notch over an open card's terminal pane moves the pane
        // inside the card, never the stage under it: up stops it
        // following, back down to its tail resumes.
        MouseKind::ScrollUp if pane.is_some() => {
            if let Some(term) = pane.as_deref().and_then(|id| model.terminals.term_mut(id)) {
                term.scroll_up(Viewport::WHEEL);
            }
        }
        MouseKind::ScrollDown if pane.is_some() => {
            if let Some(term) = pane.as_deref().and_then(|id| model.terminals.term_mut(id)) {
                term.scroll_down(Viewport::WHEEL);
            }
        }
        MouseKind::ScrollUp if !work::graph_wheel(model, &event, true) => {
            wheeled_viewport(model, undocked).wheel_up(total, height, now, animate);
        }
        MouseKind::ScrollDown if !work::graph_wheel(model, &event, false) => {
            wheeled_viewport(model, undocked).wheel_down(total, height, now, animate);
        }
        MouseKind::Down(Button::Left) => {
            // Settings owns the window while it is up (view 10): every
            // press lands on the surface, never on the room behind.
            if model.settings.is_some() {
                return settings_click(model, target);
            }
            // Search is an overlay too: a scope chip re-scopes the same
            // query and keeps the float, a row opens the hit, a click
            // anywhere else puts it away and spends the press.
            if model.search.is_some() {
                if target == Some(Widget::OverlayQuery.id()) {
                    return Vec::new();
                }
                if let Some(scope) = target
                    .and_then(Widget::search_scope_index)
                    .and_then(SearchScope::at)
                {
                    if let Some(search) = model.search.as_mut() {
                        search.set_scope(scope);
                    }
                    return Vec::new();
                }
                let index = target.and_then(Widget::search_row_index);
                let results = Search::results(model);
                model.search = None;
                if let Some(hit) = index.and_then(|i| results.rows.into_iter().nth(i)) {
                    return search_confirm(model, hit);
                }
                return Vec::new();
            }
            // The palette is an overlay: a click on one of its rows
            // confirms that row, a click anywhere else puts it away
            // and spends the press — the room behind a float is not
            // clickable through it.
            if model.palette.is_some() {
                if target == Some(Widget::OverlayQuery.id()) {
                    return Vec::new();
                }
                let index = target.and_then(Widget::palette_row_index);
                let rows = Palette::rows(model);
                model.palette = None;
                if let Some(row) = index.and_then(|i| rows.into_iter().nth(i)) {
                    return palette_confirm(model, row, false);
                }
                return Vec::new();
            }
            if let Some(binding) = target
                .and_then(Widget::footer_chip_index)
                .and_then(|index| super::view::hint_context(model).get(index).copied())
            {
                return fold(
                    model,
                    Msg::Key(KeyEvent::new(binding.code).with(binding.mods)),
                );
            }
            match target.and_then(Widget::from_id) {
                Some(Widget::ReasoningToggle) => {
                    model.reasoning_expanded = !model.reasoning_expanded;
                    model.pane = Pane::Rail;
                    return Vec::new();
                }
                Some(Widget::LatestDecision) => {
                    if let Some(step) = model.trace.steps().last() {
                        model.inspect(Selected::Step(step.number));
                    }
                    return Vec::new();
                }
                Some(Widget::TracePrevious | Widget::TraceNext) => {
                    let next = target == Some(Widget::TraceNext.id());
                    let current =
                        model.trace.steps().iter().position(|step| {
                            model.selected() == Some(&Selected::Step(step.number))
                        });
                    let count = model.trace.steps().len();
                    if count > 0 {
                        let index = current.map_or(count - 1, |i| {
                            if next {
                                (i + 1).min(count - 1)
                            } else {
                                i.saturating_sub(1)
                            }
                        });
                        model.inspect(Selected::Step(model.trace.steps()[index].number));
                    }
                    return Vec::new();
                }
                Some(Widget::LinkedWork) => {
                    if let Some(Selected::Step(number)) = model.selected()
                        && let Some((_, selected)) = Inspector::linked_work(model, *number)
                    {
                        nav::open_record(model, selected);
                    }
                    return Vec::new();
                }
                Some(Widget::Commands) => {
                    model.palette = Some(Palette::default());
                    return Vec::new();
                }
                Some(Widget::PromptFiles) if model.moment().is_none() => {
                    super::FlightComposer::add_files(model);
                    return Vec::new();
                }
                Some(Widget::Prompt) => {
                    model.pane = Pane::Stage;
                    return Vec::new();
                }
                Some(Widget::PromptAction) if model.moment().is_none() => {
                    if (model.tab != StageTab::Roadmap || model.roadmap_request_pending())
                        && let Some(command) = session::composer_stop(model)
                    {
                        return vec![command];
                    }
                    if !model.session.prompt.text().trim().is_empty() {
                        model.session.completion = None;
                        return session::send(model);
                    }
                    return Vec::new();
                }
                Some(Widget::RailToggle) => {
                    model.rail.toggle();
                    model.session.turn.rewrap(model.turn_width());
                    return Vec::new();
                }
                Some(Widget::RailDivider) => {
                    model.rail.begin_drag();
                    return Vec::new();
                }
                // The dock's grip: the press remembers
                // where it began, and the drags that follow size the
                // dock by the rows the pointer moves.
                Some(Widget::TermDockGrip) => {
                    let (_, y) = model.pointer_at(&event);
                    model.term_dock.begin_drag(y);
                    return Vec::new();
                }
                Some(brief) if Widget::BRIEF.contains(&brief) => return brief_click(model, brief),
                Some(fault) if Widget::FAULTS.contains(&fault) => return fault_click(model, fault),
                // The ledger head's verbs take the
                // same fold the keys and the ⌘K rows take.
                Some(Widget::LoopPause) => return loop_toggle(model).unwrap_or_default(),
                Some(Widget::LoopStep) => {
                    return loop_verb(model, LoopCmd::Step).unwrap_or_default();
                }
                Some(Widget::LoopCheckpoint) => {
                    return vec![Cmd::Checkpoint(checkpoint_now(model, CHECKPOINT_NOW_LABEL))];
                }
                Some(Widget::LoopAbort) => {
                    return loop_verb(model, LoopCmd::Abort).unwrap_or_default();
                }
                // The meter's Raise: the bad day's
                // `Increase limit`, before the breaker trips.
                Some(Widget::BudgetRaise) => {
                    raise_budget(model);
                    return Vec::new();
                }
                // The identity band's chunk chip: a navigation chip rises to the chunk's star.
                Some(Widget::SessionChunk) => {
                    work::session_chunk_click(model);
                    return Vec::new();
                }
                Some(Widget::Accept) => return work::proposal_click(model, true),
                Some(Widget::Reject) => return work::proposal_click(model, false),
                Some(Widget::Clear) => return work::blocker_click(model),
                Some(Widget::Checkpoint) => return vec![Cmd::Engine(EngineCmd::Checkpoint)],
                Some(Widget::ToolOpen) => return changes::selected_edit_click(model, false),
                Some(Widget::ToolUndo) => return changes::selected_edit_click(model, true),
                Some(Widget::InspectorDetail) => {
                    if model.trace.viewer().is_some() || model.changes.viewer().is_some() {
                        model.trace.close_viewer();
                        model.changes.close_viewer();
                        model.inspector_open = false;
                    } else {
                        model.inspector_open = !model.inspector_open;
                    }
                    model.inspector_scroll = 0;
                    model.pane = Pane::Inspector;
                    return Vec::new();
                }
                Some(
                    Widget::Inspector
                    | Widget::InspectorTitle
                    | Widget::InspectorStatus
                    | Widget::InspectorMetadata
                    | Widget::InspectorFields
                    | Widget::InspectorActions,
                ) => {
                    model.pane = Pane::Inspector;
                    return Vec::new();
                }
                Some(Widget::Rail) => {
                    model.pane = Pane::Rail;
                    return Vec::new();
                }
                Some(Widget::Stage) => {
                    model.pane = Pane::Stage;
                    return Vec::new();
                }
                Some(Widget::TraceDetail) => {
                    if model.surface().is_native() {
                        model.trace.toggle_detail();
                    } else {
                        if model.trace.viewer().is_some() || model.changes.viewer().is_some() {
                            model.trace.close_viewer();
                            model.changes.close_viewer();
                            model.inspector_open = false;
                        } else {
                            model.inspector_open = !model.inspector_open;
                        }
                        model.inspector_scroll = 0;
                    }
                    return Vec::new();
                }
                Some(Widget::RoadmapFilter) => {
                    work::filter_click(model);
                    return Vec::new();
                }
                Some(widget) if work::sort_column(widget).is_some() => {
                    if let Some(column) = work::sort_column(widget) {
                        work::sort_click(model, column);
                    }
                    return Vec::new();
                }
                Some(Widget::RoadmapReset) => {
                    work::reset_click(model);
                    return Vec::new();
                }
                Some(Widget::RequestLatest) => {
                    model.roadmap_request_scroll = usize::MAX;
                    model.pane = Pane::Inspector;
                    return Vec::new();
                }
                Some(Widget::RoadmapRequests) => {
                    model.roadmap_requests_open = true;
                    model.pane = Pane::Inspector;
                    return Vec::new();
                }
                Some(Widget::RoadmapDetails) => {
                    model.roadmap_requests_open = false;
                    model.pane = Pane::Inspector;
                    return Vec::new();
                }
                Some(Widget::RoadmapRequest) => {
                    model.roadmap_requests_open = true;
                    super::FlightComposer::request_change(model);
                    return Vec::new();
                }
                Some(Widget::RoadmapStart) => return work::roadmap_chunk_click(model, false),
                Some(Widget::RoadmapComplete) => return work::roadmap_chunk_click(model, true),
                Some(Widget::ShelfExport) => return shelf_export(model),
                Some(Widget::ShelfShare) => return shelf_share(model),
                Some(
                    receipt @ (Widget::ReportCommit | Widget::ReportGates | Widget::ReportCost),
                ) => {
                    return report_receipt(model, receipt);
                }
                Some(Widget::ReportTrace) => return report_trace(model),
                Some(Widget::LaunchNext) => return launch(model, LaunchAction::Next),
                Some(Widget::LaunchLoop) => return choose_loop(model),
                Some(Widget::LaunchGates) => return launch(model, LaunchAction::Gates),
                // `＋ New project` is Birth F1's door:
                // the camera moves to the ground.
                Some(Widget::GalaxyNew) => {
                    birth_open(model, false);
                    return Vec::new();
                }
                Some(birth) if Widget::BIRTH.contains(&birth) => {
                    return birth_click(model, birth);
                }
                Some(Widget::RoadmapGraph) => {
                    work::graph_click(model, &event);
                    return Vec::new();
                }
                Some(widget) => {
                    // A crumb is the altitude path,
                    // laid out on a window only: it moves the camera.
                    if let Some(place) = widget.place_target(model.place) {
                        model.show_place(place);
                        return if place == Place::Changes {
                            vec![Cmd::Workbench(super::workbench_cmd::WorkbenchCmd::Journal)]
                        } else {
                            Vec::new()
                        };
                    }
                    if let Some(tab) = widget.tab_target() {
                        if tab == StageTab::Roadmap && model.tab == StageTab::Roadmap {
                            model.roadmap_requests_open = false;
                        }
                        model.show(tab);
                        model.pane = Pane::Stage;
                        return Vec::new();
                    }
                }
                None => {}
            }
            if let Some(index) = target.and_then(Widget::completion_index) {
                if let Some(completion) = &mut model.session.completion {
                    completion.select(index);
                }
                session::accept_completion(model);
                return Vec::new();
            }
            if let Some(index) = target.and_then(Widget::edit_open_index) {
                return changes::edit_click(model, index, false);
            }
            if let Some(index) = target.and_then(Widget::edit_undo_index) {
                return changes::edit_click(model, index, true);
            }
            // Preview: the row's own verb, always drawn. On a window it opens the Changes reading at that
            // checkpoint — the camera turns, `review_since` reads the
            // previewed row — and folding the preview back holds the
            // camera; a terminal's rail previews and stays.
            if let Some(index) = target.and_then(Widget::spine_row_index) {
                let spine = model.rail.spine_mut();
                spine.preview(index);
                if spine.selected() == Some(index) && model.surface().is_native() {
                    model.show_changes();
                }
                return Vec::new();
            }
            // Restore and Fork: the verbs the window reveals on a row
            // under the pointer (a terminal has `/restore` and
            // `/fork`); a row is one click from either only while the
            // pointer already rests on it.
            if let Some(index) = target.and_then(Widget::spine_restore_index) {
                return checkpoint_row_verb(model, index, false);
            }
            if let Some(index) = target.and_then(Widget::spine_fork_index) {
                return checkpoint_row_verb(model, index, true);
            }
            // Delegate: the charter offer's verb.
            if let Some(index) = target.and_then(Widget::charter_offer_index) {
                return delegate_offer(model, index);
            }
            if let Some(index) = target.and_then(Widget::change_row_index) {
                changes::row_click(model, index);
                return Vec::new();
            }
            if let Some(index) = target.and_then(Widget::change_open_index) {
                return changes::open_click(model, index);
            }
            if let Some(index) = target.and_then(Widget::change_undo_index) {
                return changes::undo_click(model, index);
            }
            if let Some(index) = target.and_then(Widget::change_approve_index) {
                return changes::approve_click(model, index);
            }
            if let Some(index) = target.and_then(Widget::session_row_index) {
                let selected =
                    super::session_item::SessionItem::collect(&model.trace, &model.changes)
                        .get(index)
                        .map(|row| row.selection());
                if let Some(selected) = selected {
                    model.inspect(selected);
                    if !model.uses_flight_recorder() {
                        model.pane = Pane::Stage;
                    }
                    reveal_session_selection(model);
                }
                return Vec::new();
            }
            if let Some(index) = target.and_then(Widget::trace_row_index) {
                if let Some(step) = model.trace.steps().get(index) {
                    let selected = Selected::Step(step.number);
                    model.inspect(selected);
                    if !model.uses_flight_recorder() {
                        model.pane = Pane::Stage;
                    }
                    reveal_session_selection(model);
                }
                return Vec::new();
            }
            if let Some(number) = target.and_then(Widget::trace_pin_step) {
                return pin_click(model, number);
            }
            if let Some(number) = target.and_then(Widget::trace_link_step) {
                model.trace.expand_step(number);
                model.inspect(Selected::Step(number));
                reveal_session_selection(model);
                return Vec::new();
            }
            if let Some(number) = target.and_then(Widget::trace_chunk_step) {
                return work::chunk_jump(model, number);
            }
            if let Some(index) = target.and_then(Widget::roadmap_dependency_index) {
                if let Some(selected) = Inspector::roadmap_dependency(model, index) {
                    if matches!(selected, Selected::Chunk(_)) {
                        model.inspect(selected);
                    } else {
                        nav::open_record(model, selected);
                    }
                }
                return Vec::new();
            }
            if let Some(index) = target.and_then(Widget::sky_star_index) {
                return work::sky_star_click(model, index);
            }
            if let Some(index) = target.and_then(Widget::galaxy_cluster_index) {
                return galaxy_dive(model, index);
            }
            // An interview chip is a pick; a proposal row is grabbed —
            // the drag ends on the row the button comes up over.
            if let Some(index) = target.and_then(Widget::birth_chip_index) {
                model.birth.toggle_chip(index);
                return Vec::new();
            }
            if let Some(index) = target.and_then(Widget::birth_row_index) {
                model.birth.drag_from(index);
                return Vec::new();
            }
            if let Some(index) = target.and_then(Widget::roadmap_row_index) {
                work::row_click(model, index);
                return Vec::new();
            }
            if let Some(index) = target.and_then(Widget::signal_row_index) {
                if let Some(signal) = model.roadmap.signals().get(index) {
                    let selected = Selected::Signal(signal.id.clone());
                    model.inspect(selected);
                    if !model.uses_flight_recorder() {
                        model.pane = Pane::Stage;
                    }
                }
                return Vec::new();
            }
            if let Some(index) = target.and_then(Widget::proposal_accept_index) {
                return work::roadmap_proposal_click(model, index, true);
            }
            if let Some(index) = target.and_then(Widget::proposal_reject_index) {
                return work::roadmap_proposal_click(model, index, false);
            }
            if let Some(index) = target.and_then(Widget::signal_promote_index) {
                return signal_click(model, index, SignalAct::Promote);
            }
            if let Some(index) = target.and_then(Widget::signal_ignore_index) {
                return signal_click(model, index, SignalAct::Ignore);
            }
            if let Some(index) = target.and_then(Widget::signal_snooze_index) {
                return signal_click(model, index, SignalAct::Snooze);
            }
            if let Some(index) = target.and_then(Widget::signal_research_index) {
                return signal_click(model, index, SignalAct::Research);
            }
            if let Some(index) = target.and_then(Widget::launch_session_index) {
                return resume_session(model, index);
            }
            if let Some(index) = target.and_then(Widget::review_file_index) {
                model.select_review_file(index);
                return Vec::new();
            }
            if let Some(index) = target.and_then(Widget::stage_row_index) {
                let row = model.top_row().saturating_add(index);
                // The cap is the running card's head: the press that
                // folds the card also starts the held kill, which a click's release cancels inside the
                // dead zone before it shows.
                arm_cap_hold(model, row);
                session::select_row(model, row);
                return Vec::new();
            }
            // An answer button or *Show images*, on the strip or the
            // Moment's card: the decision feature's click, so a click
            // and the key that presses the same button agree about the
            // condition in the register.
            return decision::click(model, target).unwrap_or_default();
        }
        // The divider follows the pointer's column, whichever side the
        // rail is on; the width it asks for is the surface's arithmetic.
        MouseKind::Drag(Button::Left) if model.rail.is_dragging() => {
            let (x, _) = model.pointer_at(&event);
            let width = model.rail_drag_width(x);
            model.rail.drag_width(width, model.size.cols);
            model.session.turn.rewrap(model.turn_width());
        }
        // The cap let go before the half second: nothing happens.
        MouseKind::Up(Button::Left) if model.hold.as_ref().is_some_and(TermHold::is_by_cap) => {
            model.hold = None;
        }
        MouseKind::Up(Button::Left) if model.rail.is_dragging() => model.rail.end_drag(),
        // The dock's grip follows the pointer's row; the release is
        // the height this project remembers.
        MouseKind::Drag(Button::Left) if model.term_dock.is_dragging() => {
            let (_, y) = model.pointer_at(&event);
            if model.term_dock.drag_to(y).is_some() {
                model.sync_dock_window();
            }
        }
        MouseKind::Up(Button::Left) if model.term_dock.is_dragging() => {
            if let Some(rows) = model.term_dock.end_drag() {
                model.settings_data.dock_rows = Some(rows);
            }
        }
        // The pan moves by the pointer's exact delta: whole cells on a
        // terminal, fractional ones on a window (`Model::pointer_at`).
        MouseKind::Drag(Button::Left) if model.roadmap_drag.is_some() => {
            work::graph_drag(model, &event);
        }
        MouseKind::Up(Button::Left) if model.roadmap_drag.is_some() => work::end_graph_drag(model),
        // A proposal row dropped on another reorders the list (Birth
        // F4's `drag · Reorder`); dropped anywhere else, the grab is
        // simply let go.
        MouseKind::Up(Button::Left) if model.birth.dragging().is_some() => {
            match target.and_then(Widget::birth_row_index) {
                Some(to) => {
                    model.birth.drop_on(to);
                }
                None => {
                    model.birth.drop_on(usize::MAX);
                }
            }
        }
        // A right-click on a proposal row opens it for renaming
        // (`right-click · Edit`).
        MouseKind::Down(Button::Right) => {
            if let Some(index) = target.and_then(Widget::birth_row_index) {
                model.birth.edit(index);
            }
        }
        MouseKind::Move => {
            model.hovered = target;
            model.pointer = Some((event.col, event.row));
            work::graph_hover(model, target, &event);
        }
        _ => {}
    }
    Vec::new()
}

// ── Birth ─────────

/// Open Birth's flow: the camera moves to [`Place::Birth`] where the
/// flow stands — the ground for a fresh flow, the moment it was left
/// at otherwise — with the slot `armed` while a desktop drag hovers.
/// Native only: a terminal has no altitude to open it at.
fn birth_open(model: &mut Model, armed: bool) {
    if !model.surface().is_native() {
        return;
    }
    model.birth.open(armed);
    model.show_place(Place::Birth);
}

/// A folder dropped from the desktop: the flow opens if it was not
/// open, and the folder is the ground — the survey is asked for.
fn birth_drop(model: &mut Model, path: String) -> Vec<Cmd> {
    if !model.surface().is_native() {
        return Vec::new();
    }
    birth_open(model, false);
    birth_choose(model, Ground::Folder(path))
}

/// Choose the ground and ask the entry point to survey it — unless the
/// same ground's survey is already kept, in which case the flow simply
/// returns to it.
fn birth_choose(model: &mut Model, ground: Ground) -> Vec<Cmd> {
    let now = model.ticks;
    model
        .birth
        .choose(ground, now)
        .map(|cmd| vec![Cmd::Birth(cmd)])
        .unwrap_or_default()
}

/// Advance the flow where the tick took it: the survey's rows landed
/// and held their beat, the interview opens.
fn birth_settle(model: &mut Model) {
    if model.surface().is_native() && model.place == Place::Birth {
        let animate = model.theme.motion().is_full();
        model.birth.settle(model.now(), animate);
    }
}

/// A job's answer for the flow: the survey's report, the drafted
/// proposal, the roadmap written — or a failure while the survey was
/// being read, which stands the flow down to the ground. Any other
/// output is not Birth's.
fn birth_job(model: &mut Model, output: &JobOutput) {
    match output {
        JobOutput::Survey(report) => {
            model.birth.surveyed((**report).clone(), model.ticks);
            birth_settle(model);
        }
        JobOutput::Proposal(proposal) => model.birth.charted((**proposal).clone(), model.ticks),
        JobOutput::Born(data) => {
            // First light: the accepted roadmap is the Sky the window
            // shows, seeded from what was written, and the project's
            // name is the header's.
            model.roadmap.seed(&data.chunks, &[]);
            model.title = Some(data.name.clone());
            model.birth.born((**data).clone());
        }
        JobOutput::Failed(_) if model.birth.moment() == BirthMoment::Survey => {
            model.birth.survey_failed();
        }
        _ => {}
    }
}

/// A click on one of Birth's controls, by the moment it belongs to.
fn birth_click(model: &mut Model, widget: Widget) -> Vec<Cmd> {
    if model.place != Place::Birth {
        return Vec::new();
    }
    match widget {
        Widget::BirthSlot => model.birth.type_into(GroundField::Browse),
        Widget::BirthClone => model.birth.type_into(GroundField::Clone),
        Widget::BirthEmpty => model.birth.type_into(GroundField::Empty),
        Widget::BirthContinue => return birth_continue(model),
        Widget::BirthAccept => return birth_accept(model),
        Widget::BirthRedraw => {
            return model
                .birth
                .redraw()
                .map(|cmd| vec![Cmd::Birth(cmd)])
                .unwrap_or_default();
        }
        Widget::BirthEdit => {
            model.birth.edit(0);
        }
        Widget::BirthClaim => return birth_claim(model),
        Widget::BirthLater => model.show(StageTab::Roadmap),
        _ => {}
    }
    Vec::new()
}

/// `Continue ↵`: the current answer is committed; the last one asks
/// for the chart.
fn birth_continue(model: &mut Model) -> Vec<Cmd> {
    model
        .birth
        .advance()
        .map(|cmd| vec![Cmd::Birth(cmd)])
        .unwrap_or_default()
}

/// `Accept this roadmap`: the proposal is written to think-and-ship.
fn birth_accept(model: &mut Model) -> Vec<Cmd> {
    model
        .birth
        .accept()
        .map(|cmd| vec![Cmd::Birth(cmd)])
        .unwrap_or_default()
}

/// `Claim <chunk>` on first light: the window closes and the entry
/// point reopens the cockpit on the new project's first iteration —
/// the launcher's own quit-and-dispatch, on the new folder.
fn birth_claim(model: &mut Model) -> Vec<Cmd> {
    let Some(data) = model.birth.born_data() else {
        return Vec::new();
    };
    let Some(chunk) = data.first_ready() else {
        return Vec::new();
    };
    let action = LaunchAction::Born {
        dir: data.dir.clone(),
        chunk: chunk.to_owned(),
    };
    launch(model, action)
}

/// Birth's keys, on a window at [`Place::Birth`]: a ground field
/// being typed takes plain keys and Enter chooses; the interview's
/// own-answer field takes plain keys and Enter continues; a row being
/// renamed takes plain keys and Enter commits; Enter on the charting
/// accepts and on first light claims. `None` elsewhere, so every other
/// key keeps its meaning.
fn birth_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if !model.surface().is_native() || model.place != Place::Birth {
        return None;
    }
    let plain = key.mods.is_empty() || key.mods == Modifiers::SHIFT;
    if !plain {
        return None;
    }
    if model.birth.field().is_some() {
        return Some(match key.code {
            KeyCode::Char(c) => {
                model.birth.field_char(c);
                Vec::new()
            }
            KeyCode::Backspace => {
                model.birth.field_backspace();
                Vec::new()
            }
            KeyCode::Enter => {
                let beside = model.birth.dir().to_owned();
                match model.birth.field_ground(&beside) {
                    Some(ground) => birth_choose(model, ground),
                    None => Vec::new(),
                }
            }
            KeyCode::Esc => return None,
            _ => Vec::new(),
        });
    }
    if model.birth.editing().is_some() {
        return Some(match key.code {
            KeyCode::Char(c) => {
                model.birth.edit_char(c);
                Vec::new()
            }
            KeyCode::Backspace => {
                model.birth.edit_backspace();
                Vec::new()
            }
            KeyCode::Enter => {
                model.birth.commit_edit();
                Vec::new()
            }
            KeyCode::Esc => return None,
            _ => Vec::new(),
        });
    }
    match (model.birth.moment(), key.code) {
        (BirthMoment::Interview, KeyCode::Char(c)) => {
            model.birth.answer_char(c);
            Some(Vec::new())
        }
        (BirthMoment::Interview, KeyCode::Backspace) => {
            model.birth.answer_backspace();
            Some(Vec::new())
        }
        (BirthMoment::Interview, KeyCode::Enter) => Some(birth_continue(model)),
        (BirthMoment::Charting, KeyCode::Enter) => Some(birth_accept(model)),
        (BirthMoment::FirstLight, KeyCode::Enter) => Some(birth_claim(model)),
        _ => None,
    }
}

/// Which triage act a signal button asks for.
enum SignalAct {
    Promote,
    Ignore,
    Snooze,
    /// \[Research\]: the agent's work, not the engine's — see
    /// [`signal_click`].
    Research,
}

/// How long \[Snooze\] quiets a signal.
const SNOOZE_MINUTES: u32 = 60;

/// What one wheel notch multiplies the tech-tree's scale by — four
/// notches double it, so a zoom reads as a glide rather than a jump.
pub(super) const ZOOM_NOTCH: f32 = 1.19;

/// What one press of the zoom key multiplies the tech-tree's scale by.
/// Coarser than a wheel notch on purpose: a notch is one of a stream a
/// finger rolls out, a key press is deliberate, and at the wheel's
/// 1.19 it would take eleven of them to reach [`Camera::MAX_ZOOM`].
pub(super) const ZOOM_KEY_STEP: f32 = 1.5;

/// A triage button on the signal at `index` in the inbox.
///
/// Promote, Ignore and Snooze are the engine's own verbs. Research is
/// not: the engine's `signal_research` wants findings — a summary, a
/// confidence, sources — and a button has none to give, so writing one
/// from here would forge an enrichment and unlock Promote on a signal
/// nobody looked at. Research therefore asks the agent, the way a typed
/// prompt does: echoed on the stage, refused while a turn
/// streams, and the card stays Open until real findings land on the
/// broadcast.
fn signal_click(model: &mut Model, index: usize, act: SignalAct) -> Vec<Cmd> {
    let Some((id, title)) = model
        .roadmap
        .signals()
        .get(index)
        .map(|signal| (signal.id.clone(), signal.title().to_owned()))
    else {
        return Vec::new();
    };
    let cmd = match act {
        SignalAct::Promote => EngineCmd::PromoteSignal { id },
        SignalAct::Ignore => EngineCmd::IgnoreSignal { id },
        SignalAct::Snooze => EngineCmd::SnoozeSignal {
            id,
            minutes: SNOOZE_MINUTES,
        },
        SignalAct::Research => {
            if model.session.state == TurnState::Streaming {
                return Vec::new();
            }
            let text = format!(
                "Research signal {id} (\u{201c}{title}\u{201d}): ground it in the codebase, \
                 research the world, and record the findings with signal_research."
            );
            model
                .session
                .turn
                .push_text(Voice::User, &format!("{text}\n"));
            model.session.notice = None;
            model.session.state = TurnState::Streaming;
            let (total, height) = model.conversation_window();
            model
                .session
                .viewport
                .end(total, height, model.now(), false);
            return vec![Cmd::Prompt(text)];
        }
    };
    vec![Cmd::Engine(cmd)]
}

/// \[Export\] on the shelf: write the selected shipped chunk's
/// deliverables as files, the chapter beside the receipts — the bundle
/// comes back on the same shelf, naming where it went.
fn shelf_export(model: &Model) -> Vec<Cmd> {
    let (Some(shelf), Some(chapter)) = (model.shelf(), model.chapter()) else {
        return Vec::new();
    };
    vec![Cmd::Shelf(ShelfCmd::Export {
        chunk: shelf.chunk.clone(),
        chapter,
    })]
}

/// \[Share page\] on the Report: write the chapter as one page — the
/// same document the view composes — and the shelf comes back naming
/// where it went.
fn shelf_share(model: &Model) -> Vec<Cmd> {
    let (Some(shelf), Some(chapter)) = (model.shelf(), model.chapter()) else {
        return Vec::new();
    };
    vec![Cmd::Shelf(ShelfCmd::Share {
        chunk: shelf.chunk.clone(),
        chapter,
    })]
}

/// A receipt's verb on the Report (`View` on the commit, `Log` on the
/// gates, `Detail` on the cost): the receipt is put on the stage as
/// notes and the camera turns to it — the bad day's `Details` pattern,
/// the one honest thing a window can do with a record it has no viewer
/// for. Nothing moves without a shelf to read.
fn report_receipt(model: &mut Model, receipt: Widget) -> Vec<Cmd> {
    let Some(shelf) = model.shelf() else {
        return Vec::new();
    };
    let lines: Vec<String> = match receipt {
        Widget::ReportCommit => vec![format!("commit \u{b7} {}", shelf.commit_text())],
        Widget::ReportGates if shelf.gates.is_empty() => vec!["gates \u{b7} none kept".to_owned()],
        Widget::ReportGates => shelf
            .gates
            .iter()
            .map(|(name, passed)| {
                format!(
                    "gate \u{b7} {name} \u{b7} {}",
                    if *passed { "passed" } else { "failed" }
                )
            })
            .collect(),
        Widget::ReportCost => vec![format!("cost \u{b7} {}", shelf.spend_text())],
        _ => return Vec::new(),
    };
    for line in lines {
        model.session.turn.push_note(&line);
    }
    model.show(StageTab::Turn);
    Vec::new()
}

/// `Open` on the Report's trace receipt — the provenance chip: the
/// Trace view, with the chunk's first reasoning step expanded so the
/// reader lands on the step and not on the feed's tail. With nothing
/// linked the Trace still opens; there is no step to expand.
fn report_trace(model: &mut Model) -> Vec<Cmd> {
    let first = model
        .shelf()
        .and_then(|shelf| shelf.think_refs.first().copied());
    model.show(StageTab::Session);
    if let Some(number) = first {
        model.inspect(Selected::Step(number));
    }
    Vec::new()
}

/// \[Pin\] / \[Unpin\] on the expanded step: ask the engine port for the
/// opposite of the flag as it stands — the model flips only on the
/// port's [`EngineEvent::Pinned`] answer, so the star never lies.
fn pin_click(model: &Model, number: u32) -> Vec<Cmd> {
    let Some(step) = model.trace.step(number) else {
        return Vec::new();
    };
    vec![Cmd::Engine(EngineCmd::Pin {
        step_number: number,
        pinned: !step.pinned,
    })]
}

/// Scroll the visible detail surface without moving the conversation or work list.
fn detail_scroll(model: &mut Model, delta: isize) {
    if model.tab == StageTab::Roadmap && model.roadmap_requests_open {
        super::FlightRequests::scroll(model, delta);
    } else {
        Inspector::scroll(model, delta);
    }
}

#[cfg(test)]
#[path = "update.tests.rs"]
mod tests;
