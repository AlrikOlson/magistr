//! The model as a layout tree.

use magistr_theme::math::{darken, lerp};
use magistr_theme::{Palette, ThemeColor};

use super::animation::Animation;
use super::binding::Binding;
use super::copy::Copy;
use super::dock_item::DockItem;
use super::frame_clock::FrameClock;
use super::gate_led::GateLed;
use super::hover;
use super::launcher_session::LauncherSession;
use super::loop_status::LoopStatus;
use super::model::{Model, PROMPT_MAX_ROWS, UNDOCK_COLS};
use super::motion::Motion;
use super::permission::Permission;
use super::plan_task::PlanTask;
use super::progress::Progress;
use super::rail::Rail;
use super::rail_mark::RailMark;
use super::roadmap_chunk::RoadmapChunk;
use super::session_outcome::SessionOutcome;
use super::size::Size;
use super::space::Space;
use super::stage_tab::StageTab;
use super::summary::Summary;
use super::task_state::TaskState;
use super::tooltip::Tooltip;
use super::turn_state::TurnState;
use super::widget::Widget;
use crate::agent::{PermissionKind, ToolStatus};
use crate::layout::{Frame, Inset, Length, Node};
use crate::render::{Attrs, Cell, Color, Style, width};
use crate::theme::{Icons, Pen};
use crate::turn::{Accent, Row, RowKind, diff_kind};

/// What the stage says before the agent has said anything.
const EMPTY_STAGE: &str = "nothing yet — waiting for the agent";
/// How far toward its own ink the session behind a Moment is drawn —
/// `Pen::fade`'s parameter, so 0 is the dim pen and 1 is untouched.
/// Low enough to read as backdrop, and the fade's own contrast floor
/// keeps it legible whatever the palette.
const MOMENT_DIM: f32 = 0.2;
/// The approval card's width and maximum share of the screen.
const MOMENT_COLS: u16 = 62;
/// Columns the card keeps clear on each side when the screen is too
/// narrow for [`MOMENT_COLS`].
const MOMENT_MARGIN: u16 = 4;
/// What the conditional-approval register says before anything is
/// typed into it — the prompt text.
const CONDITION_HINT: &str = "Add a condition. Example: allow, stop if other rows change";
/// The columns the prompt's mark (the icon and a space) takes, and the
/// gutter a continuation row wears instead.
const PROMPT_MARK_COLUMNS: u16 = 2;
const PROMPT_GUTTER: &str = "  ";
/// The cursor cell at the end of the line: a no-break space, so the
/// wrapper does not trim it away.
const CURSOR_BLANK: &str = "\u{a0}";
/// Steps one wave of the streaming shimmer takes to travel down the
/// gutter: about 720 ms at the runtime's tick.
const SHIMMER_STEPS: u64 = 12;
/// Columns the status strip gives the chunk's name before it is cut,
/// so the phases, LEDs and clock still fit 80 columns beside it.
const CHUNK_COLUMNS: usize = 20;
/// Dashes an accelerator-sheet group divider trails its label with —
/// fixed, since the sheet's rows clip in the stage's viewport rather
/// than measure it.
const SHEET_DIVIDER_COLS: usize = 24;

/// The label [`TraceStep::fields`](crate::cockpit::TraceStep::fields)
/// gives the step's `execution_ref` — the one field row that can become
/// a chunk jump.
const EXECUTION_REF: &str = "execution ref";

/// The cockpit: a header naming the turn's state and where the window
/// is, the phase rail (the roadmap chunk in progress, the loop's
/// iteration, the phases with the current one bright, the gate LEDs, the
/// clock), the left rail panel beside the stage (the ship plan as a
/// checklist, the roadmap's position, the pending signals — collapsible
/// on its toggle, resizable on its divider, its title in the border),
/// the stage panel with the tab bar inside its frame and the visible
/// rows of the turn under it (a placeholder on the other tabs), the
/// Needs-you dock — a labelled divider when nothing waits, the
/// needs-you panel in the focus accent with a row per card when
/// something does — the prompt line, and
/// a footer with what the keys do now and what the terminal answered —
/// every widget in its pen from the model's theme, the whole screen on
/// the palette's base.
///
/// The stage costs the screen, not the turn: one one-row text node per
/// visible row, each in the pen of its kind.
///
/// Pure: everything on screen comes from `model` and `clock`.
#[must_use]
pub fn view(model: &Model, clock: &FrameClock) -> Node {
    let pen = Pen::for_theme(&model.theme());
    let header = header_bar(model, clock, &pen);

    // The stage: a framed panel with the tab bar inside the frame's
    // first row, so the tabs attach to the pane they switch instead of
    // floating.
    let stage = Node::column()
        .grow(1.0)
        .min_width(Length::Cells(0))
        .frame(Frame::new(pen.panel(false).clone()))
        .child(tab_bar(model, &pen))
        .child(
            Node::column()
                .id(Widget::Stage.id())
                .style(pen.stage.clone())
                .grow(1.0)
                .viewport()
                .children(stage_rows(model, &pen)),
        );

    // The rail and its divider, in the order they stand beside the
    // stage: rail, then divider.
    let rail_side: Vec<Node> = if model.rail().is_collapsed() {
        vec![collapsed_rail(model, &pen)]
    } else {
        // Keep the divider unstyled so panel backgrounds do not create a stripe.
        // Its one-cell hit area matches the column-based rail drag calculations.
        vec![
            rail(model, &pen),
            Node::column()
                .id(Widget::RailDivider.id())
                .shrink(0.0)
                .width(Length::Cells(1)),
        ]
    };
    // A viewport itself: the rail's content must never push the prompt
    // line and the footer off a short screen.
    let mut body = Node::row()
        .grow(1.0)
        .viewport()
        .children(rail_side)
        .child(stage);
    if let Some(tab) = model.undocked_tab() {
        body = body
            .child(Node::column().shrink(0.0).width(Length::Cells(1)))
            .child(undock(model, tab, &pen));
    }

    let mut root = Node::column().style(pen.base.clone());
    root = root.child(header);
    root = root.child(status_strip(model, &pen));
    // The plain-text session's one-line banner: which terminal would
    // show the pixel cockpit. Absent entirely with pixels.
    if let Some(text) = model.banner() {
        root = root.child(
            Node::text(text)
                .style(pen.footer.clone())
                .height(Length::Cells(1)),
        );
    }
    root = root.child(body);
    // The terminal dock rises over the stage above the
    // prompt, under the Needs-you dock and the prompt box that follow.
    if let Some(dock) = term_dock(model, &pen) {
        root = root.child(dock);
    }
    root = root.child(dock(model, &pen));
    if let Some(popup) = completion_popup(model, &pen) {
        root = root.child(popup);
    }
    root = root.child(prompt_box(model, &pen));
    // The Moment: a decision the agent is
    // blocked on takes the screen. The session behind it *dims* — one
    // pass over the finished tree through `Pen::fade`, which moves
    // foregrounds and never grounds, so what is being decided over
    // stays on screen and legible (the fade holds the contrast floor);
    // a scrim with a background of its own would blank it, which is the
    // layout violation to avoid. The card is declared after the dim so it
    // is the one thing at full ink.
    if let Some(permission) = model.moment() {
        root.restyle_all(&|style| pen.fade(style, MOMENT_DIM));
        root = root.child(Node::centered_overlay(moment_card(model, permission, &pen)));
    }
    // The hover card is declared *after* everything it can cover, which
    // is what `Node::overlay`'s own contract asks for: the layer decides
    // hit order, declaration order decides paint order
    // (``).
    if let Some(card) = model.tooltip() {
        root = root.child(hover_card(&card, model.size(), &pen));
    }
    let mut root = root.child(footer_bar(model, &pen));
    // The pointer's answer, last: one pass over the finished tree, so
    // every id the hit registry will register is covered by the same
    // walk rather than by each surface remembering to ask.
    hover::answer(&mut root, model.hovered(), model.pressed(), &pen);
    root
}

/// The hover card the pointer earned, hung off the pointer's own cell.
///
/// Out of flow ([`Node::overlay`]), so it displaces nothing and is
/// clipped to the screen rather than to whatever it happens to be
/// declared inside — a card hung off a chip in the one-row footer is not
/// cut off by that row. It carries **no id**, so it is transparent to
/// the hit registry and to the accessibility tree: a card that swallowed
/// the click meant for the control it is describing would be worse than
/// no card at all.
fn hover_card(card: &Tooltip, size: Size, pen: &Pen) -> Node {
    let (col, row) = card.place(size.cols, size.rows);
    Node::row()
        .style(pen.footer.clone())
        .overlay(
            Inset::none()
                .left(Length::Cells(col))
                .top(Length::Cells(row)),
        )
        .width(Length::Cells(card.cols()))
        .height(Length::Cells(1))
        .padding(Space::PAD)
        .child(Node::text(card.text).height(Length::Cells(1)))
}

/// The stage's tab bar: one tab per surface, the active one bright —
/// pulsing once when it was just switched to. A viewport, so its labels clip on a narrow screen instead of squeezing
/// the rail's divider out of the row.
fn tab_bar(model: &Model, pen: &Pen) -> Node {
    Node::row()
        .style(pen.rail.clone())
        .gap(Space::GAP)
        .viewport()
        .min_width(Length::Cells(0))
        .height(Length::Cells(1))
        .children(StageTab::ALL.into_iter().map(|tab| {
            let active = model.tab() == tab;
            // The tab just switched to pulses once on the tick.
            let style = match in_motion(model).and_then(|(m, now)| m.tab_transition(now)) {
                Some(t) if active => pen.pulse(pen.tab(true), t),
                _ => pen.tab(active).clone(),
            };
            Node::text(tab.label())
                .id(Widget::tab(tab).id())
                .style(style)
                .shrink(0.0)
                .height(Length::Cells(1))
        }))
}

/// The undocked tab's third column at ≥160 columns: its name as a
/// heading, then the very rows the stage would show for it. The rows
/// keep their widget ids — the undocked tab is never the active one, so
/// a click resolves to exactly one surface. Rigid rows on a viewport,
/// like every scrolling column.
fn undock(model: &Model, tab: StageTab, pen: &Pen) -> Node {
    let mut rows = vec![
        Node::text(format!("{} {}", pen.icons.undock, Widget::tab(tab).label()))
            .style(pen.heading.clone())
            .height(Length::Cells(1)),
    ];
    rows.extend(match tab {
        StageTab::Changes => changes_rows(model, pen),
        _ => trace_rows(model, pen),
    });
    Node::column()
        .id(Widget::Undock.id())
        .style(pen.stage.clone())
        .width(Length::Cells(UNDOCK_COLS))
        .shrink(0.0)
        .viewport()
        .padding(Space::PAD)
        .children(rows.into_iter().map(|row| row.shrink(0.0)))
}

/// Where the rail's chrome fade stands, if it is fading: the rail's
/// rows come in from the dim pen over [`Motion::CHROME_FADE_STEPS`]
/// after a collapse or an expand — on a window only, since a terminal
/// never takes the stamp (`MotionKey::rail_open`).
fn rail_fade(model: &Model) -> Option<f32> {
    in_motion(model).and_then(|(m, now)| m.rail_fade(now))
}

/// `node` in `base`'s pen while the rail fades, and *untouched* — no
/// style set at all, inheriting as it always did — once it has settled,
/// so the terminal's tree is the tree it was, node for node.
fn fading(node: Node, pen: &Pen, base: &crate::render::Style, fade: Option<f32>) -> Node {
    match fade {
        Some(t) => node.style(pen.fade(base, t)),
        None => node,
    }
}

/// The collapsed rail: a two-column strip whose toggle expands it back.
fn collapsed_rail(model: &Model, pen: &Pen) -> Node {
    let fade = rail_fade(model);
    Node::column()
        .id(Widget::Rail.id())
        .style(pen.footer.clone())
        // Two columns, never one: the collapsed rail is a fixed strip
        // and not a pane that gives ground when the row is tight.
        .shrink(0.0)
        .width(Length::Cells(2))
        .child(fading(
            Node::text(pen.icons.active)
                .id(Widget::RailToggle.id())
                .height(Length::Cells(1)),
            pen,
            &pen.footer,
            fade,
        ))
}

/// The rail: its toggle row, the objective, the plan as a checklist,
/// the roadmap's position and the pending signals — a viewport, so a
/// long plan scrolls instead of pushing the footer off. On the left of
/// a terminal, on the right of a window (`Model::rail_on_right`).
fn rail(model: &Model, pen: &Pen) -> Node {
    let state = model.rail();
    let fade = rail_fade(model);
    let column = Node::column()
        .id(Widget::Rail.id())
        .style(pen.stage.clone())
        .width(Length::Cells(state.content_cols(model.size().cols)))
        .viewport()
        .frame(
            Frame::new(pen.panel(false).clone())
                .title("plan")
                .title_style(pen.heading.clone()),
        );
    let mut column = column
        .child(
            Node::text(pen.icons.collapse)
                .id(Widget::RailToggle.id())
                .style(arriving(pen, &pen.note, fade))
                .height(Length::Cells(1)),
        )
        .child(fading(
            Node::text(state.ship().objective().unwrap_or("no objective yet")),
            pen,
            &pen.stage,
            fade,
        ));
    for (index, task) in state.ship().tasks().iter().enumerate() {
        // The task that just completed pops once — bright and bold,
        // then back to the checklist pen it settles on.
        let style = pen.row(RowKind::Tool(task_status(task.state)));
        let style = match in_motion(model).and_then(|(m, now)| m.task_pop(index, now)) {
            Some(t) => pen.pop(style, t),
            None => style.clone(),
        };
        column = column.child(
            Node::text(task_text(task, pen.icons))
                .style(arriving(pen, &style, fade))
                .height(Length::Cells(1)),
        );
    }
    // The counter a completed chunk moves pops with it.
    let progress = match in_motion(model).and_then(|(m, now)| m.chunk_pop(now)) {
        Some(t) => pen.pop(&pen.rail, t),
        None => pen.rail.clone(),
    };
    column = column.child(
        Node::text(progress_text(state, pen.icons))
            .style(arriving(pen, &progress, fade))
            .height(Length::Cells(1)),
    );
    if state.progress().1 > 0 {
        column = column.child(rail_gauge(model, state, pen));
    }
    if !model.roadmap().is_empty() {
        column = column.child(rail_graph(model, pen));
    }
    column.child(
        Node::text(format!("signals {}", state.signals_pending()))
            .style(arriving(pen, &pen.note, fade))
            .height(Length::Cells(1)),
    )
}

