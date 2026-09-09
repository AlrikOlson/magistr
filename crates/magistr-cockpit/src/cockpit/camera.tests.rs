use super::*;

/// The limits a two-axis pan is clamped to in these tests: generous, so
/// a clamp is never the thing under test unless it is named.
const ROOM: (f32, f32) = (100.0, 100.0);

#[test]
fn a_fresh_camera_sits_at_the_origin_at_the_fit() {
    let camera = Camera::new();
    assert_eq!(camera.pan(), (0.0, 0.0));
    assert!((camera.zoom() - 1.0).abs() < f32::EPSILON);
    assert_eq!(camera.pan_at(0.0), (0.0, 0.0));
    assert!(!camera.is_gliding(0.0), "nothing has asked it to move");
}

#[test]
fn a_drag_lands_at_once_and_never_glides() {
    let mut camera = Camera::new();
    camera.drag(-4.0, -3.0, ROOM);
    assert_eq!(camera.pan(), (4.0, 3.0), "the view moves against the drag");
    assert_eq!(
        camera.pan_at(0.0),
        (4.0, 3.0),
        "the paint is the settled pan"
    );
    assert!(!camera.is_gliding(0.0));
}

#[test]
fn a_drag_clamps_to_the_room_the_overflow_leaves() {
    let mut camera = Camera::new();
    camera.drag(-500.0, 2.0, (10.0, 0.0));
    assert_eq!(camera.pan(), (10.0, 0.0));
}

#[test]
fn a_wheel_notch_keeps_the_point_under_the_pointer_and_lands_at_once() {
    let mut camera = Camera::new();
    // The grid point under screen x = 10 sits 10 - MARGIN into the
    // scaled grid; after the notch it must still sit under x = 10.
    camera.wheel(2.0, 10.0, 4.0, 1.5, ROOM);
    assert!((camera.zoom() - 2.0).abs() < 1e-5);
    let (px, py) = camera.pan();
    assert!((px - 8.5).abs() < 1e-4, "pan_x {px}");
    assert!((py - 2.5).abs() < 1e-4, "pan_y {py}");
    assert!(!camera.is_gliding(0.0), "a gesture is instant");
}

#[test]
fn a_wheel_notch_clamps_the_zoom_to_its_range() {
    let mut camera = Camera::new();
    camera.wheel(1000.0, 0.0, 0.0, 1.5, ROOM);
    assert!((camera.zoom() - Camera::MAX_ZOOM).abs() < 1e-5);
    camera.wheel(0.0001, 0.0, 0.0, 1.5, ROOM);
    assert!((camera.zoom() - Camera::MIN_ZOOM).abs() < 1e-5);
}

#[test]
fn a_glide_leaves_the_settled_camera_where_it_was_asked_and_eases_the_paint() {
    let mut camera = Camera::new();
    camera.glide_to((10.0, 6.0), 2.0, 0.0, true);
    assert_eq!(camera.pan(), (10.0, 6.0), "settled at once");
    assert!((camera.zoom() - 2.0).abs() < f32::EPSILON);
    // The paint starts back where the camera was and arrives over
    // Eased::STEPS.
    let (x0, y0) = camera.pan_at(0.0);
    assert!(x0.abs() < 1e-4 && y0.abs() < 1e-4, "starts at the old pan");
    assert!((camera.zoom_at(0.0) - 1.0).abs() < 1e-4);
    let mid = camera.pan_at((Eased::STEPS / 2) as f32);
    assert!(mid.0 > 0.0 && mid.0 < 10.0, "mid-glide {mid:?}");
    assert_eq!(camera.pan_at(Eased::STEPS as f32), (10.0, 6.0));
    assert!((camera.zoom_at(Eased::STEPS as f32) - 2.0).abs() < 1e-6);
}

#[test]
fn a_glide_runs_for_exactly_eased_steps() {
    let mut camera = Camera::new();
    camera.glide_to((10.0, 0.0), 1.0, 0.0, true);
    assert!(camera.is_gliding(0.0));
    assert!(camera.is_gliding((Eased::STEPS - 1) as f32));
    assert!(
        !camera.is_gliding(Eased::STEPS as f32),
        "over, so the tick stops"
    );
}

