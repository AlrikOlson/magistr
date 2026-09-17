use super::*;
use crate::cockpit::StageTab;

/// The controls the update loop actually acts on all answer the pointer
/// as buttons — the tab bar, the rail's toggle, every dock button, the
/// trace and roadmap actions, the launcher's actions.
#[test]
fn every_named_control_lifts() {
    let controls = [
        Widget::TabTurn,
        Widget::TabSession,
        Widget::TabRoadmap,
        Widget::ToolOpen,
        Widget::ToolUndo,
        Widget::RailToggle,
        Widget::Allow,
        Widget::Always,
        Widget::Deny,
        Widget::Never,
        Widget::Accept,
        Widget::Reject,
        Widget::Clear,
        Widget::Checkpoint,
        Widget::RoadmapStart,
        Widget::RoadmapComplete,
        Widget::RoadmapRequest,
        Widget::RoadmapRequests,
        Widget::RoadmapDetails,
        Widget::RequestLatest,
        Widget::LaunchNext,
        Widget::LaunchLoop,
        Widget::LaunchGates,
        Widget::LaunchRoadmap,
        Widget::LatestDecision,
        Widget::ReasoningToggle,
        Widget::TracePrevious,
        Widget::TraceNext,
        Widget::PromptFiles,
        Widget::Commands,
        Widget::SettingsClose,
        Widget::SettingsOpenFile,
    ];
    for widget in controls {
        assert_eq!(
            HoverRole::of(widget.id()),
            HoverRole::Lift,
            "{widget:?} is a control"
        );
    }
    for tab in StageTab::ALL {
        assert_eq!(HoverRole::of(Widget::tab(tab).id()), HoverRole::Lift);
    }
}

/// The rows you click to open, select or resume raise as a band; the
/// buttons that sit on them lift; a revision link underlines.
#[test]
fn the_indexed_spaces_split_into_rows_buttons_and_one_link() {
    let rows = [
        Widget::stage_row(0),
        Widget::change_row(3),
        Widget::trace_row(2),
        Widget::roadmap_row(11),
        Widget::completion(1),
        Widget::launch_session(0),
    ];
    for id in rows {
        assert_eq!(HoverRole::of(id), HoverRole::Surface, "{id:?} is a row");
    }
    let buttons = [
        Widget::edit_open(0),
        Widget::edit_undo(0),
        Widget::change_open(1),
        Widget::change_undo(1),
        Widget::change_approve(1),
        Widget::trace_pin(7),
        Widget::proposal_accept(0),
        Widget::proposal_reject(0),
        Widget::signal_promote(2),
        Widget::signal_ignore(2),
        Widget::signal_snooze(2),
        Widget::footer_chip(4),
    ];
    for id in buttons {
        assert_eq!(HoverRole::of(id), HoverRole::Lift, "{id:?} is a button");
    }
    assert_eq!(HoverRole::of(Widget::trace_link(12)), HoverRole::Link);
    assert_eq!(
        HoverRole::of(Widget::roadmap_dependency(12)),
        HoverRole::Link
    );
}

/// Structure stays quiet: tinting a whole panel because the pointer
/// crossed its padding would be noise. The two graph blocks are inert
/// here on purpose — the tech-tree runs its own node-level hover.
#[test]
fn the_panels_bars_and_graph_blocks_are_inert() {
    let structure = [
        Widget::Header,
        Widget::HeaderBrand,
        Widget::HeaderSpark,
        Widget::Status,
        Widget::Iteration,
        Widget::Research,
        Widget::Plan,
        Widget::Execute,
        Widget::Verify,
        Widget::Gates,
        Widget::Elapsed,
        Widget::Chunk,
        Widget::Rail,
        Widget::RailGraph,
        Widget::RailGauge,
        Widget::Stage,
        Widget::Undock,
        Widget::Dock,
        Widget::TraceGraph,
        Widget::RoadmapGraph,
        Widget::Prompt,
        Widget::Footer,
    ];
    for widget in structure {
        assert_eq!(
            HoverRole::of(widget.id()),
            HoverRole::Inert,
            "{widget:?} is chrome"
        );
    }
}