/// Rows the rail's mini-DAG block takes once the pixel surface is
/// placed; in cell-only mode the two summary lines stand alone.
const RAIL_GRAPH_ROWS: u16 = 6;

/// The rail's mini-DAG block: empty rows for the pixel tree to cover
/// once the runtime reported its rect, the two-line text summary until
/// then (and always in cell-only mode). Rigid, like every block a
/// reported rect depends on.
fn rail_graph(model: &Model, pen: &Pen) -> Node {
    let mut graph = Node::column().id(Widget::RailGraph.id()).shrink(0.0);
    if model.rail_graph_rect().is_some() {
        return graph.height(Length::Cells(RAIL_GRAPH_ROWS));
    }
    for line in model.roadmap().mini_summary(pen.icons) {
        graph = graph.child(
            Node::text(line)
                .style(pen.note.clone())
                .height(Length::Cells(1)),
        );
    }
    graph.height(Length::Cells(2))
}

/// The rail's roadmap gauge: the eased fill as a bar, then the
/// `done/total` label — one row, as it always was. With the pixel layer
/// live the bar's cells are blank and [`Gauge`](crate::pixel::Gauge)
/// draws the fill at sub-cell resolution over them (the driver narrows
/// the row's rect to [`rail_gauge_cols`]); otherwise the cells are the
/// `▰▱` ramp. Either way the row is the same width, so nothing moves
/// when the probe answers — and when the rail is too narrow for a bar
/// the label stands alone, as it always did.
fn rail_gauge(model: &Model, state: &Rail, pen: &Pen) -> Node {
    let (done, total) = state.progress();
    let width = usize::from(rail_gauge_cols(model));
    if width == 0 {
        return segment(&format!("{done}/{total}"), pen.rail.clone()).id(Widget::RailGauge.id());
    }
    let fill = model.motion().rail_fill(model.now());
    let bar = if model.has_pixels() {
        " ".repeat(width)
    } else {
        progress_bar(fill, width, pen.icons)
    };
    segment(&format!("{bar} {done}/{total}"), pen.rail.clone()).id(Widget::RailGauge.id())
}

/// Cells of the rail's gauge row the bar itself takes — the row less
/// the ` done/total` label. The driver narrows the row's placed rect to
/// this so the pixel gauge covers the bar and not the label. `0` when
/// the rail is too narrow for a bar at all.
pub fn rail_gauge_cols(model: &Model) -> u16 {
    let state = model.rail();
    let (done, total) = state.progress();
    let label = format!(" {done}/{total}");
    let label = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
    state
        .content_cols(model.size().cols)
        .saturating_sub(2 + label)
}

/// The eased fraction as `width` cells of the `▰▱` ramp.
fn progress_bar(fill: f32, width: usize, icons: &Icons) -> String {
    let filled = filled_cells(fill, width);
    let mut bar: String = icons.bar_full.repeat(filled);
    bar.push_str(&icons.bar_empty.repeat(width - filled));
    bar
}

/// Cells of a `cells`-wide bar a fill of `fill` (`0..=1`) covers —
/// floored, with a hair of slack so a settled `done/total` lands on the
/// same cell integer division would.
fn filled_cells(fill: f32, cells: usize) -> usize {
    ((fill.clamp(0.0, 1.0) * cells as f32 + 1e-3).floor() as usize).min(cells)
}

/// One checklist line: the state's glyph, the title, and the checks
/// recorded on the task when there are any.
fn task_text(task: &PlanTask, icons: &Icons) -> String {
    let mut line = format!("{} {}", task.state.glyph(icons), task.title);
    if task.checks_failed > 0 {
        line.push_str(&format!(" {}{}", icons.failed, task.checks_failed));
    } else if task.checks_passed > 0 {
        line.push_str(&format!(" {}{}", icons.done, task.checks_passed));
    }
    line
}

/// The pen a checklist line borrows: the tool pens already grade
/// pending/running/done/failed, which is exactly the checklist's grading.
const fn task_status(state: TaskState) -> ToolStatus {
    match state {
        TaskState::Planned | TaskState::Skipped => ToolStatus::Pending,
        TaskState::Active => ToolStatus::InProgress,
        TaskState::Completed => ToolStatus::Completed,
        TaskState::Blocked => ToolStatus::Failed,
    }
}

/// Where the roadmap stands: the chunk in progress and done/total.
fn progress_text(state: &Rail, icons: &Icons) -> String {
    let (done, total) = state.progress();
    match state.chunk() {
        Some(chunk) => format!("{} {chunk} · {done}/{total} done", icons.here),
        None => format!("roadmap {done}/{total} done"),
    }
}

/// `text` cut to `columns` characters, with an ellipsis when it was
/// cut — at the last word break inside the limit when the text has one,
/// so a cut never splits a word.
fn shorten(text: &str, columns: usize) -> String {
    if text.chars().count() <= columns {
        return text.to_owned();
    }
    let cut: String = text.chars().take(columns.saturating_sub(1)).collect();
    let cut = match cut.rfind(' ') {
        Some(at) if at > 0 => cut[..at].trim_end().to_owned(),
        _ => cut,
    };
    format!("{cut}…")
}

/// The status strip — the header bar's second row, on the same chrome
/// surface: the roadmap chunk in progress, once one started; while a
/// loop runs, `iteration i/N` (`paused i/N`, `ended i/N` when so);
/// always the four phases as a segmented indicator with the current one
/// bright; while a loop runs, one LED per gate started and the elapsed
/// time.
fn status_strip(model: &Model, pen: &Pen) -> Node {
    let current = model.phase().and_then(Widget::phase);
    let mut strip = Node::row()
        .id(Widget::Status.id())
        .style(pen.footer.clone())
        .gap(Space::GAP)
        .height(Length::Cells(1));
    if let Some(chunk) = model.rail().chunk() {
        strip = strip.child(
            Node::text(shorten(chunk, CHUNK_COLUMNS))
                .id(Widget::Chunk.id())
                .style(pen.permission.clone())
                .shrink(0.0),
        );
    }
    if let Some(status) = model.loop_status() {
        strip = strip.child(
            Node::text(iteration_text(status))
                .id(Widget::Iteration.id())
                .shrink(0.0),
        );
    }
    // Gapless, powerline-style: the ▸ separators segment the phases
    // without costing the columns the plain-gap strip spent, so the
    // whole strip still fits 80 columns beside three gates and the
    // clock.
    let mut phases = Node::row().shrink(0.0).height(Length::Cells(1));
    for (index, phase) in Widget::PHASES.iter().enumerate() {
        if index > 0 {
            phases = phases.child(segment(pen.icons.active, pen.footer.clone()));
        }
        phases = phases.child(
            Node::text(phase.label())
                .id(phase.id())
                .style(pen.phase(current == Some(*phase)).clone())
                .shrink(0.0)
                .height(Length::Cells(1)),
        );
    }
    strip = strip.child(phases);
    let Some(status) = model.loop_status() else {
        return strip;
    };
    if !status.gates().is_empty() {
        strip = strip.child(
            Node::row()
                .id(Widget::Gates.id())
                .gap(Space::TIGHT)
                .shrink(0.0)
                .height(Length::Cells(1))
                .children(status.gates().iter().enumerate().map(|(index, gate)| {
                    // A gate that just settled flashes once on its own
                    // pen — so a pass pops bright and a failure pops in
                    // the error colour, one wave each.
                    let style = pen.gate(gate.passed());
                    let popped = in_motion(model).and_then(|(m, now)| m.gate_pop(index, now));
                    let style = match popped {
                        Some(t) => pen.pop(style, t),
                        None => style.clone(),
                    };
                    Node::text(gate_text(gate, model, pen.icons))
                        .style(style)
                        .shrink(0.0)
                })),
        );
    }
    // The judge's LED, only once there is a verdict: a loop with no judge
    // paints the strip it always did.
    if let Some((text, verdict)) = status.verdict_text(pen.icons).zip(status.verdict()) {
        strip = strip.child(
            Node::text(text)
                .style(pen.gate(Some(verdict.ok && !verdict.impossible)).clone())
                .shrink(0.0),
        );
    }
    strip.child(
        Node::text(status.elapsed_text())
            .id(Widget::Elapsed.id())
            .shrink(0.0),
    )
}

/// A gate LED's text: the spinner's frame while its job runs and the
/// tick turns, its outcome glyph otherwise.
fn gate_text(gate: &GateLed, model: &Model, icons: &Icons) -> String {
    if gate.passed().is_none() && model.animates(Animation::Motion) {
        format!("{} {}", icons.spin(model.spin_step()), gate.name())
    } else {
        gate.text(icons)
    }
}

/// `iteration i/N` while it runs, `paused i/N`, `ended i/N` — the word
/// states the loop, the numbers stay, and the whole strip fits 80
/// columns with three gates and a two-digit iteration.
fn iteration_text(status: &LoopStatus) -> String {
    let word = if status.ended().is_some() {
        "ended"
    } else if status.is_paused() {
        "paused"
    } else {
        "iteration"
    };
    format!("{word} {}/{}", status.iteration(), status.max())
}

/// The visible rows of the turn, one node each — or the Changes, Trace
/// or Roadmap tab's rows on those tabs. Dispatches on
/// [`Model::shown_tab`], which is [`Model::tab`] everywhere but the
/// native Home slot — there the Sky (the tech-tree) stands in for the
/// launcher until builds the Galaxy.
fn stage_rows(model: &Model, pen: &Pen) -> Vec<Node> {
    if model.is_sheet_open() {
        return sheet_rows(model, pen);
    }
    if model.shown_tab() == StageTab::Home {
        return launcher_rows(model, pen);
    }
    if model.shown_tab() == StageTab::Changes {
        return changes_rows(model, pen);
    }
    if model.shown_tab() == StageTab::Trace {
        return trace_rows(model, pen);
    }
    if model.shown_tab() == StageTab::Roadmap {
        return roadmap_rows(model, pen);
    }
    if model.turn().is_empty() {
        return vec![Node::text(EMPTY_STAGE).height(Length::Cells(1))];
    }
    let top = model.top_row();
    let rows = model.visible_rows();
    let moving = model.animates(Animation::Motion);
    let shimmer_from = shimmer_start(model, top, rows.len());
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let at = top.saturating_add(index);
            let spun = moving.then(|| spun_row(row, model, pen)).flatten();
            let row = spun.as_ref().unwrap_or(row);
            // A row the last fold added comes in from the dim pen over
            // three ticks; a row that was already there is untouched, so
            // a settled stage is the stage it always was.
            let fade = in_motion(model).and_then(|(m, now)| m.row_arrival(at, now));
            let content = match (model.hold_at_row(at), row.kind) {
                // The held kill fills the card's head red as it is
                // held, from the left, by the ticks.
                (Some(fraction), _) if fraction > 0.0 => held_head(row, fraction, model, pen),
                (_, RowKind::EditButtons) => edit_buttons(row, index, pen),
                (_, RowKind::Image) => image_row(model, at, pen),
                (_, RowKind::Pane) => pane_row(model, at, pen),
                _ => stage_row(row, pen, fade),
            };
            let accent = model.turn().accent_at_row(at);
            let gutter = match shimmer_from {
                Some(start) if index >= start && accent != Accent::Meta => pen.shimmer(
                    pen.accent(accent),
                    shimmer_phase(model.now(), index - start),
                ),
                _ => pen.accent(accent).clone(),
            };
            Node::row()
                .height(Length::Cells(1))
                .style(arriving(pen, pen.row(row.kind), fade))
                .id(Widget::stage_row(index))
                .child(segment(accent.gutter(), gutter))
                .child(content)
        })
        .collect()
}

/// The first visible row of the block that is streaming — the tail
/// run of rows sharing the last row's accent — while the turn streams
/// and motion is on; `None` otherwise, and the gutter is still.
fn shimmer_start(model: &Model, top: usize, visible: usize) -> Option<usize> {
    if model.turn_state() != TurnState::Streaming
        || !model.animates(Animation::Motion)
        || visible == 0
    {
        return None;
    }
    let turn = model.turn();
    let last = turn.accent_at_row(top + visible - 1);
    let mut start = visible;
    while start > 0 && turn.accent_at_row(top + start - 1) == last {
        start -= 1;
    }
    Some(start)
}

/// Where row `offset` of the streaming block sits on the shimmer's
/// wave at step `ticks`: the crest travels down the block one row a
/// step, wrapping every [`SHIMMER_STEPS`].
fn shimmer_phase(now: f32, offset: usize) -> f32 {
    let offset = (offset as u64 % SHIMMER_STEPS) as f32;
    let step = (now + SHIMMER_STEPS as f32 - offset).rem_euclid(SHIMMER_STEPS as f32);
    step / SHIMMER_STEPS as f32
}

/// A tool card's head row with its running mark replaced by the
/// spinner's frame at this step — the same frame every spinner on
/// screen is on — while the tick runs; `None` for any other row. Every
/// mark is one cell, so the wrap stays right.
fn spun_row(row: &Row, model: &Model, pen: &Pen) -> Option<Row> {
    if row.kind != RowKind::Tool(ToolStatus::InProgress) || !row.spans.is_empty() {
        return None;
    }
    let rest = row.text.strip_prefix(pen.icons.running)?;
    let mut spun = row.clone();
    spun.text = format!("{}{rest}", pen.icons.spin(model.spin_step()));
    Some(spun)
}

