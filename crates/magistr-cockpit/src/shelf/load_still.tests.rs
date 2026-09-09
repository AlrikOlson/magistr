use super::*;

/// A `width × height` PNG of one flat colour, encoded the way a golden
/// is written.
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("a pixmap allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(40, 120, 200, 255));
    pixmap.encode_png().expect("png encodes")
}

#[test]
fn a_golden_on_disk_loads_with_its_name_kind_and_pixels() {
    let dir = tempfile::tempdir().expect("tempdir");
    let motion = dir.path().join("golden").join("motion");
    std::fs::create_dir_all(&motion).expect("dirs");
    let path = motion.join("gate-pop-mocha-80x24.png");
    std::fs::write(&path, png(48, 16)).expect("write");

    let still = load_still(&path).expect("a picture loads");
    assert_eq!(still.name, "gate-pop-mocha-80x24.png");
    assert_eq!(still.source, path);
    assert_eq!(still.kind, StillKind::Filmstrip);
    assert_eq!(still.size(), (48, 16));
    assert_eq!(still.image.alt(), "gate-pop-mocha-80x24.png");
}

#[test]
fn a_missing_file_or_a_non_picture_is_no_still() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(load_still(&dir.path().join("absent.png")).is_none());
    let text = dir.path().join("notes.png");
    std::fs::write(&text, b"not a picture").expect("write");
    assert!(load_still(&text).is_none());
}
