use super::DagLayout;

#[test]
fn inserts_virtual_vertices_for_long_edges() {
    let layout = DagLayout::new(&[0, 2], &[(0, 1)], &[0, 1]);
    assert_eq!(layout.points.len(), 3);
    assert_eq!(layout.paths[0].points.len(), 3);
    assert_eq!(layout.paths[0].points[1].0, 1.0);
}

#[test]
fn reduction_removes_transitive_edges_but_keeps_cycles() {
    assert_eq!(
        DagLayout::reduce_edges(vec![(0, 1), (1, 2), (0, 2)], 3),
        vec![(0, 1), (1, 2)]
    );
    assert_eq!(
        DagLayout::reduce_edges(vec![(0, 1), (1, 0)], 2),
        vec![(0, 1), (1, 0)]
    );
}