/// A card's head while the kill is held: the row's own text, its first `fraction`
/// of the stage's columns on the palette's red ground through the
/// terminal pen — the program's own red, floored for the text on it —
/// and the rest as the row was. The fill is whole cells, so the edge
/// steps by one column a tick.
fn held_head(row: &Row, fraction: f32, model: &Model, pen: &Pen) -> Node {
    let columns = usize::from(model.turn_width());
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let filled = ((fraction.clamp(0.0, 1.0) * columns as f32).round() as usize).min(columns);
    let mut text = row.text.clone();
    let mut cells = width(&text) as usize;
    while cells < columns {
        text.push(' ');
        cells += 1;
    }
    let mut split = text.len();
    let mut seen = 0;
    for (at, c) in text.char_indices() {
        if seen >= filled {
            split = at;
            break;
        }
        seen += width(c.encode_utf8(&mut [0; 4])) as usize;
    }
    let red = pen.term(Color::Default, Color::Indexed(1), Attrs::BOLD);
    Node::row()
        .height(Length::Cells(1))
        .child(segment(&text[..split], red))
        .child(segment(&text[split..], pen.row(row.kind).clone()))
}

/// The Changes tab: the read-only viewer pane while one is open; else
/// the journal list — one head row per entry, the expanded one showing
/// its buttons and its diff — then the last run's rows, failing first;
/// a quiet note while the journal is empty.
fn changes_rows(model: &Model, pen: &Pen) -> Vec<Node> {
    let changes = model.changes();
    // The heading stays whatever the journal does, and the run's
    // verdict joins it once one has landed — a gate's answer is what
    // brings a user back to the tab, and behind a long journal it would
    // otherwise sit off the bottom of the stage. Both are one row, so
    // the pinned block is bounded however much the run said; the result
    // rows themselves scroll with everything else. Counted by
    // [`Changes::pinned_rows`](crate::cockpit::Changes::pinned_rows).
    let head = if changes.viewer().is_some() {
        // The pane names the file it is showing, and how to leave it.
        format!(
            "{} {} · {} closes",
            pen.icons.file,
            changes.heading(),
            Binding::ESC_VIEWER.keys
        )
    } else {
        changes.heading()
    };
    let mut rows = vec![
        Node::text(head)
            .style(pen.heading.clone())
            .height(Length::Cells(1)),
    ];
    if let Some(summary) = changes.run_summary() {
        let failed = changes
            .runs()
            .is_some_and(|(_, results)| results.iter().any(|row| !row.passed));
        rows.push(
            Node::text(summary)
                .style(pen.row(RowKind::Check(!failed)).clone())
                .height(Length::Cells(1)),
        );
    }
    // Everything below scrolls: a long journal, one expanded diff or a
    // run's rows each run past the stage otherwise. The window is the
    // model's, so the keys and the wheel move exactly these rows.
    let (total, height) = model.changes_window();
    let top = model.changes_top().min(total);
    let bottom = top.saturating_add(height).min(total);
    let mut body = changes_body(model, pen);
    body.truncate(bottom);
    rows.extend(body.drain(top..).map(|row| row.shrink(0.0)));
    rows
}

/// The Changes tab's scrolling rows, in order — counted by
/// [`Changes::body_rows`](crate::cockpit::Changes::body_rows).
fn changes_body(model: &Model, pen: &Pen) -> Vec<Node> {
    let changes = model.changes();
    if let Some((_, lines)) = changes.viewer() {
        // The pane's title is pinned above; only its lines scroll.
        return lines
            .iter()
            .map(|line| {
                Node::text(line.as_str())
                    .style(pen.code.clone())
                    .height(Length::Cells(1))
            })
            .collect();
    }
    if changes.is_empty() {
        // The pinned heading already says the journal is empty.
        return Vec::new();
    }
    let mut rows = Vec::new();
    for (index, entry) in changes.entries().iter().enumerate() {
        let expanded = changes.is_expanded(&entry.id);
        rows.push(change_head(
            entry,
            index,
            expanded,
            changes.is_approved(&entry.id),
            pen,
        ));
        if expanded {
            rows.push(change_buttons(index, pen));
            rows.extend(diff_rows(&entry.diff, pen));
        }
    }
    if let Some((_, results)) = changes.runs() {
        // The run's own heading is the pinned verdict strip.
        rows.extend(results.iter().map(|row| {
            let glyph = pen.icons.check(row.passed);
            let text = if row.detail.is_empty() {
                format!("{glyph} {}", row.name)
            } else {
                format!("{glyph} {} · {}", row.name, row.detail)
            };
            Node::text(text)
                .style(pen.row(RowKind::Check(row.passed)).clone())
                .height(Length::Cells(1))
        }));
    }
    rows
}

/// One journal entry's head row: the fold marker, the path, the +/−
/// counts, the parse verdict, and ` · approved` once the human marked
/// it — hittable to expand or fold the diff.
fn change_head(
    entry: &crate::workbench::JournalEntry,
    index: usize,
    expanded: bool,
    approved: bool,
    pen: &Pen,
) -> Node {
    let marker = pen.icons.fold(expanded);
    let verdict = if entry.parse_ok {
        "parse ok"
    } else {
        "parse failed"
    };
    let mut text = format!(
        "{marker} {} · {}{} {}{} · {verdict}",
        entry.path, pen.icons.added, entry.added, pen.icons.removed, entry.removed
    );
    if approved {
        text.push_str(" · approved");
    }
    let status = if entry.parse_ok {
        ToolStatus::Completed
    } else {
        ToolStatus::Failed
    };
    Node::text(text)
        .id(Widget::change_row(index))
        .style(pen.row(RowKind::Tool(status)).clone())
        .height(Length::Cells(1))
}

/// An expanded change's button row: \[Open\], \[Undo\] and \[Approve\],
/// each hittable by the entry's index.
fn change_buttons(index: usize, pen: &Pen) -> Node {
    Node::row()
        .height(Length::Cells(1))
        .child(segment("  ", pen.code.clone()))
        .child(segment("[Open]", pen.allow.clone()).id(Widget::change_open(index)))
        .child(segment("  ", pen.code.clone()))
        .child(segment("[Undo]", pen.deny.clone()).id(Widget::change_undo(index)))
        .child(segment("  ", pen.code.clone()))
        .child(segment("[Approve]", pen.allow.clone()).id(Widget::change_approve(index)))
}

/// A diff body's rows, each line in the pen of its kind — a conflict
/// marker, added, removed, hunk header, context — indented under the
/// head row. The classifier is [`diff_kind`], shared with the turn
/// feed's edit cards so the two surfaces cannot disagree.
fn diff_rows(diff: &str, pen: &Pen) -> Vec<Node> {
    diff.lines()
        .map(|line| {
            let kind = diff_kind(line);
            Node::text(format!("  {line}"))
                .style(pen.row(kind).clone())
                .height(Length::Cells(1))
        })
        .collect()
}

/// The Trace tab: the checkpoint pane while one is open; else the think
/// feed under a heading with its \[Checkpoint\] button — the branch
/// block when steps branched, one head row per step, the expanded one
/// showing its buttons, its revision links and every field — then the
/// ship timeline of actions and checks; a quiet note while nothing has
/// landed.
fn trace_rows(model: &Model, pen: &Pen) -> Vec<Node> {
    let trace = model.trace();
    let mut rows = Vec::new();
    if trace.viewer().is_some() {
        rows.push(
            Node::text(format!(
                "{} checkpoint · {} closes",
                pen.icons.checkpoint,
                Binding::ESC_VIEWER.keys
            ))
            .style(pen.heading.clone())
            .height(Length::Cells(1)),
        );
    } else {
        rows.push(
            Node::row()
                .height(Length::Cells(1))
                .child(segment("think trace", pen.heading.clone()))
                .child(segment("  ", pen.stage.clone()))
                .child(segment("[Checkpoint]", pen.allow.clone()).id(Widget::Checkpoint.id())),
        );
        if trace.has_branches() {
            let mut graph = Node::column().id(Widget::TraceGraph.id());
            for line in trace.branch_rows() {
                graph = graph.child(
                    Node::text(line)
                        .style(pen.note.clone())
                        .height(Length::Cells(1)),
                );
            }
            rows.push(graph);
        }
    }
    // Everything below the heading (and the branch graph, which anchors
    // the pixel surface and so stays whole) scrolls: a long trace runs
    // past the stage otherwise. The window is the model's, so the keys
    // and the wheel move exactly these rows.
    let (total, height) = model.trace_window();
    let top = model.trace_top().min(total);
    let bottom = top.saturating_add(height).min(total);
    let mut body = trace_body(model, pen);
    body.truncate(bottom);
    rows.extend(body.drain(top..));
    rows
}

/// The Trace tab's scrolling rows, in order — counted by
/// [`Trace::body_rows`](crate::cockpit::Trace::body_rows).
fn trace_body(model: &Model, pen: &Pen) -> Vec<Node> {
    let trace = model.trace();
    if let Some(lines) = trace.viewer() {
        return lines
            .iter()
            .map(|line| {
                Node::text(line.as_str())
                    .style(pen.code.clone())
                    .height(Length::Cells(1))
            })
            .collect();
    }
    if trace.is_empty() {
        return vec![
            Node::text("no trace yet — the think feed is quiet")
                .style(pen.note.clone())
                .height(Length::Cells(1)),
        ];
    }
    let mut rows = Vec::new();
    for (index, step) in trace.steps().iter().enumerate() {
        let expanded = trace.is_expanded(step.number);
        rows.push(trace_head(step, index, expanded, pen));
        if expanded {
            rows.push(trace_buttons(step, pen));
            let jump = step
                .execution_ref
                .as_deref()
                .is_some_and(|reference| model.roadmap().chunk_for_ref(reference).is_some());
            rows.extend(trace_fields(step, jump, pen));
        }
    }
    if !trace.moments().is_empty() {
        rows.push(
            Node::text("ship")
                .style(pen.heading.clone())
                .height(Length::Cells(1)),
        );
        rows.extend(trace.moments().iter().map(|moment| moment_row(moment, pen)));
    }
    rows
}

/// Columns the tech-tree block keeps as a text legend on its left —
/// the workstream names in tier order; the driver places the pixel
/// surface to their right, so glyphs and pixels never overlap.
pub const ROADMAP_LEGEND_COLS: u16 = 26;

/// The name column's width on a chunk row.
const ROADMAP_NAME_COLS: usize = 24;

/// A one-cell breathing row between the tab's sections.
fn spacer() -> Node {
    Node::text("").height(Length::Cells(1))
}

/// `text` clipped to `max` characters, ellipsised when it was longer.
fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut clipped: String = text.chars().take(max.saturating_sub(1)).collect();
        clipped.push('…');
        clipped
    }
}

/// The Roadmap tab: a heading that carries the accounting and the
/// tech-tree block (workstream legend on the left, the pixel DAG placed
/// to its right in Graphics mode) pinned above a scrolling body — the
/// hover line, then the working set as status sections (one aligned row
/// per chunk, the selected one expanded to its detail card with
/// \[Start\]/\[Complete\], done collapsed to a count) and the open
/// proposals and the signals inbox, each row with its buttons; a quiet
/// note while the store is empty.
fn roadmap_rows(model: &Model, pen: &Pen) -> Vec<Node> {
    let roadmap = model.roadmap();
    let mut rows = vec![
        Node::row()
            .height(Length::Cells(1))
            .child(segment("roadmap", pen.heading.clone()))
            .child(segment("   ", pen.stage.clone()))
            .child(segment(&roadmap.summary_line(), pen.note.clone())),
    ];
    if roadmap.is_empty() {
        rows.push(
            Node::text("no roadmap yet — the store is empty")
                .style(pen.note.clone())
                .height(Length::Cells(1)),
        );
        return rows;
    }
    let graph_rows = model.roadmap_graph_rows();
    // No shrink: the tree must not resize when a detail card opens
    // below it — the list clips instead.
    let mut graph = Node::column()
        .id(Widget::RoadmapGraph.id())
        .height(Length::Cells(graph_rows))
        .shrink(0.0);
    // the scale readout takes the block's last row while the
    // tree is off the fit, and nothing at all while it sits on it. It
    // is *reserved* rather than appended: the legend gives up a band
    // instead of the readout being the thing that clips, since a
    // readout that vanishes on a busy roadmap says the wrong thing
    // about where the camera is.
    let scale = roadmap.scale_label();
    let bands = usize::from(graph_rows).saturating_sub(usize::from(scale.is_some()));
    for line in roadmap.band_rows().into_iter().take(bands).chain(scale) {
        graph = graph.child(
            Node::text(clip(&line, usize::from(ROADMAP_LEGEND_COLS) - 2))
                .style(pen.note.clone())
                .height(Length::Cells(1)),
        );
    }
    rows.push(graph);
    // Everything below the heading and the tech-tree block (which
    // anchors the pixel surface, and so stays whole) scrolls: a
    // hundred-chunk roadmap runs far past the stage otherwise. The
    // window is the model's, so the keys and the wheel move exactly
    // these rows.
    let (total, height) = model.roadmap_window();
    let top = model.roadmap_top().min(total);
    let bottom = top.saturating_add(height).min(total);
    let mut body = roadmap_body(model, pen);
    body.truncate(bottom);
    rows.extend(body.drain(top..));
    // Every row rigid: an overfull tab clips at the stage's viewport
    // instead of flex-shrinking a row to nothing and shifting every
    // click target below it.
    rows.into_iter().map(|row| row.shrink(0.0)).collect()
}

