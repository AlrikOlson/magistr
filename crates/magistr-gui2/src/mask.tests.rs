use super::*;

#[test]
fn a_view_with_no_mask_is_diffed_whole_at_the_default_threshold() {
    let mask = Mask::none();
    assert_eq!(mask.threshold(), Mask::DEFAULT_THRESHOLD);
    assert!(!mask.covers(0, 0, Medium::Night));
    assert_eq!(mask.exclusions(Medium::Day).count(), 0);
    let missing = Path::new("/nonexistent/99.mask.json");
    assert_eq!(
        Mask::load(missing).expect("a missing mask is no mask"),
        mask
    );
}

#[test]
fn a_rect_covers_its_pixels_and_a_medium_scoped_one_only_in_that_medium() {
    let mask = Mask::parse(
        r#"{"threshold": 0.02, "excluded": [
            {"x": 10, "y": 20, "w": 5, "h": 3, "why": "the clock"},
            {"x": 100, "y": 100, "w": 8, "h": 8, "why": "a night halo", "media": "night"}
        ]}"#,
    )
    .expect("parses");
    assert!((mask.threshold() - 0.02).abs() < f32::EPSILON);
    // Inclusive at the origin, exclusive at the far edge.
    assert!(mask.covers(10, 20, Medium::Day));
    assert!(mask.covers(14, 22, Medium::Day));
    assert!(!mask.covers(15, 22, Medium::Day));
    assert!(!mask.covers(14, 23, Medium::Day));
    assert!(!mask.covers(9, 20, Medium::Day));
    // The halo is masked by night and measured by day.
    assert!(mask.covers(100, 100, Medium::Night));
    assert!(!mask.covers(100, 100, Medium::Day));
    assert_eq!(mask.exclusions(Medium::Night).count(), 2);
    assert_eq!(
        mask.exclusions(Medium::Day).collect::<Vec<_>>(),
        vec![(10, 20, 5, 3, "the clock")]
    );
}

#[test]
fn regions_parse_strictly_and_carry_their_own_budgets() {
    let mask = Mask::parse(
        r#"{"threshold": 0.02, "regions": [
            {"name": "header", "x": 0, "y": 0, "w": 100, "h": 52, "budget": 0.01},
            {"name": "feed", "x": 0, "y": 52, "w": 60, "h": 40, "budget": 0.02}
        ]}"#,
    )
    .expect("parses");
    let regions = mask.regions();
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].name, "header");
    assert!((regions[0].budget - 0.01).abs() < f32::EPSILON);
    // Inclusive at the origin, exclusive at the far edge.
    assert!(regions[0].contains(0, 0));
    assert!(regions[0].contains(99, 51));
    assert!(!regions[0].contains(100, 51));
    assert!(!regions[0].contains(99, 52));
    assert_eq!(regions[1].name, "feed");
    // No regions is an empty slice, and Mask::none has none.
    assert!(Mask::none().regions().is_empty());
    let refused = [
        r#"{"regions": {}}"#,
        r#"{"regions": [{"x": 0, "y": 0, "w": 1, "h": 1, "budget": 0.1}]}"#,
        r#"{"regions": [{"name": " ", "x": 0, "y": 0, "w": 1, "h": 1, "budget": 0.1}]}"#,
        r#"{"regions": [{"name": "a", "x": 0, "y": 0, "w": 0, "h": 1, "budget": 0.1}]}"#,
        r#"{"regions": [{"name": "a", "x": 0, "y": 0, "w": 1, "h": 1}]}"#,
        r#"{"regions": [{"name": "a", "x": 0, "y": 0, "w": 1, "h": 1, "budget": 2}]}"#,
        r#"{"regions": [{"name": "a", "x": 0, "y": 0, "w": 1, "h": 1, "budget": 0.1, "why": "no"}]}"#,
        r#"{"regions": [{"name": "a", "x": 0, "y": 0, "w": 1, "h": 1, "budget": 0.1},
                        {"name": "a", "x": 5, "y": 5, "w": 1, "h": 1, "budget": 0.1}]}"#,
    ];
    for json in refused {
        assert!(Mask::parse(json).is_err(), "accepted {json}");
    }
}

#[test]
fn the_format_is_read_strictly() {
    let refused = [
        r#"[]"#,
        r#"{"treshold": 0.1}"#,
        r#"{"threshold": 2}"#,
        r#"{"threshold": "1%"}"#,
        r#"{"excluded": {}}"#,
        r#"{"excluded": [{"x": 1, "y": 1, "w": 1}]}"#,
        r#"{"excluded": [{"x": 1, "y": 1, "w": 0, "h": 1, "why": "zero"}]}"#,
        r#"{"excluded": [{"x": -1, "y": 1, "w": 1, "h": 1, "why": "neg"}]}"#,
        r#"{"excluded": [{"x": 1, "y": 1, "w": 1, "h": 1}]}"#,
        r#"{"excluded": [{"x": 1, "y": 1, "w": 1, "h": 1, "why": " "}]}"#,
        r#"{"excluded": [{"x": 1, "y": 1, "w": 1, "h": 1, "why": "ok", "media": "dusk"}]}"#,
        r#"{"excluded": [{"x": 1, "y": 1, "w": 1, "h": 1, "why": "ok", "note": "extra"}]}"#,
    ];
    for json in refused {
        assert!(Mask::parse(json).is_err(), "accepted {json}");
    }
    let empty = Mask::parse("{}").expect("an empty object is no mask");
    assert_eq!(empty, Mask::none());
}

#[test]
fn a_malformed_file_is_an_error_naming_the_path() {
    let dir = std::env::temp_dir().join(format!("gui2-mask-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("07.mask.json");
    std::fs::write(&path, "{\"threshold\": \"lots\"}").expect("write");
    let error = Mask::load(&path).expect_err("refused");
    assert!(error.to_string().contains("07.mask.json"), "{error}");
    std::fs::remove_dir_all(&dir).expect("cleanup");
}
