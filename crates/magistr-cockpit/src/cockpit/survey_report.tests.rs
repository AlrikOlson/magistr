use super::*;

#[test]
fn a_report_counts_every_fact_and_receipt_as_a_row() {
    let mut report = SurveyReport::named("tidewatch");
    assert_eq!(report.rows(), 0);
    report.detected.push((SurveyKind::Stack, "Rust".to_owned()));
    report.detected.push((SurveyKind::Git, "main".to_owned()));
    report.written.push((
        ScaffoldKind::Config,
        ".magistr/project.json".to_owned(),
        true,
    ));
    assert_eq!(report.rows(), 3);
    assert_eq!(report.name, "tidewatch");
}