/// The Roadmap tab's scrolling rows, in order — counted by
/// [`Roadmap::body_rows`](crate::cockpit::Roadmap::body_rows).
fn roadmap_body(model: &Model, pen: &Pen) -> Vec<Node> {
    let roadmap = model.roadmap();
    let glow = in_motion(model).map(|(_, now)| Motion::glow_phase(now));
    let mut rows = Vec::new();
    if let Some(title) = roadmap.hovered_title() {
        rows.push(
            Node::text(format!("{} {title}", pen.icons.here))
                .style(pen.note.clone())
                .height(Length::Cells(1)),
        );
    }
    let list = roadmap.rows();
    // A chunk selected on the tree but absent from the list (done) shows
    // its card right under the graph, where the click just landed.
    if let Some(open) = roadmap.selected()
        && !list.iter().any(|chunk| chunk.id == open.id)
    {
        rows.push(spacer());
        rows.push(
            Node::text(format!(
                "{} {} — {}",
                pen.icons.done,
                open.name,
                clip(&open.title, 80)
            ))
            .style(pen.stage.clone())
            .height(Length::Cells(1)),
        );
        rows.extend(roadmap_detail(open, pen));
    }
    let mut section: Option<&str> = None;
    for (index, chunk) in list.iter().enumerate() {
        if section != Some(chunk.status.as_str()) {
            section = Some(chunk.status.as_str());
            let others = list.iter().filter(|c| c.status == chunk.status).count();
            rows.push(spacer());
            rows.push(roadmap_section_header(&chunk.status, others, pen));
        }
        let selected = roadmap.selected().is_some_and(|open| open.id == chunk.id);
        rows.push(roadmap_head(chunk, index, selected, glow, pen));
        if selected {
            rows.push(roadmap_buttons(chunk, pen));
            rows.extend(roadmap_detail(chunk, pen));
        }
    }
    if roadmap.done_count() > 0 {
        rows.push(spacer());
        rows.push(
            Node::text(format!(
                "{} done · {}",
                pen.icons.done,
                roadmap.done_count()
            ))
            .style(pen.note.clone())
            .height(Length::Cells(1)),
        );
    }
    let proposals = roadmap.proposals();
    if !proposals.is_empty() {
        rows.push(spacer());
        rows.push(
            Node::text(format!("{} proposals", pen.icons.proposal))
                .style(pen.heading.clone())
                .height(Length::Cells(1)),
        );
        for (index, (chunk, kind, suggestion, reason)) in proposals.iter().enumerate() {
            let mut row = Node::row()
                .height(Length::Cells(1))
                .child(segment("  ", pen.stage.clone()))
                .child(segment(
                    &format!(
                        "{:<w$}",
                        clip(&chunk.name, ROADMAP_NAME_COLS),
                        w = ROADMAP_NAME_COLS
                    ),
                    pen.stage.clone(),
                ))
                .child(segment(
                    &format!("  {} → {suggestion}", kind.label()),
                    pen.stage.clone(),
                ));
            if !reason.is_empty() {
                row = row.child(segment(&format!(" · {reason}"), pen.note.clone()));
            }
            rows.push(
                row.child(segment("   ", pen.stage.clone()))
                    .child(
                        segment("[Accept]", pen.allow.clone()).id(Widget::proposal_accept(index)),
                    )
                    .child(segment(" ", pen.stage.clone()))
                    .child(
                        segment("[Reject]", pen.deny.clone()).id(Widget::proposal_reject(index)),
                    ),
            );
        }
    }
    let signals = roadmap.signals();
    if !signals.is_empty() {
        rows.push(spacer());
        rows.push(
            Node::text(format!("{} signals", pen.icons.signal))
                .style(pen.heading.clone())
                .height(Length::Cells(1)),
        );
        for (index, signal) in signals.iter().enumerate() {
            let mut meta = String::new();
            if let Some(confidence) = signal.confidence {
                meta.push_str(&format!(" · {confidence:.1}"));
            }
            meta.push_str(&format!(" · {}", signal.status));
            rows.push(
                Node::row()
                    .height(Length::Cells(1))
                    .child(segment("  ", pen.stage.clone()))
                    .child(segment(
                        &format!(
                            "{} · {} — {}",
                            signal.kind,
                            signal.from,
                            clip(&signal.body, 60)
                        ),
                        pen.stage.clone(),
                    ))
                    .child(segment(&meta, pen.note.clone()))
                    .child(segment("   ", pen.stage.clone()))
                    .child(
                        segment("[Promote]", pen.allow.clone()).id(Widget::signal_promote(index)),
                    )
                    .child(segment(" ", pen.stage.clone()))
                    .child(segment("[Ignore]", pen.deny.clone()).id(Widget::signal_ignore(index)))
                    .child(segment(" ", pen.stage.clone()))
                    .child(segment("[Snooze]", pen.note.clone()).id(Widget::signal_snooze(index))),
            );
        }
    }
    rows
}

/// A status section's header: the glyph the rows underneath drop, and
/// how many of them follow.
fn roadmap_section_header(status: &str, count: usize, pen: &Pen) -> Node {
    let icons = pen.icons;
    let (glyph, label) = match status {
        "in_progress" => (icons.active, "in progress"),
        "pending" => (icons.pending, "pending"),
        "blocked" => (icons.failed, "blocked"),
        _ => (icons.planned, "backlog"),
    };
    Node::text(format!("{glyph} {label} · {count}"))
        .style(pen.heading.clone())
        .height(Length::Cells(1))
}

/// One chunk's row: the fold marker, the name column in its status's
/// pen, the dimmed band and workstream, and the proposal flag — the
/// status itself lives on the section header. Hittable to open or fold
/// the detail card. The chunk in progress wears the glow's breath at
/// `glow`, so the cell path says "here" the way the pixel
/// halo does.
fn roadmap_head(
    chunk: &crate::cockpit::RoadmapChunk,
    index: usize,
    selected: bool,
    glow: Option<f32>,
    pen: &Pen,
) -> Node {
    let marker = pen.icons.fold(selected);
    let name_pen = match chunk.status.as_str() {
        "in_progress" => pen.tool_running.clone(),
        "blocked" => pen.tool_failed.clone(),
        "backlog" => pen.note.clone(),
        _ => pen.stage.clone(),
    };
    // the cell tech-tree is a legend rather than a picture, so
    // "you are here" breathes on the chunk's own row instead — the same
    // raised cosine, at the same phase, as the pixel halo.
    let name_pen = match glow.filter(|_| chunk.is_in_progress()) {
        Some(phase) => pen.pulse(&name_pen, phase),
        None => name_pen,
    };
    let mut node = Node::row()
        .id(Widget::roadmap_row(index))
        .height(Length::Cells(1))
        .child(segment(&format!("{marker} "), pen.note.clone()))
        .child(segment(
            &format!(
                "{:<w$}",
                clip(&chunk.name, ROADMAP_NAME_COLS),
                w = ROADMAP_NAME_COLS
            ),
            name_pen,
        ))
        .child(segment(&format!("  {:<8}", chunk.band()), pen.note.clone()));
    if let Some(group) = &chunk.group {
        node = node.child(segment(&format!("  {group}"), pen.note.clone()));
    }
    if !chunk.proposals.is_empty() {
        node = node.child(segment(
            &format!("  {}", pen.icons.proposal),
            pen.permission.clone(),
        ));
    }
    node
}

/// The selected chunk's button row: \[Start\] while it can start,
/// \[Complete\] while it is in progress — each an ask for the engine
/// port.
fn roadmap_buttons(chunk: &crate::cockpit::RoadmapChunk, pen: &Pen) -> Node {
    let mut node = Node::row()
        .height(Length::Cells(1))
        .child(segment("    ", pen.stage.clone()));
    if matches!(chunk.status.as_str(), "pending" | "blocked" | "backlog") {
        node = node.child(segment("[Start]", pen.allow.clone()).id(Widget::RoadmapStart.id()));
    }
    if chunk.is_in_progress() {
        node =
            node.child(segment("[Complete]", pen.allow.clone()).id(Widget::RoadmapComplete.id()));
    }
    node
}

/// The selected chunk's detail rows: the full title, the description,
/// the acceptance bullets, the deps and the cross-refs — the non-empty
/// ones, indented under the head row and dimmed where they are context
/// rather than the claim.
fn roadmap_detail(chunk: &crate::cockpit::RoadmapChunk, pen: &Pen) -> Vec<Node> {
    let mut rows = vec![
        Node::text(format!("    {}", chunk.title))
            .style(pen.stage.clone())
            .height(Length::Cells(1)),
    ];
    for line in chunk.description.lines().filter(|l| !l.is_empty()) {
        rows.push(
            Node::text(format!("    {line}"))
                .style(pen.note.clone())
                .height(Length::Cells(1)),
        );
    }
    for bullet in &chunk.acceptance {
        rows.push(
            Node::text(format!("    {} {bullet}", pen.icons.bullet))
                .style(pen.bullet.clone())
                .height(Length::Cells(1)),
        );
    }
    if !chunk.deps.is_empty() {
        rows.push(
            Node::text(format!("    deps: {}", chunk.deps.join(", ")))
                .style(pen.note.clone())
                .height(Length::Cells(1)),
        );
    }
    if !chunk.cross_refs.is_empty() {
        rows.push(
            Node::text(format!("    refs: {}", chunk.cross_refs.join(", ")))
                .style(pen.note.clone())
                .height(Length::Cells(1)),
        );
    }
    rows
}

/// One step's head row: the fold marker, the number, the purpose, the
/// confidence, the pin star, the branch and the revised-by hint —
/// hittable to expand or fold the step.
fn trace_head(step: &crate::cockpit::TraceStep, index: usize, expanded: bool, pen: &Pen) -> Node {
    let marker = pen.icons.fold(expanded);
    let mut text = format!("{marker} {} think {}", pen.icons.think, step.number);
    if !step.purpose.is_empty() {
        text.push_str(&format!(" · {}", step.purpose));
    }
    if let Some(confidence) = step.confidence {
        text.push_str(&format!(" · {confidence:.2}"));
    }
    if step.pinned {
        text.push_str(&format!(" {}", pen.icons.pinned));
    }
    if let Some(branch) = step.branch_name.as_deref().or(step.branch_id.as_deref()) {
        text.push_str(&format!(" ({branch})"));
    }
    if let Some(by) = step.revised_by {
        text.push_str(&format!(" · revised by {by}"));
    }
    Node::text(text)
        .id(Widget::trace_row(index))
        .style(pen.row(RowKind::Event).clone())
        .height(Length::Cells(1))
}

/// An expanded step's button row: \[Pin\] or \[Unpin\] asks the engine
/// port, and each revision link navigates to the step it names.
fn trace_buttons(step: &crate::cockpit::TraceStep, pen: &Pen) -> Node {
    let label = if step.pinned { "[Unpin]" } else { "[Pin]" };
    let mut node = Node::row()
        .height(Length::Cells(1))
        .child(segment("  ", pen.stage.clone()))
        .child(segment(label, pen.allow.clone()).id(Widget::trace_pin(step.number)));
    if let Some(revises) = step.revises {
        node = node.child(segment("  ", pen.stage.clone())).child(
            segment(&format!("[revises {revises}]"), pen.link.clone())
                .id(Widget::trace_link(revises)),
        );
    }
    if let Some(by) = step.revised_by {
        node = node.child(segment("  ", pen.stage.clone())).child(
            segment(&format!("[revised by {by}]"), pen.link.clone()).id(Widget::trace_link(by)),
        );
    }
    node
}

/// An expanded step's fields, one labelled row per non-empty field (a
/// multi-line value continues on indented rows). The list itself is the
/// step's ([`TraceStep::fields`](crate::cockpit::TraceStep::fields)), so
/// the scroll counts exactly these rows.
///
/// With `jump`, the execution-ref row is the chunk link: the row it
/// always was, painted in the link pen and carrying
/// [`Widget::trace_chunk`]. It is the same row either way — a ref no
/// chunk answers stays plain text, and neither case moves a row, which
/// is what keeps [`Trace::body_rows`](crate::cockpit::Trace::body_rows)
/// right without knowing anything about the roadmap.
fn trace_fields(step: &crate::cockpit::TraceStep, jump: bool, pen: &Pen) -> Vec<Node> {
    step.fields()
        .iter()
        .flat_map(|(label, value)| {
            let linked = jump && *label == EXECUTION_REF;
            value.lines().enumerate().map(move |(line_index, line)| {
                let text = if line_index == 0 {
                    format!("  {label} · {line}")
                } else {
                    format!("    {line}")
                };
                let row = Node::text(text).height(Length::Cells(1));
                if linked && line_index == 0 {
                    row.style(pen.link.clone())
                        .id(Widget::trace_chunk(step.number))
                } else {
                    row.style(pen.code.clone())
                }
            })
        })
        .collect()
}

/// One ship-timeline row: an action with its kind, description and
/// task, or a check with its glyph, exit code and duration.
fn moment_row(moment: &crate::cockpit::ShipMoment, pen: &Pen) -> Node {
    use crate::cockpit::ShipMoment;
    match moment {
        ShipMoment::Action {
            task_id,
            kind,
            description,
            ..
        } => {
            let mut text = String::from(pen.icons.active);
            if !kind.is_empty() {
                text.push_str(&format!(" {kind}"));
            }
            if !description.is_empty() {
                text.push_str(&format!(" · {description}"));
            }
            if !task_id.is_empty() {
                text.push_str(&format!(" ({task_id})"));
            }
            Node::text(text)
                .style(pen.row(RowKind::Event).clone())
                .height(Length::Cells(1))
        }
        ShipMoment::Check {
            name,
            passed,
            exit_code,
            duration_ms,
        } => {
            let glyph = pen.icons.check(*passed);
            let mut text = format!("{glyph} {name}");
            if let Some(code) = exit_code {
                text.push_str(&format!(" · exit {code}"));
            }
            if let Some(ms) = duration_ms {
                text.push_str(&format!(" · {}", duration(*ms)));
            }
            Node::text(text)
                .style(pen.row(RowKind::Check(*passed)).clone())
                .height(Length::Cells(1))
        }
    }
}