/// Every named widget is classified, and the two sets together are the
/// whole enum — a widget added without a role decision shows up here.
#[test]
fn every_named_widget_is_either_a_control_or_chrome() {
    let lifting = Widget::ALL
        .into_iter()
        .filter(|w| HoverRole::of(w.id()) == HoverRole::Lift)
        .count();
    let linking = Widget::ALL
        .into_iter()
        .filter(|w| HoverRole::of(w.id()) == HoverRole::Link)
        .count();
    let inert = Widget::ALL
        .into_iter()
        .filter(|w| HoverRole::of(w.id()) == HoverRole::Inert)
        .count();
    assert_eq!(
        lifting + linking + inert + 2,
        Widget::ALL.len(),
        "the divider and the dock's grip are the two named widgets that raise; nothing else escapes"
    );
    assert_eq!(
        HoverRole::of(Widget::TermDockGrip.id()),
        HoverRole::Surface,
        "the dock's grip is a drag handle like the divider (DESIGN.md §6.2)"
    );
    assert_eq!(
        HoverRole::of(Widget::TermDock.id()),
        HoverRole::Inert,
        "the docked screen is the wheel's target, not a control"
    );
    assert_eq!(
        lifting, 65,
        "the controls the update loop acts on, the Galaxy's ＋ New project, the bad day's five verbs, Birth's nine, the ledger head's four, the meter's Raise, recorder navigation, Add files, Commands, Settings, Request a change, Requests, Item details, Latest reply, Reset view and Reasoning Show/Hide controls"
    );
    assert_eq!(
        linking,
        Widget::CRUMBS.len() + 3,
        "the altitude path's crumbs (gui2-2b3b), the Brief's dismiss, the identity band's work chip and Linked work"
    );
    assert_eq!(
        HoverRole::of(Widget::RailDivider.id()),
        HoverRole::Surface,
        "the drag handle raises as a bar rather than lifting a space"
    );
}

/// The pointer is a hand over what can be clicked and nothing else.
#[test]
fn only_buttons_and_links_ask_for_a_hand() {
    assert!(HoverRole::Lift.is_clickable());
    assert!(HoverRole::Link.is_clickable());
    assert!(!HoverRole::Surface.is_clickable());
    assert!(!HoverRole::Inert.is_clickable());
}

#[test]
fn a_chunk_jump_is_a_link_like_a_revision_link() {
    assert_eq!(HoverRole::of(Widget::trace_chunk(12)), HoverRole::Link);
}

/// The rail's verbs: a row's Restore and
/// Fork and the meter's Raise lift like every button; a charter row
/// raises like the checkpoint row beside it — a surface, not a verb,
/// until the delegation dial lands.
#[test]
fn the_rails_verbs_lift_and_its_rows_raise() {
    assert_eq!(HoverRole::of(Widget::spine_restore(3)), HoverRole::Lift);
    assert_eq!(HoverRole::of(Widget::spine_fork(3)), HoverRole::Lift);
    assert_eq!(HoverRole::of(Widget::BudgetRaise.id()), HoverRole::Lift);
    assert_eq!(HoverRole::of(Widget::spine_row(3)), HoverRole::Surface);
    assert_eq!(HoverRole::of(Widget::charter_row(1)), HoverRole::Surface);
    assert_eq!(HoverRole::of(Widget::charter_offer(0)), HoverRole::Lift);
}

/// The identity band's chunk chip navigates — it rises to the chunk's
/// star on the Sky — so it answers as a link, like other
/// navigation chip, like the header's crumbs.
#[test]
fn the_chunk_chip_answers_as_a_link() {
    assert_eq!(HoverRole::of(Widget::SessionChunk.id()), HoverRole::Link);
}
