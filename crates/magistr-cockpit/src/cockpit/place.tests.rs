use super::*;

/// Esc climbs exactly one altitude, and the Galaxy is the top.
#[test]
fn up_climbs_one_altitude_and_the_galaxy_is_the_top() {
    for place in Place::ALL {
        match place.up() {
            // The inbox is altitude 1's second room: leaving it lands
            // in the first, the Sky, on the same altitude.
            Some(up) if place == Place::Signals => assert_eq!(up, Place::Sky),
            // The Report is a star opened: leaving it lands on the
            // constellation the star is on, not on a session it has
            // none of.
            Some(up) if place == Place::Report => assert_eq!(up, Place::Sky),
            // Birth was opened from the Galaxy and has no Sky of its
            // own yet: it climbs back to where it was opened from.
            Some(up) if place == Place::Birth => assert_eq!(up, Place::Galaxy),
            Some(up) => assert_eq!(
                up.altitude() + 1,
                place.altitude(),
                "{place:?} climbs to {up:?}"
            ),
            None => assert_eq!(place, Place::Galaxy, "only the Galaxy holds"),
        }
    }
    assert_eq!(Place::Changes.up(), Some(Place::Session));
    assert_eq!(Place::Trace.up(), Some(Place::Session));
    assert_eq!(Place::Report.up(), Some(Place::Sky));
    assert_eq!(Place::Sky.up(), Some(Place::Galaxy));
}

/// `ALL` walks the altitudes in order, and the default place is the
/// Session — what a model built for a turn opens on.
#[test]
fn all_is_in_altitude_order_and_the_default_is_the_session() {
    let altitudes: Vec<u8> = Place::ALL.iter().map(|p| p.altitude()).collect();
    assert_eq!(altitudes, [0, 1, 1, 1, 2, 3, 3, 3]);
    assert_eq!(Place::Birth.up(), Some(Place::Galaxy));
    assert_eq!(
        Place::Signals.up(),
        Some(Place::Sky),
        "the inbox leaves to the Sky"
    );
    assert_eq!(Place::default(), Place::Session);
}

/// Tab walks every place and comes back; Shift-Tab walks it back.
#[test]
fn next_and_prev_walk_every_place_and_wrap() {
    let mut place = Place::Galaxy;
    let mut walked = Vec::new();
    for _ in 0..Place::ALL.len() {
        walked.push(place);
        place = place.next();
    }
    assert_eq!(walked, Place::ALL);
    assert_eq!(place, Place::Galaxy, "and wraps");
    for place in Place::ALL {
        assert_eq!(place.next().prev(), place);
        assert_eq!(place.prev().next(), place);
    }
}

/// Every place draws a stage surface; every altitude below the Galaxy
/// round-trips through the tab it draws. The Galaxy stands in the
/// launcher's slot, and that slot does not round-trip on purpose: a
/// window *opened* on the Home tab opens the Sky, the room you open
/// into, not the top of the world.
#[test]
fn every_place_draws_a_surface_and_the_tabs_round_trip() {
    for place in Place::ALL {
        match place {
            Place::Galaxy => assert_eq!(place.tab(), StageTab::Home, "the launcher's slot"),
            // The inbox and the Report share the Roadmap tab's surface
            // (a terminal's inbox is a section of that tab, a shipped
            // chunk its row) and do not round-trip on purpose: a
            // window opened on the roadmap opens the Sky.
            Place::Signals | Place::Report | Place::Birth => {
                assert_eq!(
                    place.tab(),
                    StageTab::Roadmap,
                    "{place:?} draws the roadmap's surface"
                );
                assert_eq!(Place::of_tab(place.tab()), Place::Sky);
            }
            Place::Changes => assert_eq!(place.tab(), StageTab::Session),
            _ => assert_eq!(Place::of_tab(place.tab()), place, "{place:?}"),
        }
    }
    assert_eq!(
        Place::of_tab(StageTab::Home),
        Place::Sky,
        "a window opened on Home opens the Sky"
    );
    assert_ne!(
        Place::Galaxy.tab(),
        Place::Sky.tab(),
        "the Galaxy no longer draws the Sky's surface"
    );
    for tab in StageTab::ALL {
        let _ = Place::of_tab(tab);
    }
}