/// `873ms` under a second, `1.2s` from there on.
fn duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{}.{}s", ms / 1000, (ms % 1000) / 100)
    }
}

/// An expanded edit card's button row: each bracketed label becomes a
/// hittable segment — \[Undo\] in the deny pen, \[Open\] in the allow
/// pen — addressed by the row's visible index; the gutter wrapper keeps
/// the stage-row id, so a click beside the buttons still folds the card.
fn edit_buttons(row: &crate::turn::Row, index: usize, pen: &Pen) -> Node {
    let text = row.text.as_str();
    let mut node = Node::row().height(Length::Cells(1));
    let mut at = 0;
    while let Some(open) = text[at..].find('[') {
        let start = at + open;
        let Some(close) = text[start..].find(']') else {
            break;
        };
        let end = start + close + 1;
        if start > at {
            node = node.child(segment(&text[at..start], pen.code.clone()));
        }
        let label = &text[start..end];
        let (id, style) = if label == "[Undo]" {
            (Widget::edit_undo(index), pen.deny.clone())
        } else {
            (Widget::edit_open(index), pen.allow.clone())
        };
        node = node.child(segment(label, style).id(id));
        at = end;
    }
    if at < text.len() {
        node = node.child(segment(&text[at..], pen.code.clone()));
    }
    node
}

/// One row of the stage: one text node in the kind's pen, or — when the
/// row carries inline spans — a row of segments, each in its own pen.
/// The caller ids it so a click can find it. `fade` is how far the row's
/// arrival has come, when it is still arriving: every pen the row wears,
/// its inline spans included, comes in on the same curve.
fn stage_row(row: &crate::turn::Row, pen: &Pen, fade: Option<f32>) -> Node {
    if row.spans.is_empty() {
        return Node::text(row.text.as_str())
            .style(arriving(pen, pen.row(row.kind), fade))
            .height(Length::Cells(1));
    }
    let mut node = Node::row().height(Length::Cells(1));
    let mut at = 0;
    for span in &row.spans {
        if span.start > at {
            node = node.child(segment(
                &row.text[at..span.start],
                arriving(pen, pen.row(row.kind), fade),
            ));
        }
        node = node.child(segment(
            &row.text[span.start..span.end],
            arriving(pen, &pen.span(row.kind, &span.style), fade),
        ));
        at = span.end;
    }
    if at < row.text.len() {
        node = node.child(segment(
            &row.text[at..],
            arriving(pen, pen.row(row.kind), fade),
        ));
    }
    node
}

/// The motion layer as the view reads it, with the step to read it at:
/// `None` under reduced motion, so every surface draws its final frame
/// without each one having to remember to ask. The tick never runs
/// there, but a stamp taken before the mode changed would otherwise
/// still be legible.
fn in_motion(model: &Model) -> Option<(&crate::cockpit::Motion, f32)> {
    model
        .theme()
        .motion()
        .is_full()
        .then(|| (model.motion(), model.now()))
}

/// `style` as it paints while its row arrives: faded in from the dim pen
/// while `fade` names a step of the arrival, exactly itself once the row
/// has settled — so nothing that is not arriving pays for this at all.
fn arriving(pen: &Pen, style: &crate::render::Style, fade: Option<f32>) -> crate::render::Style {
    fade.map_or_else(|| style.clone(), |t| pen.fade(style, t))
}

/// What the pane's first row says while its terminal has written
/// nothing yet.
const PANE_WAITING: &str = "no output yet";
/// The cells the strip's bar takes.
const STRIP_BAR_CELLS: u16 = 20;

/// The pane's time rail beside a row — blank past the content.
fn rail_text(mark: Option<RailMark>, pen: &Pen) -> String {
    mark.map_or_else(blank_rail, |mark| mark.text(pen.icons))
}

/// The rail's width of nothing: the indent an open card's rows sit
/// under when the rail has no mark there.
/// What the pane's strip row says of a program the reader ended
///  — the same words the window's head and receipt
/// use.
const KILLED_BY_YOU: &str = "killed by you";

fn blank_rail() -> String {
    " ".repeat(usize::from(RailMark::COLS))
}

/// The pen the pane's rail and strip are drawn in: the program's
/// default ink, dim — the pane's own dim ink.
fn pane_dim(pen: &Pen) -> Style {
    pen.term(Color::Default, Color::Default, Attrs::DIM)
}

/// The strip under the card's head: the program's own progress report as a bar in the
/// pane's columns with its percent, in the error pen when the program
/// says so, with no fraction when it has none.
fn strip_row(progress: Progress, columns: u16, ground: &Style, pen: &Pen) -> Node {
    let dim = pane_dim(pen);
    let (percent, label, style) = match progress {
        Progress::Running(p) => (p, format!("{p}%"), &dim),
        Progress::Failed(p) => (p, format!("{p}%"), &pen.error),
        Progress::Indeterminate => (0, "working".to_owned(), &dim),
        Progress::Cleared => (0, String::new(), &dim),
    };
    let filled = u16::from(percent).min(100) * STRIP_BAR_CELLS / 100;
    let bar: String = (0..STRIP_BAR_CELLS)
        .map(|cell| {
            if cell < filled {
                pen.icons.bar_full
            } else {
                pen.icons.bar_empty
            }
        })
        .collect();
    let text = format!("{}{bar} {label}", blank_rail());
    Node::text(clip(&text, usize::from(columns + RailMark::COLS)))
        .style(style.clone().bg(ground.bg))
        .height(Length::Cells(1))
}

/// One row of an open card's terminal pane, at the turn's absolute row
/// index: the program's own cells — its colours through the palette's
/// ANSI table and the contrast floor ([`Pen::term`]), its attributes —
/// sent as runs of one pen behind the time rail, the
/// cursor cell reversed while the run is live and the pane on its
/// tail, and on the pane's last row while it is scrolled up, the jump
/// pill `↓ Latest · +N` at the right edge counting what arrived. The
/// pane is the last [`Term::PANE_ROWS`](crate::cockpit::Term::PANE_ROWS)
/// rows of what the program wrote at the reader's place in it, cut to
/// the stage's mono measure — under the strip row while the program
/// reports its progress.
fn pane_row(model: &Model, at: usize, pen: &Pen) -> Node {
    use crate::cockpit::Term;
    let ground = pen.row(RowKind::Pane);
    let Some((id, offset)) = model.turn().pane_at_row(at) else {
        return Node::text("").height(Length::Cells(1));
    };
    let Some(term) = model.term(id) else {
        let text = if offset == 0 {
            format!("{}{PANE_WAITING}", blank_rail())
        } else {
            String::new()
        };
        return Node::text(text)
            .style(pen.note.clone().bg(ground.bg))
            .height(Length::Cells(1));
    };
    let ground = ground.clone();
    let columns = model.turn().mono_width().saturating_sub(RailMark::COLS);
    let strip = model.turn().has_strip(id);
    if strip && offset == 0 {
        // A program the reader killed says so where its progress was.
        if term.is_killed() {
            let text = format!("{}{} {KILLED_BY_YOU}", blank_rail(), pen.icons.stopped);
            return Node::text(clip(&text, usize::from(columns + RailMark::COLS)))
                .style(pen.error.clone().bg(ground.bg))
                .height(Length::Cells(1));
        }
        return strip_row(term.progress(), columns, &ground, pen);
    }
    let offset = offset.saturating_sub(usize::from(strip));
    let cursor = term.cursor().filter(|(r, _)| *r == offset).map(|(_, c)| c);
    let Some(cells) = term.pane_row(offset) else {
        // Past what the program has written: the pane's ground alone.
        return Node::text("").style(ground).height(Length::Cells(1));
    };
    // On the pane's last row while it is held, the jump pill takes the
    // right edge and the program's row yields the columns it needs.
    let pill =
        (offset + 1 == Term::PANE_ROWS && !term.is_following()).then(|| jump_pill(term, pen));
    let rail = rail_text(term.mark(offset), pen);
    cells_row(&cells, cursor, pill, columns, &rail, ground, pen)
}

/// The jump pill's text: `↓ Latest · +N`, counting what arrived under
/// a held window.
fn jump_pill(term: &crate::cockpit::Term, pen: &Pen) -> String {
    format!("{} Latest \u{b7} +{}", pen.icons.latest, term.arrived())
}

/// One row of a program's screen — the card's pane or the dock's — as
/// runs of one pen: the `rail` in the pane's dim ink, then `cells` cut
/// to `columns`, the `cursor` column reversed, the `pill` at the right
/// edge when there is one.
fn cells_row(
    cells: &[crate::cockpit::TermCell],
    cursor: Option<u16>,
    pill: Option<String>,
    columns: u16,
    rail: &str,
    ground: Style,
    pen: &Pen,
) -> Node {
    // The trailing cells nothing was written to are the ground's, so
    // the runs stop at the last cell that says something — or at the
    // cursor, which is drawn even on an empty row.
    let last = cells
        .iter()
        .rposition(|cell| {
            cell.text != " "
                || cell.fg != Color::Default
                || cell.bg != Color::Default
                || !cell.attrs.is_empty()
        })
        .map_or(0, |i| i + 1)
        .max(cursor.map_or(0, |c| usize::from(c) + 1));
    let budget = pill
        .as_deref()
        .map_or(columns, |pill| columns.saturating_sub(width(pill) + 1));
    let mut node = Node::row()
        .height(Length::Cells(1))
        .child(segment(rail, pane_dim(pen)));
    let mut used: u16 = 0;
    let mut run = String::new();
    let mut run_pen: Option<Style> = None;
    for (col, cell) in cells.iter().enumerate().take(last) {
        if cell.is_blank() {
            continue;
        }
        let w = width(&cell.text);
        if used + w > budget {
            break;
        }
        let mut attrs = cell.attrs;
        if cursor == Some(col as u16) {
            attrs |= Attrs::REVERSE;
        }
        let style = pen.term(cell.fg, cell.bg, attrs);
        if run_pen.as_ref() != Some(&style) {
            if let Some(done) = run_pen.take() {
                node = node.child(segment(&run, done));
                run.clear();
            }
            run_pen = Some(style);
        }
        run.push_str(&cell.text);
        used += w;
    }
    if let Some(done) = run_pen.take() {
        node = node.child(segment(&run, done));
    }
    if let Some(pill) = pill {
        let pad = columns.saturating_sub(used).saturating_sub(width(&pill));
        node = node
            .child(segment(&" ".repeat(usize::from(pad)), ground.clone()))
            .child(segment(&pill, pen.chip_key.clone()));
    }
    node.style(ground)
}

/// The terminal dock: the running card's screen raised
/// over the stage above the prompt, `None` while it is down. A grip row
/// — the panel grammar's labelled divider, naming the command and the
/// key that lowers it, the row the dock is dragged by — then the
/// docked window's rows through the pane's own painter, the jump pill
/// on the last while the reader holds it. The rows it takes come off
/// the stage ([`Model::stage_rows`]) so the feed keeps its tail above
/// it; while it rises or lowers the row count is the glide's.
fn term_dock(model: &Model, pen: &Pen) -> Option<Node> {
    let (term, height) = model.docked_term()?;
    if height == 0 {
        return None;
    }
    let cols = model.size().cols;
    let id = model.term_dock().id().unwrap_or_default();
    let title = model.turn().terminal_title(id).unwrap_or(id);
    let hint = Binding::hint(&[Binding::DOCK]);
    let label = format!(" {} ", clip(title, usize::from(cols) / 2));
    let fill = usize::from(cols).saturating_sub(width(&label) as usize + width(&hint) as usize + 6);
    let grip = Node::row()
        .id(Widget::TermDockGrip.id())
        .style(pen.stage.clone())
        .height(Length::Cells(1))
        .child(segment("\u{2500}\u{2500}", pen.panel_border.clone()))
        .child(segment(&label, pen.rail.clone()))
        .child(segment(&"\u{2500}".repeat(fill), pen.panel_border.clone()))
        .child(segment(&format!(" {hint} "), pen.rail.clone()))
        .child(segment("\u{2500}\u{2500}", pen.panel_border.clone()));
    let ground = pen.row(RowKind::Pane).clone();
    let columns = cols.saturating_sub(RailMark::COLS);
    let cursor = term.window_cursor();
    let rows = u16::try_from(height).unwrap_or(u16::MAX);
    let mut column = Node::column()
        .id(Widget::TermDock.id())
        .style(ground.clone())
        .height(Length::Cells(rows + 1))
        .child(grip);
    for offset in 0..height {
        let row = match term.window_row(offset) {
            Some(cells) => {
                let cursor = cursor.filter(|(r, _)| *r == offset).map(|(_, c)| c);
                let pill =
                    (offset + 1 == height && !term.is_following()).then(|| jump_pill(term, pen));
                let rail = rail_text(term.window_mark(offset), pen);
                cells_row(&cells, cursor, pill, columns, &rail, ground.clone(), pen)
            }
            None => Node::text("")
                .style(ground.clone())
                .height(Length::Cells(1)),
        };
        column = column.child(row);
    }
    Some(column)
}

/// One row of an image block, at the turn's absolute row index.
///
/// With the pixel layer live the row is left blank in the stage's own
/// ground: the driver composites the image over these cells, and the
/// blank keeps the slot exactly as wide as the placement so nothing
/// moves when the probe answers. Without it the same row is drawn in
/// halfblocks — `▀` per cell, the image's upper pixel as the
/// foreground over its lower one as the background — which is two image
/// rows per terminal row and costs one node either way.
fn image_row(model: &Model, row: usize, pen: &Pen) -> Node {
    let Some((image, offset, (cols, rows))) = model.turn().image_at_row(row) else {
        return Node::text("").height(Length::Cells(1));
    };
    let ground = pen.row(RowKind::Image).clone();
    if model.has_pixels() {
        return Node::leaf()
            .width(Length::Cells(cols))
            .height(Length::Cells(1))
            .shrink(0.0)
            .style(ground);
    }
    let cells = image
        .halfblocks(cols, rows, offset)
        .into_iter()
        .map(|(upper, lower)| {
            let style = ground
                .clone()
                .fg(Color::Rgb(upper.0, upper.1, upper.2))
                .bg(Color::Rgb(lower.0, lower.1, lower.2));
            Cell::new(HALFBLOCK, style)
        })
        .collect();
    Node::cells(cells)
}

