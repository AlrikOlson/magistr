use super::*;

#[test]
fn the_cube_corners_and_the_greys_are_xterms_own() {
    assert_eq!(xterm256(16), ThemeColor::new(0, 0, 0));
    assert_eq!(xterm256(231), ThemeColor::new(255, 255, 255));
    assert_eq!(xterm256(196), ThemeColor::new(255, 0, 0), "cube red");
    assert_eq!(xterm256(46), ThemeColor::new(0, 255, 0), "cube green");
    assert_eq!(xterm256(21), ThemeColor::new(0, 0, 255), "cube blue");
    assert_eq!(xterm256(232), ThemeColor::new(8, 8, 8), "first grey");
    assert_eq!(xterm256(255), ThemeColor::new(238, 238, 238), "last grey");
}

#[test]
fn an_ansi_index_falls_to_the_cubes_first_entry() {
    assert_eq!(xterm256(0), ThemeColor::new(0, 0, 0));
    assert_eq!(xterm256(15), ThemeColor::new(0, 0, 0));
}
