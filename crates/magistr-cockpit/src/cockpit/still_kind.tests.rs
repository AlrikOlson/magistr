use std::path::Path;

use super::*;

#[test]
fn a_picture_under_a_motion_directory_is_a_filmstrip_and_the_rest_are_stills() {
    assert_eq!(
        StillKind::of(Path::new(
            "crates/magistr-tui/tests/golden/motion/gate-pop-mocha-80x24.png"
        )),
        StillKind::Filmstrip
    );
    assert_eq!(
        StillKind::of(Path::new(
            "crates/magistr-tui/tests/golden/screens/roadmap-mocha-80x24.png"
        )),
        StillKind::Still
    );
    assert_eq!(
        StillKind::of(Path::new(
            "crates/magistr-gui2/tests/golden/comp/sky-rest-night.png"
        )),
        StillKind::Still
    );
    // The word has to be a whole component: a file merely *named* motion
    // is a still.
    assert_eq!(
        StillKind::of(Path::new("tests/golden/screens/motion.png")),
        StillKind::Still
    );
}

#[test]
fn the_words_and_the_bundle_directories_are_the_gates_own() {
    assert_eq!(StillKind::Still.label(), "still");
    assert_eq!(StillKind::Filmstrip.label(), "filmstrip");
    assert_eq!(StillKind::Still.dir_name(), "stills");
    assert_eq!(StillKind::Filmstrip.dir_name(), "filmstrips");
}