/// The glyph an image's cell row is drawn with: the upper half block, so
/// its foreground is the top pixel and its background the bottom one.
const HALFBLOCK: &str = "▀";

/// Screen columns below which the launcher's cards stack full-width
/// instead of sitting on the two-column grid.
const LAUNCHER_GRID_MIN_COLS: u16 = 100;

/// The Home tab: the launcher as a dashboard. Four cards in panel
/// chrome — the project (or the `magistr init` card while the project
/// has no config), the roadmap's progress with the next-ready chunk
/// called out, the recent sessions with their status icons, the action
/// buttons as key-cap controls — on a grid that fills the stage: two
/// columns at ≥[`LAUNCHER_GRID_MIN_COLS`] screen columns, stacked
/// full-width below. The card data arrives as one job the entry point
/// spawned; until it delivers the cards say so.
fn launcher_rows(model: &Model, pen: &Pen) -> Vec<Node> {
    let avail = model.stage_rows();
    if model.size().cols < LAUNCHER_GRID_MIN_COLS {
        // Stacked: compact cards budgeted to the measured stage rows —
        // the sessions list takes exactly what is left, so no card's
        // ring ever lands on another row's cells.
        let cap = avail.saturating_sub(14).max(1);
        return vec![
            launcher_card(Widget::CardProject, pen, project_rows(model, pen, true)).shrink(0.0),
            launcher_card(Widget::CardRoadmap, pen, roadmap_card_rows(model, pen, 0)).shrink(0.0),
            launcher_card(Widget::CardActions, pen, vec![action_chips(pen)]).shrink(0.0),
            launcher_card(Widget::CardSessions, pen, session_rows(model, pen, cap)).shrink(0.0),
        ];
    }
    // The grid: each column's growing card stretches its stack to the
    // stage's bottom edge, so the cards own the screen the old text
    // column left dead. Half the row each, so the bars and rows size
    // predictably.
    let session_cap = avail.saturating_sub(8).max(1);
    let board_cap = avail.saturating_sub(11).max(1);
    let project = launcher_card(Widget::CardProject, pen, project_rows(model, pen, false));
    let roadmap = launcher_card(
        Widget::CardRoadmap,
        pen,
        roadmap_card_rows(model, pen, board_cap),
    );
    let sessions = launcher_card(
        Widget::CardSessions,
        pen,
        session_rows(model, pen, session_cap),
    );
    let actions = launcher_card(Widget::CardActions, pen, action_rows(pen));
    vec![
        Node::row()
            .grow(1.0)
            .gap(Space::TIGHT)
            .child(
                Node::column()
                    .grow(1.0)
                    .width(Length::Percent(0.5))
                    .min_width(Length::Cells(0))
                    .child(project.shrink(0.0))
                    .child(sessions.grow(1.0)),
            )
            .child(
                Node::column()
                    .grow(1.0)
                    .width(Length::Percent(0.5))
                    .min_width(Length::Cells(0))
                    .child(roadmap.grow(1.0))
                    .child(actions.shrink(0.0)),
            ),
    ]
}

/// One launcher card: a framed panel named in its border, its id on the
/// panel so the grid's coverage is measurable from the placed rects — a
/// viewport, so a long row clips inside the ring instead of painting
/// over its neighbour.
fn launcher_card(widget: Widget, pen: &Pen, rows: Vec<Node>) -> Node {
    Node::column()
        .id(widget.id())
        .viewport()
        .min_width(Length::Cells(0))
        .frame(
            Frame::new(pen.panel(false).clone())
                .title(widget.label())
                .title_style(pen.heading.clone()),
        )
        .children(rows.into_iter().map(|row| row.shrink(0.0)))
}

/// The project card's rows: the config's identity as labelled values —
/// or where to get a config while the project has none.
///
/// On a terminal that renders kitty's text sizing protocol the `name`
/// row is replaced by a headline: two blank rows carrying
/// [`Widget::CardHeadline`], which the runtime writes the project's name
/// across as one scaled `OSC 66` run ([`Model::headline`]). Everywhere
/// else — which is every terminal the goldens rasterize — the card is
/// exactly the four labelled rows it always was.
fn project_rows(model: &Model, pen: &Pen, compact: bool) -> Vec<Node> {
    let cols = launcher_card_cols(model);
    let Some(data) = model.launcher() else {
        return vec![segment("loading project status…", pen.note.clone())];
    };
    if !data.has_config {
        return vec![
            segment(
                &format!("{} not configured", pen.icons.warning),
                pen.error.clone(),
            ),
            segment(
                "run `magistr init` to set this project up",
                pen.stage.clone(),
            ),
        ];
    }
    if compact {
        return vec![
            segment(
                &shorten(&format!("{} · {}", data.project, data.stack), cols),
                pen.stage.clone(),
            ),
            segment(
                &shorten(&format!("{} · {}", data.git, data.backend), cols),
                pen.stage.clone(),
            ),
        ];
    }
    let headline = model.headline().is_some();
    let labelled: Vec<(&str, &str)> = if headline {
        // The headline *is* the name, so the label row would say it twice.
        vec![
            ("stack", data.stack.as_str()),
            ("git", data.git.as_str()),
            ("backend", data.backend.as_str()),
        ]
    } else {
        vec![
            ("name", data.project.as_str()),
            ("stack", data.stack.as_str()),
            ("git", data.git.as_str()),
            ("backend", data.backend.as_str()),
        ]
    };
    let rows = labelled.into_iter().map(|(label, value)| {
        Node::row()
            .height(Length::Cells(1))
            .child(segment(&format!("{label:<8} "), pen.thought.clone()))
            .child(segment(
                &shorten(value, cols.saturating_sub(9)),
                pen.stage.clone(),
            ))
    });
    if headline {
        return std::iter::once(headline_slot()).chain(rows).collect();
    }
    rows.collect()
}

/// The two blank rows the project card reserves for its scaled
/// headline. Blank by construction: the terminal draws the run's block
/// over both of them, so anything of ours in the second row would be
/// painted over — and would put the cells back the moment the diff
/// noticed them change.
fn headline_slot() -> Node {
    Node::column()
        .id(Widget::CardHeadline.id())
        .height(Length::Cells(crate::render::Headline::ROWS))
        .shrink(0.0)
}

/// The roadmap card's rows: the colour-ramped progress bar, the
/// next-ready chunk called out by name and title, and the accounting
/// line — `no roadmap yet` until a seed arrives.
fn roadmap_card_rows(model: &Model, pen: &Pen, board_cap: usize) -> Vec<Node> {
    let roadmap = model.roadmap();
    let (done, total) = (roadmap.done_count(), roadmap.active_count());
    if total == 0 {
        return vec![segment("no roadmap yet", pen.note.clone())];
    }
    let cols = launcher_card_cols(model);
    let palette = model.theme().palette();
    let mut rows = vec![ramp_bar(
        model.motion().roadmap_fill(model.now()),
        done,
        total,
        launcher_bar_cells(model, done, total),
        &palette,
        pen,
    )];
    match roadmap.next_ready() {
        Some(chunk) => {
            let mut next = Node::row()
                .height(Length::Cells(1))
                .child(segment(
                    &format!("next {} ", pen.icons.active),
                    pen.rail.clone(),
                ))
                .child(segment(&chunk.name, pen.user.clone()));
            if !chunk.title.is_empty() {
                let free = cols.saturating_sub(7 + chunk.name.chars().count());
                next = next.child(segment(
                    &shorten(&format!(" — {}", chunk.title), free),
                    pen.stage.clone(),
                ));
            }
            rows.push(next);
        }
        None => rows.push(segment(
            &format!("next {} nothing ready", pen.icons.active),
            pen.note.clone(),
        )),
    }
    rows.push(segment(
        &shorten(&roadmap.summary_line(), cols),
        pen.thought.clone(),
    ));
    for chunk in roadmap.rows().into_iter().take(board_cap) {
        rows.push(board_row(chunk, cols, pen));
    }
    rows
}

/// Cells the roadmap card's bar takes: the card's interior columns less
/// the ` done/total` label, floored so a tiny screen still shows a bar.
fn launcher_bar_cells(model: &Model, done: usize, total: usize) -> usize {
    let label = format!(" {done}/{total}").chars().count();
    launcher_card_cols(model).saturating_sub(label).max(10)
}

/// The roadmap's progress as a colour-ramped bar: every cell keeps its
/// position's colour on the red→yellow→blue ramp (the theme's lerp),
/// the cells under `fill` (the eased fraction) bright, the not-yet
/// cells darkened toward the base — then `done/total` at the end.
fn ramp_bar(
    fill: f32,
    done: usize,
    total: usize,
    cells: usize,
    palette: &Palette,
    pen: &Pen,
) -> Node {
    let filled = filled_cells(fill, cells);
    let base = Color::Rgb(palette.base.r, palette.base.g, palette.base.b);
    let mut row = Node::row().height(Length::Cells(1));
    for cell in 0..cells {
        let t = cell as f64 / cells.saturating_sub(1).max(1) as f64;
        let (glyph, color) = if cell < filled {
            (pen.icons.bar_full, ramp(palette, t))
        } else {
            (pen.icons.bar_empty, darken(ramp(palette, t), 0.55))
        };
        row = row.child(segment(
            glyph,
            Style::new()
                .fg(Color::Rgb(color.r, color.g, color.b))
                .bg(base),
        ));
    }
    row.child(segment(&format!(" {done}/{total}"), pen.stage.clone()))
}

/// Position `t` along the red→yellow→blue ramp — behind reads red, done
/// reads the pass accent (green sits on Latte's 3:1 contrast line, so
/// blue closes the ramp as it does the gate LEDs).
fn ramp(palette: &Palette, t: f64) -> ThemeColor {
    if t < 0.5 {
        lerp(palette.red, palette.yellow, t * 2.0)
    } else {
        lerp(palette.yellow, palette.blue, (t - 0.5) * 2.0)
    }
}

/// The sessions card's rows: one per recent session, its outcome as an
/// icon in the outcome's pen, the when and duration quiet, the preview
/// in body text — the hovered row raised as a surface band.
fn session_rows(model: &Model, pen: &Pen, cap: usize) -> Vec<Node> {
    let Some(data) = model.launcher() else {
        return vec![segment("loading…", pen.note.clone())];
    };
    if data.sessions.is_empty() {
        return vec![segment("none yet", pen.note.clone())];
    }
    let shown = if data.sessions.len() > cap {
        cap.saturating_sub(1)
    } else {
        data.sessions.len()
    };
    let mut rows: Vec<Node> = data
        .sessions
        .iter()
        .take(shown)
        .enumerate()
        .map(|(index, session)| session_row(index, session, model, pen))
        .collect();
    if shown < data.sessions.len() {
        rows.push(segment(
            &format!("… {} more", data.sessions.len() - shown),
            pen.note.clone(),
        ));
    }
    if let Some(note) = model.session_note() {
        rows.push(segment(
            &format!("{} {note}", pen.icons.blocked),
            pen.warn.clone(),
        ));
    }
    rows
}

/// One session row, its id in the hit registry so the pointer can find
/// it. The arrows' selection — which resumes on Enter — raises the row,
/// swapping every pen onto the first surface and the row's band fill
/// with them. Hover is not asked for here: since the pointer's
/// answer is one pass over the finished tree ([`hover::raise`]), and
/// this row is one [`HoverRole::Surface`](super::HoverRole::Surface)
/// among many.
fn session_row(index: usize, session: &LauncherSession, model: &Model, pen: &Pen) -> Node {
    let raised = model.session_selected() == Some(index);
    let icon = match (session.outcome, raised) {
        (SessionOutcome::Success, false) => &pen.heading,
        (SessionOutcome::Success, true) => &pen.diff_add,
        (SessionOutcome::Error, false) => &pen.error,
        (SessionOutcome::Error, true) => &pen.diff_remove,
        (SessionOutcome::Cancelled, false) => &pen.thought,
        (SessionOutcome::Cancelled, true) => &pen.fence,
    };
    let (quiet, body) = if raised {
        (&pen.fence, &pen.code)
    } else {
        (&pen.thought, &pen.stage)
    };
    let when = format!("{}  {}  ", session.when, session.duration);
    let free = launcher_card_cols(model).saturating_sub(2 + when.chars().count());
    let mut row = Node::row()
        .id(Widget::launch_session(index))
        .height(Length::Cells(1));
    if raised {
        row = row.style(pen.code.clone());
    }
    row.child(segment(
        &format!("{} ", session.outcome.glyph(pen.icons)),
        icon.clone(),
    ))
    .child(segment(&when, quiet.clone()))
    .child(segment(&shorten(&session.preview, free), body.clone()))
}

/// A button label from the registry: `[keys] does`.
fn accel_label(binding: Binding) -> String {
    format!("[{}] {}", binding.keys, binding.does)
}

/// The launcher's actions, each with the button widget that fires it.
const LAUNCHER_ACTIONS: [(Binding, Widget); 4] = [
    (Binding::NEXT, Widget::LaunchNext),
    (Binding::LOOP, Widget::LaunchLoop),
    (Binding::GATES, Widget::LaunchGates),
    (Binding::ROADMAP, Widget::LaunchRoadmap),
];

