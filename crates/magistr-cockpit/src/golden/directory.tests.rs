use super::*;

fn solid(width: u32, height: u32, rgb: [u8; 3]) -> Pixmap {
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..width * height {
        data.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
    }
    Pixmap::from_vec(
        data,
        tiny_skia::IntSize::from_wh(width, height).expect("size"),
    )
    .expect("pixmap")
}

#[test]
fn a_missing_golden_is_written_and_reported_as_blessed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let golden = Golden::new(dir.path().join("screens"));
    let frame = solid(4, 2, [10, 20, 30]);
    let err = golden
        .check_with_update("first", &frame, false)
        .expect_err("blessing fails the check");
    assert_eq!(
        err,
        GoldenError::Blessed {
            golden: golden.path("first")
        }
    );
    assert!(golden.path("first").exists());
    let again = Pixmap::load_png(golden.path("first")).expect("readable");
    assert_eq!(again.data(), frame.data());
    assert!(err.to_string().contains("commit it"));
}

#[test]
fn a_matching_frame_passes_and_clears_a_stale_candidate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let golden = Golden::new(dir.path());
    let frame = solid(4, 2, [10, 20, 30]);
    let _ = golden.check_with_update("same", &frame, false);
    std::fs::write(golden.candidate_path("same"), b"stale").expect("write");
    assert_eq!(golden.check_with_update("same", &frame, false), Ok(()));
    assert!(!golden.candidate_path("same").exists());
}

#[test]
fn an_edge_pixel_a_shade_off_is_anti_aliasing_and_passes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let golden = Golden::new(dir.path());
    let _ = golden.check_with_update("aa", &solid(40, 25, [10, 20, 30]), false);
    // One pixel in a thousand, off by a shade on every channel.
    let mut shade = solid(40, 25, [10, 20, 30]);
    shade.pixels_mut()[7] =
        tiny_skia::PremultipliedColorU8::from_rgba(70, 80, 90, 255).expect("opaque");
    assert_eq!(golden.check_with_update("aa", &shade, false), Ok(()));
    // Two pixels in a thousand is more than an edge.
    shade.pixels_mut()[8] =
        tiny_skia::PremultipliedColorU8::from_rgba(70, 80, 90, 255).expect("opaque");
    assert!(matches!(
        golden.check_with_update("aa", &shade, false),
        Err(GoldenError::Mismatch { differing: 2, .. })
    ));
}

#[test]
fn a_differing_frame_writes_the_candidate_beside_the_golden() {
    let dir = tempfile::tempdir().expect("tempdir");
    let golden = Golden::new(dir.path());
    let _ = golden.check_with_update("diff", &solid(4, 2, [10, 20, 30]), false);
    let mut changed = solid(4, 2, [10, 20, 30]);
    changed.pixels_mut()[3] =
        tiny_skia::PremultipliedColorU8::from_rgba(255, 0, 0, 255).expect("opaque red");
    let err = golden
        .check_with_update("diff", &changed, false)
        .expect_err("a changed pixel fails");
    assert_eq!(
        err,
        GoldenError::Mismatch {
            golden: golden.path("diff"),
            candidate: golden.candidate_path("diff"),
            differing: 1,
            size_changed: false,
        }
    );
    let candidate = Pixmap::load_png(golden.candidate_path("diff")).expect("candidate readable");
    assert_eq!(candidate.data(), changed.data());
    let untouched = Pixmap::load_png(golden.path("diff")).expect("golden readable");
    assert_ne!(untouched.data(), changed.data(), "the golden is left alone");
    assert!(err.to_string().contains("1 pixel(s) differ"));
    assert!(err.to_string().contains(Golden::UPDATE_ENV));
}

#[test]
fn a_size_change_counts_every_pixel() {
    let dir = tempfile::tempdir().expect("tempdir");
    let golden = Golden::new(dir.path());
    let _ = golden.check_with_update("size", &solid(4, 2, [1, 2, 3]), false);
    let err = golden
        .check_with_update("size", &solid(2, 2, [1, 2, 3]), false)
        .expect_err("size differs");
    assert!(matches!(
        err,
        GoldenError::Mismatch {
            differing: 4,
            size_changed: true,
            ..
        }
    ));
    assert!(err.to_string().contains("size changed"));
}

#[test]
fn an_unreadable_golden_is_an_io_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let golden = Golden::new(dir.path());
    std::fs::write(golden.path("broken"), b"not a png").expect("write");
    let err = golden
        .check_with_update("broken", &solid(1, 1, [0, 0, 0]), false)
        .expect_err("undecodable");
    assert!(matches!(err, GoldenError::Io { .. }), "{err:?}");
}

#[test]
fn explicit_update_creates_and_replaces_goldens_and_clears_candidates() {
    let dir = tempfile::tempdir().expect("tempdir");
    let golden = Golden::new(dir.path());
    let first = solid(4, 2, [10, 20, 30]);
    assert_eq!(golden.check_with_update("update", &first, true), Ok(()));
    assert_eq!(
        Pixmap::load_png(golden.path("update"))
            .expect("readable")
            .data(),
        first.data(),
    );

    std::fs::write(golden.candidate_path("update"), b"stale").expect("write");
    let changed = solid(2, 2, [255, 0, 0]);
    assert_eq!(golden.check_with_update("update", &changed, true), Ok(()));
    assert_eq!(
        Pixmap::load_png(golden.path("update"))
            .expect("readable")
            .data(),
        changed.data(),
    );
    assert!(!golden.candidate_path("update").exists());
}

#[test]
fn paths_are_where_they_say() {
    let golden = Golden::new("/g");
    assert_eq!(golden.path("a"), PathBuf::from("/g/a.png"));
    assert_eq!(golden.candidate_path("a"), PathBuf::from("/g/a.new.png"));
}
