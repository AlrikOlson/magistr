//! How a widget answers the pointer resting on it.

use super::widget::Widget;
use crate::widget_id::WidgetId;

/// The hover treatment a widget wears while the pointer is over it.
/// One role per *kind* of hittable, not one per widget, so
/// the cockpit answers the pointer in three moves rather than forty:
///
/// - [`HoverRole::Surface`] — a row you click to open, select or resume,
///   and the rail's drag handle: raised onto the second surface as a
///   band.
/// - [`HoverRole::Lift`] — a chip, a tab or a `[Button]`:
///   its colour lifted toward the bright extreme and bolded, the crest
///   of the tab-switch pulse held for as long as the pointer stays.
/// - [`HoverRole::Link`] — a revision link on an expanded trace step:
///   lifted and underlined, so it reads as a link rather than a button.
/// - [`HoverRole::Inert`] — structure that is registered because the
///   layout gives every node an id, not because it is a control: the
///   panels, the bars, the two pixel graph blocks (the tech-tree runs
///   its own node-level hover through
///   [`Roadmap::hover_at`](super::Roadmap::hover_at)), the prompt line
///   and the launcher's cards. Tinting a whole panel because the
///   pointer crossed its padding would be noise, not feedback.
///
/// `view.tests.rs` sweeps every id the hit registry registers on every
/// surface against this partition: an interactive id must change a
/// painted cell, an inert one is named in the test's own list. A widget
/// added without a role decision fails that sweep rather than silently
/// going quiet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HoverRole {
    /// A row or a handle: raised onto the hover ground.
    Surface,
    /// A chip, tab or button: colour lifted, bolded.
    Lift,
    /// A link: lifted and underlined.
    Link,
    /// Structure: no answer.
    Inert,
}

impl HoverRole {
    /// The role `id` answers the pointer with.
    #[must_use]
    pub fn of(id: WidgetId) -> Self {
        if let Some(widget) = Widget::from_id(id) {
            return Self::of_widget(widget);
        }
        if Widget::trace_link_step(id).is_some()
            || Widget::trace_chunk_step(id).is_some()
            || Widget::detail_link_index(id).is_some()
        {
            return Self::Link;
        }
        if Widget::stage_row_index(id).is_some()
            || Widget::change_row_index(id).is_some()
            || Widget::trace_row_index(id).is_some()
            || Widget::session_row_index(id).is_some()
            || Widget::roadmap_row_index(id).is_some()
            || Widget::roadmap_track_index(id).is_some()
            || Widget::signal_row_index(id).is_some()
            || Widget::completion_index(id).is_some()
            || Widget::palette_row_index(id).is_some()
            || Widget::search_row_index(id).is_some()
            || Widget::settings_nav_index(id).is_some()
            || Widget::launch_session_index(id).is_some()
            || Widget::spine_row_index(id).is_some()
            // A charter row raises like the checkpoint row beside it:
            // a surface, not a verb.
            || Widget::charter_row_index(id).is_some()
        {
            return Self::Surface;
        }
        if id == Widget::detail_copy()
            || Widget::detail_section_index(id).is_some()
            || Widget::edit_open_index(id).is_some()
            || Widget::edit_undo_index(id).is_some()
            || Widget::change_open_index(id).is_some()
            || Widget::change_undo_index(id).is_some()
            || Widget::change_approve_index(id).is_some()
            || Widget::trace_pin_step(id).is_some()
            || Widget::proposal_accept_index(id).is_some()
            || Widget::proposal_reject_index(id).is_some()
            || Widget::signal_promote_index(id).is_some()
            || Widget::signal_ignore_index(id).is_some()
            || Widget::signal_snooze_index(id).is_some()
            || Widget::signal_research_index(id).is_some()
            || Widget::footer_chip_index(id).is_some()
            || Widget::search_scope_index(id).is_some()
            || Widget::settings_theme_index(id).is_some()
            || Widget::settings_switch_index(id).is_some()
            || Widget::settings_density_index(id).is_some()
            // A checkpoint row's revealed verbs, and
            // the charter offer's Delegate.
            || Widget::spine_restore_index(id).is_some()
            || Widget::spine_fork_index(id).is_some()
            || Widget::charter_offer_index(id).is_some()
            // Birth's chips are picks and its rows are grabbed: both
            // lift under the pointer.
            || Widget::birth_chip_index(id).is_some()
            || Widget::birth_row_index(id).is_some()
        {
            return Self::Lift;
        }
        Self::Inert
    }