/// One action as a key-cap control: the key on a raised cap, what it
/// does beside it — the footer's grammar as a button, the id on the
/// whole control so the cap and the label both take the click.
fn action_button(binding: Binding, widget: Widget, pen: &Pen) -> Node {
    Node::row()
        .id(widget.id())
        .gap(Space::TIGHT)
        .shrink(0.0)
        .height(Length::Cells(1))
        .child(segment(
            &format!(" {} ", binding.keys),
            pen.chip_key.clone(),
        ))
        .child(segment(binding.does, pen.heading.clone()))
}

/// The actions card's rows on the grid: one control per row.
fn action_rows(pen: &Pen) -> Vec<Node> {
    LAUNCHER_ACTIONS
        .into_iter()
        .map(|(binding, widget)| action_button(binding, widget, pen))
        .collect()
}

/// The actions as one row of controls, for the stacked layout. No gap
/// of its own — each control's raised cap carries its own padding, and
/// all four have to fit an 80-column stage beside the rail.
fn action_chips(pen: &Pen) -> Node {
    let mut row = Node::row().height(Length::Cells(1));
    for (binding, widget) in LAUNCHER_ACTIONS {
        row = row.child(action_button(binding, widget, pen));
    }
    row
}

/// A launcher card's interior columns at the model's size: the stage
/// less the card's ring — half of it, less the grid's gap, on the grid.
fn launcher_card_cols(model: &Model) -> usize {
    let stage = usize::from(model.stage_width());
    if model.size().cols < LAUNCHER_GRID_MIN_COLS {
        stage.saturating_sub(2)
    } else {
        (stage.saturating_sub(usize::from(Space::TIGHT)) / 2).saturating_sub(2)
    }
}

/// One chunk on the roadmap card's board: its status as a glyph in the
/// status's pen, the name, the title when it has one.
fn board_row(chunk: &RoadmapChunk, cols: usize, pen: &Pen) -> Node {
    let icons = pen.icons;
    let (glyph, style) = match chunk.status.as_str() {
        "in_progress" => (icons.active, &pen.rail),
        "pending" => (icons.pending, &pen.stage),
        "blocked" => (icons.stopped, &pen.error),
        _ => (icons.planned, &pen.thought),
    };
    let text = if chunk.title.is_empty() {
        format!("{glyph} {}", chunk.name)
    } else {
        format!("{glyph} {} — {}", chunk.name, chunk.title)
    };
    segment(&shorten(&text, cols), style.clone())
}

/// The accelerator sheet over the stage: every binding the cockpit
/// registers, grouped as [`Binding::ALL`] carries them — generated from
/// the registry, never hand-written. The panel grammar carries the
/// hierarchy: each group opens as a labelled divider and every key sits
/// on a chip key cap, the same cap the footer wears.
///
/// The heading is pinned and the body scrolls under it (the fifth
/// viewport, ): the registry outgrew an 80x24 stage,
/// and a help overlay that silently drops half its rows is worse than
/// no help at all. The heading says where the window stands — but only
/// while the body overflows, so a sheet that fits is exactly the sheet
/// it always was.
fn sheet_rows(model: &Model, pen: &Pen) -> Vec<Node> {
    let mut body = sheet_body(pen);
    let (total, height) = model.sheet_window();
    let top = model.sheet_top().min(body.len());
    let bottom = top.saturating_add(height).min(body.len());
    let position = if height < total {
        format!(" · {}–{bottom} of {total}", top + 1)
    } else {
        String::new()
    };
    let mut rows = vec![segment(
        &format!(
            "{} keys · {} closes{position}",
            pen.icons.keys,
            Binding::ESC_SHEET.keys
        ),
        pen.heading.clone(),
    )];
    rows.extend(body.drain(top..bottom));
    rows
}

/// Every row of the sheet's body, in registry order — the sequence
/// [`Model::sheet_window`] measures and [`sheet_rows`] takes its window
/// of. Built whole so the count and the paint come from one walk of
/// [`Binding::ALL`].
fn sheet_body(pen: &Pen) -> Vec<Node> {
    let mut rows = Vec::with_capacity(Binding::sheet_rows());
    let mut group = "";
    for binding in Binding::ALL {
        if binding.group != group {
            group = binding.group;
            rows.push(segment("", pen.stage.clone()));
            rows.push(
                Node::row()
                    .height(Length::Cells(1))
                    .shrink(0.0)
                    .child(segment("── ", pen.panel_border.clone()))
                    .child(segment(group, pen.rail.clone()))
                    .child(segment(
                        &format!(" {}", "─".repeat(SHEET_DIVIDER_COLS)),
                        pen.panel_border.clone(),
                    )),
            );
        }
        rows.push(
            Node::row()
                .height(Length::Cells(1))
                .gap(Space::GAP)
                .shrink(0.0)
                .child(segment(
                    &format!(" {:<15} ", binding.keys),
                    pen.chip_key.clone(),
                ))
                .child(segment(binding.does, pen.stage.clone())),
        );
    }
    rows
}

/// One styled segment of a stage row.
fn segment(text: &str, style: crate::render::Style) -> Node {
    Node::text(text)
        .style(style)
        .height(Length::Cells(1))
        .shrink(0.0)
}

/// The Moment: centred plan card — one decision,
/// everything needed to make it, nothing else.
///
/// The whole card is data the ask brought with it: the cap names what
/// kind of approval this is, the right-hand line is
/// [`Permission::checkpoint`] and is simply **absent** when the ask was
/// not risky enough to have taken one (a placeholder would name a state
/// the spine does not hold), the steps are the plan's own, *Show
/// images* appears only while the call still has pictures to show, and
/// the `⇧A` offer names the scope a session-wide allow would cover.
///
/// The buttons carry the ids the Needs-you dock's own buttons carry, so
/// a click on the card's Allow is the same `Msg::Mouse` target the
/// strip sends and nothing downstream can tell the two apart.
/// `⇧A` appears as a caption rather than a second filled action.
fn moment_card(model: &Model, permission: &Permission, pen: &Pen) -> Node {
    let cols = MOMENT_COLS.min(model.size().cols.saturating_sub(MOMENT_MARGIN * 2));
    let inner = usize::from(cols.saturating_sub(Space::PAD * 2));
    let mut card = Node::column()
        .style(pen.stage.clone())
        .width(Length::Cells(cols))
        .padding(Space::PAD);
    // A cell grid has no elevation to lift a card off the screen with,
    // so a ring is what says "this is a surface of its own".
    card = card.frame(Frame::new(pen.panel(true).clone()));

    // The cap, and the checkpoint the ask named — pushed to the far
    // side of the same row, as shown on screen.
    let cap = format!("{} PLAN · APPROVAL REQUIRED", pen.icons.proposal);
    let mut head = Node::row()
        .height(Length::Cells(1))
        .child(segment(&cap, pen.warn.clone()));
    head = head.child(Node::leaf().grow(1.0));
    if let Some(checkpoint) = permission.checkpoint() {
        head = head.child(segment(
            &format!("Checkpoint {checkpoint} saved"),
            pen.thought.clone(),
        ));
    }
    card = card.child(head).child(spacer()).child(segment(
        &clip(permission.title(), inner),
        pen.heading.clone(),
    ));

    for (index, step) in permission.steps().enumerate() {
        let line = format!("{}. {}", index + 1, step);
        card = card.child(segment(&clip(&line, inner), pen.thought.clone()));
    }
    card = card.child(spacer());

    // The exits. Only the allow is filled: the single primary action.
    let mut buttons = Node::row().gap(Space::TIGHT).height(Length::Cells(1));
    if let Some(kind) = permission
        .choice(PermissionKind::AllowOnce)
        .map(|_| PermissionKind::AllowOnce)
    {
        buttons = buttons.child(moment_button(
            kind.label(),
            kind.key_cap(),
            Widget::Allow,
            pen.allow.clone(),
        ));
    }
    if !permission.hidden_images().is_empty() {
        buttons = buttons.child(moment_button(
            "Show images",
            "space",
            Widget::ShowImages,
            pen.permission.clone(),
        ));
    }
    if permission.choice(PermissionKind::RejectOnce).is_some() {
        buttons = buttons.child(moment_button(
            PermissionKind::RejectOnce.label(),
            PermissionKind::RejectOnce.key_cap(),
            Widget::Deny,
            pen.deny.clone(),
        ));
    }
    if permission.choice(PermissionKind::AllowAlways).is_some() {
        let offer = match permission.scope() {
            Some(scope) => format!(
                "{} · ALWAYS ALLOW ({})",
                PermissionKind::AllowAlways.key_cap(),
                scope.to_uppercase()
            ),
            None => format!("{} · ALWAYS ALLOW", PermissionKind::AllowAlways.key_cap()),
        };
        buttons = buttons
            .child(Node::leaf().grow(1.0))
            // The mono voice, not the sans: this is a key cap caption,
            // and the sans face has no ⇧ to draw.
            .child(segment(&offer, pen.thought.clone()));
    }
    card.child(buttons)
        .child(spacer())
        .child(condition_row(model, inner, pen))
}

/// An approval button with its action label and monospaced key hint.
fn moment_button(verb: &str, cap: &str, widget: Widget, style: crate::render::Style) -> Node {
    Node::row()
        .id(widget.id())
        .style(style)
        .height(Length::Cells(1))
        .shrink(0.0)
        .gap(Space::TIGHT)
        // Unstyled: both leaves inherit the button's own pen, so the
        // verb and its cap are read against the fill the button is
        // actually drawn in rather than against the screen behind it.
        .child(Node::text(verb).height(Length::Cells(1)).shrink(0.0))
        .child(
            Node::row()
                .height(Length::Cells(1))
                .shrink(0.0)
                .child(Node::text(cap).height(Length::Cells(1)).shrink(0.0)),
        )
}

/// The keyboard input for a conditional approval.
/// The modal routes text here without a clickable widget ID. Its caret is
/// the only animation while the approval card is open.
fn condition_row(model: &Model, inner: usize, pen: &Pen) -> Node {
    let columns = u16::try_from(inner.saturating_sub(2)).unwrap_or(u16::MAX);
    let mut row = Node::row()
        .style(pen.prompt.clone())
        .height(Length::Cells(1))
        .gap(Space::TIGHT)
        .child(segment("›", pen.prompt_mark.clone()));
    if model.condition().is_empty() {
        return row
            .child(segment(
                &clip(CONDITION_HINT, inner.saturating_sub(4)),
                pen.thought.clone(),
            ))
            .child(segment(CURSOR_BLANK, pen.cursor.clone()));
    }
    let rows = model.condition().window_rows(columns, 1);
    if let Some((before, at, after)) = rows.into_iter().next() {
        row = row
            .child(segment(&before, pen.prompt.clone()))
            .child(segment(&at, pen.cursor.clone()))
            .child(segment(&after, pen.prompt.clone()));
    }
    row
}

// ── The Brief and the rest state ───────────

/// The Needs-you dock: a quiet labelled divider when nothing waits —
/// the panel grammar without the panel — else the needs-you panel in
/// the focus accent, a row per card; the front card carries its
/// buttons, the cards behind it wait as text until they reach the
/// front.
fn dock(model: &Model, pen: &Pen) -> Node {
    if model.dock().is_empty() {
        let label = "nothing needs you";
        let fill = usize::from(model.size().cols).saturating_sub(label.chars().count() + 4);
        return Node::row()
            .id(Widget::Dock.id())
            .style(pen.stage.clone())
            .height(Length::Cells(1))
            .child(segment("── ", pen.panel_border.clone()))
            .child(segment(label, pen.rail.clone()))
            .child(segment(
                &format!(" {}", "─".repeat(fill)),
                pen.panel_border.clone(),
            ));
    }
    // A card's arrival flashes the ring and pulses the front card once.
    let arrival = in_motion(model).and_then(|(m, now)| m.dock_transition(now));
    let ring = match arrival {
        Some(t) => pen.shimmer(pen.panel(true), t),
        None => pen.panel(true).clone(),
    };
    let mut column = Node::column()
        .id(Widget::Dock.id())
        .style(pen.permission.clone())
        .frame(
            Frame::new(ring)
                .title("needs you")
                .title_style(pen.heading.clone())
                .meta(model.dock().len().to_string()),
        );
    // A card whose buttons the Moment is already showing keeps its row
    // and loses its buttons: two nodes carrying `Widget::Allow` would
    // put the same id in the hit registry twice, and the strip is a
    // *list* of what needs the human while the Moment is the place the
    // answer is given.
    let front = model.moment().is_none();
    for (index, card) in model.dock().cards().enumerate() {
        let pulse = arrival.filter(|_| index == 0);
        column = column.child(card_row(card, front && index == 0, pulse, pen));
    }
    column
}

/// One card's row: its text — pulsed once on arrival — then, on the
/// front card, one hittable button per action its type offers, each
/// with its accelerator.
fn card_row(card: &DockItem, front: bool, pulse: Option<f32>, pen: &Pen) -> Node {
    let text = match pulse {
        Some(t) => pen.pulse(&pen.permission, t),
        None => pen.permission.clone(),
    };
    let mut row = Node::row()
        .gap(Space::TIGHT)
        .height(Length::Cells(1))
        .child(
            Node::text(card.text(pen.icons))
                .style(text)
                .height(Length::Cells(1)),
        );
    if !front {
        return row;
    }
    match card {
        DockItem::Permission(p) => {
            for (kind, _) in p.buttons() {
                row = row.child(dock_button(
                    // The cap the sheet and the Moment's own buttons
                    // print, not the raw char: a reader must never be
                    // shown a capital they would not know to hold shift
                    // for.
                    &format!("[{}] {}", kind.key_cap(), kind.label()),
                    Widget::button(kind),
                    pen.button(kind).clone(),
                ));
            }
        }
        DockItem::Proposal { .. } => {
            row = row
                .child(dock_button(
                    &accel_label(Binding::ACCEPT),
                    Widget::Accept,
                    pen.allow.clone(),
                ))
                .child(dock_button(
                    &accel_label(Binding::REJECT),
                    Widget::Reject,
                    pen.deny.clone(),
                ));
        }
        DockItem::Blocker { .. } => {
            row = row.child(dock_button(
                &accel_label(Binding::CLEAR),
                Widget::Clear,
                pen.allow.clone(),
            ));
        }
    }
    row
}

