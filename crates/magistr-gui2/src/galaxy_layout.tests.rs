use super::*;

const FIELD: (f32, f32) = (1100.0, 553.0);

fn projects() -> Vec<GalaxyProject> {
    [
        ("magistr", Some(211), 1, 0),
        ("think-and-ship", None, 1, 1),
        ("quicksilver", None, 0, 0),
    ]
    .into_iter()
    .map(|(name, done, running, asking)| GalaxyProject {
        name: name.to_owned(),
        done,
        running,
        asking,
        ..GalaxyProject::default()
    })
    .collect()
}

/// Three projects read left, upper-right, lower-right — the volume's
/// own arrangement — and every star and label stays on the field.
#[test]
fn three_clusters_take_the_volumes_three_corners_and_stay_on_the_field() {
    let placed = GalaxyLayout::place(&projects(), FIELD, 1.0);
    assert_eq!(placed.len(), 3);
    let centre = |c: &PlacedCluster| {
        let n = c.stars.len() as f32;
        let (sx, sy) = c
            .stars
            .iter()
            .fold((0.0, 0.0), |(a, b), &(x, y)| (a + x, b + y));
        (sx / n, sy / n)
    };
    let (mx, my) = centre(&placed[0]);
    let (tx, ty) = centre(&placed[1]);
    let (qx, qy) = centre(&placed[2]);
    assert!(mx < FIELD.0 * 0.5, "the first cluster is left of centre");
    assert!(tx > FIELD.0 * 0.5 && ty < my, "the second is upper-right");
    assert!(qx > FIELD.0 * 0.5 && qy > ty, "the third is lower-right");
    for cluster in &placed {
        for &(x, y) in &cluster.stars {
            assert!(x > 0.0 && x < FIELD.0 && y > 0.0 && y < FIELD.1, "{x},{y}");
        }
        assert!(cluster.label.0 > 0.0 && cluster.label.1 < FIELD.1);
        assert!(cluster.bounds.width > 0.0 && cluster.bounds.height > 0.0);
    }
}

/// The label hangs under its own stars — a little left of the first,
/// below the last — and the click box holds the stars and the label.
#[test]
fn the_label_anchors_under_the_clusters_own_stars_inside_its_click_box() {
    for cluster in GalaxyLayout::place(&projects(), FIELD, 1.0) {
        let x0 = cluster.stars.iter().map(|s| s.0).fold(f32::MAX, f32::min);
        let y1 = cluster.stars.iter().map(|s| s.1).fold(f32::MIN, f32::max);
        assert!(cluster.label.0 < x0);
        assert!(cluster.label.1 > y1);
        let inside = |(x, y): (f32, f32)| {
            x >= cluster.bounds.x
                && x <= cluster.bounds.x + cluster.bounds.width
                && y >= cluster.bounds.y
                && y <= cluster.bounds.y + cluster.bounds.height
        };
        assert!(cluster.stars.iter().all(|&s| inside(s)));
        assert!(inside(cluster.label));
    }
}

/// Zoom spreads the clusters and grows them together; one project sits
/// at the centre with no ring to lie on.
#[test]
fn zoom_scales_the_ring_and_the_clusters_and_one_project_sits_at_the_centre() {
    let rest = GalaxyLayout::place(&projects(), FIELD, 1.0);
    let zoomed = GalaxyLayout::place(&projects(), FIELD, 2.0);
    let span = |c: &PlacedCluster| {
        let xs: Vec<f32> = c.stars.iter().map(|s| s.0).collect();
        xs.iter().fold(f32::MIN, |a, &b| a.max(b)) - xs.iter().fold(f32::MAX, |a, &b| a.min(b))
    };
    assert!(span(&zoomed[0]) > span(&rest[0]) * 1.9, "the cluster grows");
    assert!(
        (zoomed[0].stars[0].0 - zoomed[1].stars[0].0).abs()
            > (rest[0].stars[0].0 - rest[1].stars[0].0).abs(),
        "the clusters spread"
    );
    let one = GalaxyLayout::place(&projects()[..1], FIELD, 1.0);
    let (sx, sy) = one[0]
        .stars
        .iter()
        .fold((0.0, 0.0), |(a, b), &(x, y)| (a + x, b + y));
    let n = one[0].stars.len() as f32;
    assert!((sx / n - FIELD.0 * 0.5).abs() < GalaxyLayout::RX);
    assert!((sy / n - FIELD.1 * 0.47).abs() < GalaxyLayout::RY * 0.5);
    assert!(GalaxyLayout::place(&[], FIELD, 1.0).is_empty());
}
