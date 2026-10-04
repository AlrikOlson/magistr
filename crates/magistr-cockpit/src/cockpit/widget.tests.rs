use super::*;

#[test]
fn ids_are_distinct_and_round_trip() {
    for (i, widget) in Widget::ALL.into_iter().enumerate() {
        assert_eq!(widget.id(), WidgetId::new(u32::try_from(i).expect("small")));
        assert_eq!(Widget::from_id(widget.id()), Some(widget));
    }
    assert_eq!(
        Widget::from_id(WidgetId::new(Widget::ALL.len() as u32)),
        None
    );
}

#[test]
fn the_phase_rail_reads_left_to_right() {
    let labels: Vec<&str> = Widget::PHASES.iter().map(|w| w.label()).collect();
    assert_eq!(labels, ["Research", "Plan", "Execute", "Verify"]);
}

#[test]
fn every_permission_kind_has_a_button_and_only_buttons_answer() {
    for kind in PermissionKind::ALL {
        let button = Widget::button(kind);
        assert_eq!(button.answers(), Some(kind));
        assert_eq!(button.label(), kind.label());
        assert!(Widget::DOCK_BUTTONS.contains(&button));
    }
    for widget in Widget::ALL {
        assert_eq!(
            widget.answers().is_some(),
            Widget::DOCK_BUTTONS.contains(&widget)
                && !matches!(widget, Widget::Accept | Widget::Reject | Widget::Clear),
            "{widget:?}"
        );
    }
}

#[test]
fn launch_session_ids_live_outside_the_enum_space() {
    assert_ne!(Widget::launch_session(0), Widget::launch_session(1));
    assert_eq!(Widget::from_id(Widget::launch_session(0)), None);
}

#[test]
fn every_tab_has_a_widget_and_only_tabs_target() {
    for tab in StageTab::ALL {
        let widget = Widget::tab(tab);
        assert_eq!(widget.tab_target(), Some(tab));
        assert_eq!(widget.label(), tab.label());
    }
    // Only the bar's tabs and the launcher's roadmap button (a tab
    // switch by design) target a tab; the native header's crumbs
    // target a place.
    for widget in Widget::ALL {
        assert_eq!(
            widget.tab_target().is_some(),
            matches!(
                widget,
                Widget::TabTurn | Widget::TabSession | Widget::TabRoadmap | Widget::LaunchRoadmap
            ),
            "{widget:?}"
        );
    }
}

/// The crumbs are the altitude path and nothing else moves the camera
/// by widget: `projects` is the Galaxy, the project the Sky, the
/// session the Session, and `^` one altitude up — holding at the top.
#[test]
fn only_crumbs_target_a_place_and_up_holds_at_the_galaxy() {
    for widget in Widget::ALL {
        assert_eq!(
            widget.place_target(Place::Session).is_some(),
            Widget::CRUMBS.contains(&widget),
            "{widget:?}"
        );
    }
    assert_eq!(
        Widget::CrumbProjects.place_target(Place::Trace),
        Some(Place::Galaxy)
    );
    assert_eq!(
        Widget::CrumbProject.place_target(Place::Trace),
        Some(Place::Sky)
    );
    assert_eq!(
        Widget::CrumbSession.place_target(Place::Sky),
        Some(Place::Session)
    );
    for at in Place::ALL {
        assert_eq!(
            Widget::CrumbUp.place_target(at),
            Some(at.up().unwrap_or(at)),
            "{at:?}"
        );
        // The record stop is where you already are: pressing it holds.
        assert_eq!(Widget::CrumbRecord.place_target(at), Some(at), "{at:?}");
    }
    assert!(Widget::CRUMBS.contains(&Widget::CrumbRecord));
}

#[test]
fn a_chunk_link_has_its_own_space_beside_the_revision_links() {
    // Both spaces are keyed by the step's number, so the same step's
    // revision link and chunk link must not collide — and neither may
    // reach the enum's ids.
    for number in [0, 1, 9, 0xFFFF] {
        let id = Widget::trace_chunk(number);
        assert_ne!(id, Widget::trace_link(number));
        assert_eq!(Widget::trace_chunk_step(id), Some(number));
        assert_eq!(Widget::trace_link_step(id), None);
        assert_eq!(Widget::trace_pin_step(id), None);
        assert_eq!(Widget::from_id(id), None);
    }
}

#[test]
fn detail_links_round_trip_without_colliding_with_rows_or_step_links() {
    for index in [0, 1, 17, 0xFFFF] {
        let id = Widget::detail_link(index);
        assert_eq!(Widget::detail_link_index(id), Some(index));
        assert_eq!(Widget::from_id(id), None);
        assert_eq!(Widget::roadmap_row_index(id), None);
        assert_eq!(Widget::trace_link_step(id), None);
        assert_eq!(Widget::session_row_index(id), None);
        assert_eq!(Widget::detail_link_index(Widget::roadmap_row(index)), None);
    }
}

/// A cluster on the Galaxy has its own id space: it round-trips, and
/// it is never mistaken for a star on the Sky or the other way round.
#[test]
fn galaxy_cluster_ids_round_trip_in_their_own_space() {
    assert_eq!(
        Widget::galaxy_cluster_index(Widget::galaxy_cluster(7)),
        Some(7)
    );
    assert_eq!(Widget::sky_star_index(Widget::galaxy_cluster(7)), None);
    assert_eq!(Widget::galaxy_cluster_index(Widget::sky_star(7)), None);
    assert_eq!(Widget::galaxy_cluster_index(Widget::GalaxyField.id()), None);
}