/// One button on the dock's front card.
fn dock_button(text: &str, widget: Widget, style: crate::render::Style) -> Node {
    Node::text(text)
        .id(widget.id())
        .style(style)
        .shrink(0.0)
        .height(Length::Cells(1))
}

/// The prompt box: one row per visible line of the text — the mark on
/// the first row, a blank gutter on the continuations — with the cursor
/// cell (the grapheme under it, inverted) on its line, which scrolls so
/// the cursor is always on screen. The box grows with the lines up to
/// [`PROMPT_MAX_ROWS`], then slides to keep the cursor's line shown.
fn prompt_box(model: &Model, pen: &Pen) -> Node {
    let columns = model.size().cols.saturating_sub(PROMPT_MARK_COLUMNS);
    let rows = model.prompt().window_rows(columns, PROMPT_MAX_ROWS);
    let height = u16::try_from(rows.len()).unwrap_or(u16::MAX);
    let mut column = Node::column()
        .id(Widget::Prompt.id())
        .style(pen.prompt.clone())
        .height(Length::Cells(height));
    for (index, (before, at, after)) in rows.into_iter().enumerate() {
        let mark = if index == 0 {
            format!("{} ", pen.icons.prompt)
        } else {
            PROMPT_GUTTER.to_owned()
        };
        let mut line = Node::row()
            .height(Length::Cells(1))
            .child(Node::text(mark).style(pen.prompt_mark.clone()).shrink(0.0));
        if !before.is_empty() {
            line = line.child(Node::text(before).shrink(0.0));
        }
        if !at.is_empty() {
            let at = if at == " " {
                CURSOR_BLANK.to_owned()
            } else {
                at
            };
            line = line.child(Node::text(at).style(pen.cursor.clone()).shrink(0.0));
        }
        if !after.is_empty() {
            line = line.child(Node::text(after).height(Length::Cells(1)));
        }
        column = column.child(line);
    }
    column
}

/// The completion popup above the prompt: one row per candidate, the
/// selected one in the cursor's pen, each row hittable by its index.
fn completion_popup(model: &Model, pen: &Pen) -> Option<Node> {
    let completion = model.completion()?;
    let height = u16::try_from(completion.items().len()).unwrap_or(u16::MAX);
    let mut column = Node::column()
        .style(pen.footer.clone())
        .height(Length::Cells(height));
    for (index, item) in completion.items().iter().enumerate() {
        let style = if index == completion.selected() {
            pen.cursor.clone()
        } else {
            pen.footer.clone()
        };
        column = column.child(
            Node::text(item.as_str())
                .id(Widget::completion(index))
                .style(style)
                .height(Length::Cells(1)),
        );
    }
    Some(column)
}

/// The header bar: the brand slot, the session title when one is set,
/// the turn's state in its own semantic pen — behind a spinner that
/// turns while the agent talks — where the window is while
/// scrolled, and the session clock pushed to the right edge — one
/// designed bar on the chrome surface. The size and frame counter join
/// the clock in debug.
fn header_bar(model: &Model, clock: &FrameClock, pen: &Pen) -> Node {
    let mut bar = Node::row()
        .id(Widget::Header.id())
        .style(pen.footer.clone())
        .gap(Space::GAP)
        .height(Length::Cells(1));
    bar = bar.child(brand_slot(model, pen));
    if let Some(title) = model.title() {
        bar = bar.child(segment(title, pen.permission.clone()));
    }
    let state = model.turn_state();
    let (state_text, state_pen) = if state == TurnState::Streaming {
        // The header's spinner turns on the runtime's tick, which runs
        // while this is true; under reduced motion it never starts, and
        // the still running mark stands in for the spinner.
        let mark = if model.animates(Animation::Motion) {
            pen.icons.spin(model.spin_step())
        } else {
            pen.icons.running
        };
        (format!("{mark} {}", state.label()), pen.always.clone())
    } else {
        (state.label().to_owned(), pen.footer.clone())
    };
    bar = bar.child(segment(&state_text, state_pen));
    let scrolled = scrolled(model);
    if !scrolled.is_empty() {
        bar = bar.child(segment(&scrolled, pen.footer.clone()));
    }
    bar = bar.child(Node::text("").grow(1.0).height(Length::Cells(1)));
    if let Some(spark) = activity_spark(model, pen) {
        bar = bar.child(spark);
    }
    let mut clock_text = clock_text(clock);
    if model.is_debug() {
        let size = model.size();
        clock_text = format!(
            "{}×{} · frame {} · {clock_text}",
            size.cols,
            size.rows,
            clock.frame()
        );
    }
    bar.child(segment(&clock_text, pen.footer.clone()))
}

/// Cells the header's activity sparkline takes.
pub const SPARK_COLS: u16 = 8;

/// The header's brand slot: the mark, then the product name. With the
/// pixel layer live the mark's cell is left blank for
/// [`Brand`](crate::pixel::Brand) to cover — the driver places the
/// surface over the slot's first cell; otherwise the cell carries the
/// set's brand glyph. The slot is the same width either way, so the
/// bar's layout does not move when the probe answers.
fn brand_slot(model: &Model, pen: &Pen) -> Node {
    let mark = if model.has_pixels() {
        " "
    } else {
        pen.icons.brand
    };
    segment(&format!("{mark} magistr"), pen.header.clone()).id(Widget::HeaderBrand.id())
}

/// The header's activity monitor: what the session has been spending
/// ([`Model::monitor`]) — tokens per turn once the agent reports its
/// usage, how busy each of the last painted frames was until then. With
/// the pixel layer live the cells are blank and
/// [`Sparkline`](crate::pixel::Sparkline) draws the series at sub-cell
/// resolution; otherwise the same series is a ramp of block cells. The
/// slot is [`SPARK_COLS`] wide either way, and both renderings scale to
/// the series' own peak, so which series is live changes the line's
/// shape and nothing about its size.
///
/// `None` until something has happened: a session that has received
/// nothing has no activity to monitor, and a flat line claiming
/// otherwise would be furniture. It is the same rule the strip's
/// `iteration` and gate LEDs follow — the slot arrives with its news.
fn activity_spark(model: &Model, pen: &Pen) -> Option<Node> {
    let series = activity_series(model);
    if series.iter().all(|&v| v <= 0.0) {
        return None;
    }
    let text = if model.has_pixels() {
        " ".repeat(usize::from(SPARK_COLS))
    } else {
        spark_cells(&series, pen.icons, spark_sweep(model))
    };
    Some(
        Node::text(text)
            .id(Widget::HeaderSpark.id())
            .style(pen.note.clone())
            .width(Length::Cells(SPARK_COLS))
            .height(Length::Cells(1))
            .shrink(0.0),
    )
}

/// The monitor's series as the header shows it ([`Model::monitor`] —
/// tokens per turn, or the busy-frame count): the newest
/// [`SPARK_COLS`] samples, padded at the front with quiet frames so a
/// young session's line still starts at the left edge.
pub fn activity_series(model: &Model) -> Vec<f32> {
    let width = usize::from(SPARK_COLS);
    let samples = model.monitor();
    let skip = samples.len().saturating_sub(width);
    let tail: Vec<f32> = samples.skip(skip).map(|n| n as f32).collect();
    let mut series = vec![0.0; width - tail.len()];
    series.extend(tail);
    series
}

/// How far the header's activity monitor's sweep has come this frame —
/// `None` when it is settled, and always under reduced motion. Both
/// renderings of the monitor take it, so the pixel line and the cell
/// ramp draw themselves in together.
pub fn spark_sweep(model: &Model) -> Option<f32> {
    let (motion, now) = in_motion(model)?;
    motion.spark_sweep(usize::from(SPARK_COLS), now)
}

/// A series as one block cell each, scaled to its own maximum — the
/// sparkline's cell rendering.
///
/// `sweep` draws it in from the left on the same curve the pixel line
/// uses, cutting between whole cells rather than interpolating a head:
/// a column is drawn once the sweep's edge has passed its left side,
/// and blank until then. The slot is the same width throughout, so
/// nothing moves — and a settled monitor (`None`) is exactly the string
/// it always was, which is what leaves every PNG golden where it is.
fn spark_cells(series: &[f32], icons: &Icons, sweep: Option<f32>) -> String {
    let peak = series.iter().copied().fold(0.0_f32, f32::max);
    let edge = sweep.map_or(f32::INFINITY, |t| t * series.len() as f32);
    series
        .iter()
        .enumerate()
        .map(|(col, &v)| {
            if (col as f32) < edge {
                icons.spark(if peak > 0.0 { v / peak } else { 0.0 })
            } else {
                " "
            }
        })
        .collect()
}

/// The session clock: minutes and seconds since the cockpit started.
fn clock_text(clock: &FrameClock) -> String {
    Copy::clock(clock.elapsed().as_secs())
}

/// `rows a–b/n` while the window is not at the tail.
fn scrolled(model: &Model) -> String {
    if model.viewport().is_following() {
        return String::new();
    }
    let top = model.top_row();
    let shown = model.visible_rows().len();
    format!("rows {}–{}/{}", top + 1, top + shown, model.turn().len())
}

/// The bindings the footer shows now: the context follows the model's
/// focus, from the open sheet down to the idle prompt.
fn hint_context(model: &Model) -> &'static [Binding] {
    let running_loop = model.loop_status().filter(|s| s.ended().is_none());
    if model.is_sheet_open() {
        Binding::HINT_SHEET
    } else if model.completion().is_some() {
        Binding::HINT_COMPLETION
    } else if let Some(front) = model.dock().front() {
        match front {
            DockItem::Permission(_) => Binding::HINT_PERMISSION,
            DockItem::Proposal { .. } => Binding::HINT_PROPOSAL,
            DockItem::Blocker { .. } => Binding::HINT_BLOCKER,
        }
    } else if model.tab() == StageTab::Changes && model.changes().viewer().is_some()
        || model.tab() == StageTab::Trace && model.trace().viewer().is_some()
    {
        Binding::HINT_VIEWER
    } else if running_loop.is_some_and(LoopStatus::is_paused) {
        Binding::HINT_PAUSED
    } else if running_loop.is_some() {
        if model.turn_state() == TurnState::Streaming {
            Binding::HINT_LOOP_STREAMING
        } else {
            Binding::HINT_LOOP_IDLE
        }
    } else if model.turn_state() == TurnState::Streaming {
        Binding::HINT_STREAMING
    } else if model.tab() == StageTab::Home {
        Binding::HINT_HOME
    } else {
        Binding::HINT_IDLE
    }
}

/// The columns one chip costs: the cap's own two spaces around the
/// keys, the tight gap, and the label. Measured through [`width`], the
/// same per-grapheme measure the layout gives the text nodes, so the
/// arithmetic agrees with where the renderer will actually put them.
fn chip_cols(binding: &Binding) -> usize {
    usize::from(width(binding.keys))
        + 2
        + usize::from(Space::TIGHT)
        + usize::from(width(binding.does))
}

/// How many of the context's chips fit whole in `cols`.
///
/// The footer clips rather than squeezes, so a chip the bar only half
/// reaches would show its raised cap with the label beside it cut off —
/// at 80 columns the Home context ended in a bare `ctrl-d`. Every
/// `HINT_*` slice is priority-ordered, most important first, so
/// stopping at the first chip that does not fit whole keeps the
/// bindings that matter and drops the tail.
///
/// The rule is uniform: when not even the first chip fits, the answer
/// is zero and the bar is empty, rather than one clipped chip smuggled
/// back in. A screen wide enough for the whole context answers
/// `context.len()`, which is what leaves every wide surface exactly as
/// it was.
fn footer_chips(context: &[Binding], cols: usize) -> usize {
    let mut used = 0;
    for (index, binding) in context.iter().enumerate() {
        let gap = if index == 0 {
            0
        } else {
            usize::from(Space::GAP)
        };
        let want = used + gap + chip_cols(binding);
        if want > cols {
            return index;
        }
        used = want;
    }
    context.len()
}

/// The footer bar: the context's bindings as key-cap chips — each key
/// on a raised cap with what it does beside it — eliding whole chips
/// from the tail on a narrow screen instead of squeezing or cutting one
/// in half ([`footer_chips`]); the focus and the terminal's answers
/// join the bar in debug.
fn footer_bar(model: &Model, pen: &Pen) -> Node {
    let mut bar = Node::row()
        .id(Widget::Footer.id())
        .style(pen.footer.clone())
        .gap(Space::GAP)
        .viewport()
        .min_width(Length::Cells(0))
        .shrink(0.0)
        .height(Length::Cells(1));
    let context = hint_context(model);
    let fits = footer_chips(context, usize::from(model.size().cols));
    for (index, binding) in context.iter().take(fits).enumerate() {
        bar = bar.child(
            Node::row()
                // Its own id, so the pointer raises the one chip it is
                // over rather than the whole bar.
                .id(Widget::footer_chip(index))
                .gap(Space::TIGHT)
                .shrink(0.0)
                .height(Length::Cells(1))
                .child(segment(
                    &format!(" {} ", binding.keys),
                    pen.chip_key.clone(),
                ))
                .child(segment(binding.does, pen.footer.clone())),
        );
    }
    if !model.is_debug() {
        return bar;
    }
    let summary = Summary::of(model);
    let cell = summary
        .cell()
        .map_or(String::new(), |cell| format!(" · {cell}"));
    bar.child(segment(
        &format!(
            "focus {} · theme {} · background {} · keyboard {}{cell}",
            if model.is_focused() { "yes" } else { "no" },
            summary.theme(),
            summary.background(),
            summary.keyboard()
        ),
        pen.footer.clone(),
    ))
}

#[cfg(test)]
#[path = "view.tests.rs"]
mod tests;

// ── The review ────────────────────────────
