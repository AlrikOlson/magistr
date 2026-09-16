use super::*;

#[test]
fn intrinsic_cache_preserves_role_metrics_and_invalidates_on_resize() {
    let mut face = Typography::default();
    for cell in [CellSize::DEFAULT, CellSize::new(13, 29), CellSize::DEFAULT] {
        face.set_cell(cell);
        assert_eq!(face.intrinsic_entries, 0);
        for role in TextRole::ALL {
            let mut text = TextMetrics::new("Saved café 漢字 WWW iii");
            text.set_role(role);
            let mut fresh = Typography::default();
            fresh.set_cell(cell);
            let expected = fresh.intrinsic(&text);
            assert_eq!(face.intrinsic(&text), expected);
            let entries = face.intrinsic_entries;
            assert_eq!(face.intrinsic(&text), expected);
            assert_eq!(
                face.intrinsic_entries, entries,
                "a hit does not grow the cache"
            );
            assert_eq!(face.wrap(&text, 113.5), fresh.wrap(&text, 113.5));
        }
    }
}

#[test]
fn intrinsic_cache_has_bounded_storage_and_bypasses_long_content() {
    let mut face = Typography::default();
    for index in 0..300 {
        let text = TextMetrics::new(format!("Entry {index} {}", "plain words ".repeat(100)));
        face.intrinsic(&text);
        assert!(face.intrinsic_entries <= 256);
        assert!(face.intrinsic_bytes <= 256 * 1024);
    }
    let before = (face.intrinsic_entries, face.intrinsic_bytes);
    let long = TextMetrics::new("a".repeat(5000));
    assert!(face.intrinsic(&long).0 > 0.0);
    assert_eq!((face.intrinsic_entries, face.intrinsic_bytes), before);
    face.set_cell(CellSize::new(12, 28));
    assert_eq!((face.intrinsic_entries, face.intrinsic_bytes), (0, 0));
    assert!(face.intrinsic.is_empty());
}