#[test]
fn a_glide_under_reduced_motion_cuts() {
    let mut camera = Camera::new();
    camera.glide_to((10.0, 6.0), 2.0, 0.0, false);
    assert_eq!(camera.pan_at(0.0), (10.0, 6.0));
    assert!((camera.zoom_at(0.0) - 2.0).abs() < f32::EPSILON);
    assert!(!camera.is_gliding(0.0), "nothing to run, so no tick");
}

#[test]
fn a_glide_to_where_the_camera_already_is_does_not_start_one() {
    let mut camera = Camera::new();
    camera.glide_to((0.0, 0.0), 1.0, 0.0, true);
    assert!(!camera.is_gliding(0.0));
}

#[test]
fn a_second_glide_starts_from_where_the_paint_is_not_from_the_last_target() {
    let mut camera = Camera::new();
    camera.glide_to((10.0, 0.0), 1.0, 0.0, true);
    let (here, _) = camera.pan_at(2.0);
    camera.glide_to((0.0, 0.0), 1.0, 2.0, true);
    let (restarted, _) = camera.pan_at(2.0);
    assert!(
        (restarted - here).abs() < 1e-4,
        "retargeting mid-glide snapped: {here} -> {restarted}"
    );
    assert_eq!(camera.pan_at((2 + Eased::STEPS) as f32), (0.0, 0.0));
}

#[test]
fn a_drag_mid_glide_moves_the_paint_one_for_one() {
    let mut camera = Camera::new();
    camera.glide_to((10.0, 0.0), 1.0, 0.0, true);
    let before = camera.pan_at(2.0).0;
    camera.drag(-3.0, 0.0, ROOM);
    let after = camera.pan_at(2.0).0;
    assert!(
        (after - before - 3.0).abs() < 1e-4,
        "the drag must land whole on top of the glide: {before} -> {after}"
    );
    assert!(
        camera.is_gliding(2.0),
        "and the glide keeps running under it"
    );
}

// ---------------------------------------------------------------------
// the keyboard's zoom — the wheel's arithmetic on the glide's
// machinery, since a key press has no pointer to keep a point under.
// ---------------------------------------------------------------------

#[test]
fn a_zoom_step_lands_the_settled_camera_where_the_wheel_would_have() {
    let mut wheeled = Camera::new();
    wheeled.wheel(1.5, 10.0, 4.0, 1.5, ROOM);
    let mut stepped = Camera::new();
    stepped.zoom_step(1.5, 10.0, 4.0, 1.5, 0.0, true);
    assert!((stepped.zoom() - wheeled.zoom()).abs() < 1e-5);
    let ((sx, sy), (wx, wy)) = (stepped.pan(), wheeled.pan());
    assert!((sx - wx).abs() < 1e-4, "pan_x {sx} vs {wx}");
    assert!((sy - wy).abs() < 1e-4, "pan_y {sy} vs {wy}");
}

#[test]
fn a_zoom_step_glides_the_paint_instead_of_cutting_to_the_new_scale() {
    let mut camera = Camera::new();
    camera.zoom_step(2.0, 10.0, 4.0, 1.5, 0.0, true);
    assert!(
        (camera.zoom_at(0.0) - 1.0).abs() < 1e-4,
        "the paint starts at the scale it was showing"
    );
    assert!(camera.is_gliding(0.0));
    assert!((camera.zoom_at(Eased::STEPS as f32) - 2.0).abs() < 1e-5);
    assert!(
        !camera.is_gliding(Eased::STEPS as f32),
        "and then it is over"
    );
}

#[test]
fn a_zoom_step_under_reduced_motion_is_one_frame() {
    let mut camera = Camera::new();
    camera.zoom_step(2.0, 10.0, 4.0, 1.5, 0.0, false);
    assert!((camera.zoom_at(0.0) - 2.0).abs() < 1e-5);
    assert!(!camera.is_gliding(0.0), "no residual, so no tick");
}

#[test]
fn a_zoom_step_clamps_to_the_same_range_the_wheel_does() {
    let mut camera = Camera::new();
    camera.zoom_step(1000.0, 0.0, 0.0, 1.5, 0.0, true);
    assert!((camera.zoom() - Camera::MAX_ZOOM).abs() < 1e-5);
    camera.zoom_step(0.0001, 0.0, 0.0, 1.5, 0.0, true);
    assert!((camera.zoom() - Camera::MIN_ZOOM).abs() < 1e-5);
}