    /// Whether the pointer over this role should be a hand — the shape
    /// [`PointerShape`](super::PointerShape) asks kitty for.
    #[must_use]
    pub const fn is_clickable(self) -> bool {
        matches!(self, Self::Lift | Self::Link)
    }

    /// The role a named widget answers with.
    const fn of_widget(widget: Widget) -> Self {
        match widget {
            // The tab bar, the rail's toggle and every `[Button]` on a
            // card: the pointer lifts them.
            Widget::PromptAction
            | Widget::ReasoningToggle
            | Widget::LatestDecision
            | Widget::TracePrevious
            | Widget::TraceNext
            | Widget::PromptFiles
            | Widget::Commands
            | Widget::TabTurn
            | Widget::TabSession
            | Widget::TabRoadmap
            | Widget::RailToggle
            | Widget::Allow
            | Widget::Always
            | Widget::Deny
            | Widget::Never
            | Widget::ShowImages
            | Widget::Accept
            | Widget::Reject
            | Widget::Clear
            | Widget::Checkpoint
            | Widget::TraceDetail
            | Widget::ToolOpen
            | Widget::ToolUndo
            | Widget::InspectorDetail
            | Widget::RoadmapTracks
            | Widget::RoadmapDependencies
            | Widget::RoadmapTrackFilter
            | Widget::RoadmapSearch
            | Widget::RoadmapSearchClear
            | Widget::RoadmapReveal
            | Widget::RoadmapFilter
            | Widget::RoadmapSortName
            | Widget::RoadmapSortStatus
            | Widget::RoadmapSortPriority
            | Widget::RoadmapSortWorkstream
            | Widget::RoadmapRequest
            | Widget::RequestLatest
            | Widget::RoadmapRequests
            | Widget::RoadmapDetails
            | Widget::RoadmapReset
            | Widget::RoadmapStart
            | Widget::RoadmapComplete
            | Widget::LaunchNext
            | Widget::LaunchLoop
            | Widget::LaunchGates
            | Widget::LaunchRoadmap
            // The Brief's three raised verbs: buttons, so the pointer
            // lifts them like every other one.
            | Widget::BriefAnswer
            | Widget::BriefReview
            | Widget::BriefReports
            // The Galaxy's `＋ New project` is a raised button too, and
            // so is every Birth verb; the slot's wrapper lifts as the
            // `click to browse` label promises.
            | Widget::GalaxyNew
            | Widget::BirthSlot
            | Widget::BirthClone
            | Widget::BirthEmpty
            | Widget::BirthContinue
            | Widget::BirthAccept
            | Widget::BirthRedraw
            | Widget::BirthEdit
            | Widget::BirthClaim
            | Widget::BirthLater
            // The bad day's verbs are the cards' buttons (view 14).
            | Widget::FaultInstall
            | Widget::FaultDetails
            | Widget::FaultIncreaseLimit
            | Widget::FaultReceipts
            | Widget::FaultReconnect
            // The ledger head's actions are buttons with keyboard hints.
            | Widget::LoopPause
            | Widget::LoopStep
            | Widget::LoopCheckpoint
            | Widget::LoopAbort
            // The meter's Raise is the rail's one accent verb on the
            // budget.
            | Widget::BudgetRaise
            | Widget::SettingsClose
            | Widget::SettingsOpenFile => Self::Lift,
            // The native header's altitude path: each crumb
            // navigates, so it answers as a link — lifted and
            // underlined, like other navigation chips.
            Widget::CrumbUp
            | Widget::LinkedWork
            | Widget::CrumbProjects
            | Widget::CrumbProject
            | Widget::CrumbSession
            | Widget::CrumbRecord
            // The identity band's chunk chip provides navigation
            // on the cap: it rises to the chunk's star.
            | Widget::SessionChunk
            // The Brief's `esc · dismiss` is a navigation pair, not a
            // verb: it puts the overlay away rather than doing work, so
            // it reads as the crumbs do.
            | Widget::BriefDismiss => Self::Link,
            // The rail's drag handle is a bare one-column node: a lift
            // would only move a space's foreground, which is nothing to
            // look at. Raised, the column becomes a visible bar under
            // the pointer — the affordance a handle owes you.
            // The dock's grip is a drag handle like the divider:
            // raise the bar under the pointer.
            Widget::RailDivider | Widget::TermDockGrip | Widget::PanelColumns | Widget::PanelRoadmap | Widget::PanelReasoning | Widget::PanelGraph => Self::Surface,
            // Everything else a layout node happens to carry an id for
            // is chrome, not a control.
            _ => Self::Inert,
        }
    }
}

#[cfg(test)]
#[path = "hover_role.tests.rs"]
mod tests;
