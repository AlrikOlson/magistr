use super::*;

/// The event loop cannot be driven from a test — a winit window needs a
/// display server and macOS needs the main thread — so the wiring is
/// pinned in the source, the way `cmd_run_native` is pinned against
/// `cmd_run_cockpit`. What the *behaviour* is worth is proven where it
/// can be: `Frame::rescale`'s own tests measure the grid a 2x window
/// lands on.
const SOURCE: &str = include_str!("native.rs");

/// The body of `window_event`'s `ScaleFactorChanged` arm.
fn scale_arm() -> &'static str {
    let start = SOURCE
        .find("WindowEvent::ScaleFactorChanged")
        .expect("the loop handles a scale change");
    let rest = &SOURCE[start..];
    let end = rest
        .find("WindowEvent::RedrawRequested")
        .expect("the arm ends before the redraw arm");
    &rest[..end]
}

/// The first grid is taken from a face already drawn at the window's own
/// density, so the cockpit never flashes up at twice its column count
/// before a resize corrects it — there is no resize to correct it, since
/// the window opened at the size it asked for.
#[test]
fn the_first_frame_is_drawn_at_the_windows_own_scale_factor() {
    let resumed = SOURCE.split("fn resumed").nth(1).expect("the loop resumes");
    let rescale = resumed
        .find("self.frame.rescale(scale)")
        .expect("the face is drawn at the window's scale factor");
    let resize = resumed
        .find("self.resize(size.width, size.height)")
        .expect("the model is re-gridded to the window");
    assert!(
        rescale < resize,
        "the face is re-sized before the grid is taken from it"
    );
    assert!(resumed.contains("window.scale_factor()"));
}

/// A window dragged between displays re-grids: the face follows the new
/// density and the model takes the resize the cockpit already knows how
/// to fold.
#[test]
fn a_scale_change_re_sizes_the_face_and_folds_a_resize() {
    let arm = scale_arm();
    assert!(
        arm.contains("self.frame.rescale(*scale_factor)"),
        "the face follows the display"
    );
    assert!(
        arm.contains("self.resize("),
        "and the model is re-gridded through the one resize path"
    );
    assert!(
        SOURCE.contains("update(&mut self.model, Msg::Resize(size))"),
        "which is the fold the terminal runtime makes too"
    );
}

/// winit 0.30 will not be asked for a new size from inside the handler:
/// `request_inner_size` there panics (rust-windowing/winit#3704) and the
/// suggested size cannot be read back anyway (#3080), so the arm binds
/// the writer away and lets the `Resized` that follows carry the grid.
#[test]
fn the_scale_handler_never_writes_the_inner_size() {
    assert!(!SOURCE.contains("request_inner_size"));
    assert!(!SOURCE.contains("inner_size_writer:"));
    assert!(scale_arm().contains("scale_factor, .. }"));
}

/// The window's size is stated in **logical** pixels, so the same call
/// asks for the same arrangement on a 1x and a 2x display — which is
/// what makes a live window comparable with a capture.
#[test]
fn the_window_is_asked_for_a_logical_size() {
    assert_eq!(Native::DEFAULT_LOGICAL, (1280.0, 800.0));
    let native = Native::new("magistr").with_logical_size(1132.0, 641.0);
    assert_eq!(native.logical, LogicalSize::new(1132.0, 641.0));
    assert!(SOURCE.contains(".with_inner_size(self.logical)"));
}

/// The window names the project it has open, and follows a switch.
#[test]
fn the_window_title_follows_the_open_project() {
    use magistr_cockpit::cockpit::{Model, Msg, ProjectSeed, Size, update};
    let mut model = Model::new(Size::new(80, 24));
    assert_eq!(window_title("Magistr", &model), "Magistr");
    model = model.with_project("first", "/projects/first");
    assert_eq!(window_title("Magistr", &model), "first \u{2014} Magistr");
    update(
        &mut model,
        Msg::SessionFrame {
            address: magistr_cockpit::cockpit::SessionAddress {
                project: "second".into(),
                workspace: "/projects/second".into(),
                session: "second".into(),
            },
            sequence: 1,
            seed: Some(Box::new(ProjectSeed {
                name: "second".into(),
                dir: "/projects/second".into(),
                session_address: Some(magistr_cockpit::cockpit::SessionAddress {
                    project: "second".into(),
                    workspace: "/projects/second".into(),
                    session: "second".into(),
                }),
                ..ProjectSeed::default()
            })),
            messages: Vec::new(),
        },
    );
    assert_eq!(window_title("Magistr", &model), "second \u{2014} Magistr");
}
