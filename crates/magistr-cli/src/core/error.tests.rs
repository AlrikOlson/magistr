use super::*;

// Core error tests (MagistrError, suggestion(), From impls, MapMagistrErr)
// live in crates/magistr-core/src/error.tests.rs.
// This file tests CLI-specific extensions: ResultExt::suggest().

#[test]
fn suggest_converts_error_result() {
    let result: Result<()> = Err(MagistrError::Config("bad config".into()));
    let eyre_result = result.suggest();
    assert!(eyre_result.is_err());
    let report = eyre_result.unwrap_err();
    let report_str = format!("{report:?}");
    assert!(report_str.contains("bad config"));
}

#[test]
fn suggest_passes_through_ok() {
    let result: Result<i32> = Ok(42);
    let eyre_result = result.suggest();
    assert_eq!(eyre_result.unwrap(), 42);
}
