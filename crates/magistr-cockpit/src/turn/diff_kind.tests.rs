use super::*;

#[test]
fn the_ordinary_diff_lines_keep_their_kinds() {
    assert_eq!(diff_kind("+added"), RowKind::DiffAdd);
    assert_eq!(diff_kind("-removed"), RowKind::DiffRemove);
    assert_eq!(diff_kind("@@ -1,2 +1,3 @@"), RowKind::Fence);
    assert_eq!(diff_kind(" context"), RowKind::Code);
    assert_eq!(diff_kind(""), RowKind::Code);
}

#[test]
fn every_conflict_marker_reads_as_a_conflict_whichever_side_it_arrives_on() {
    for marker in ["<<<<<<< HEAD", "=======", ">>>>>>> theirs"] {
        for prefix in ["", "+", "-", " "] {
            let line = format!("{prefix}{marker}");
            assert_eq!(diff_kind(&line), RowKind::Conflict, "{line:?}");
        }
    }
}

#[test]
fn a_run_of_dashes_is_still_a_removed_line() {
    assert_eq!(diff_kind("-------"), RowKind::DiffRemove);
    assert_eq!(diff_kind("+++++++"), RowKind::DiffAdd);
    assert_eq!(diff_kind("<<< short"), RowKind::Code);
}
