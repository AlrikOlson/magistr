use super::*;

#[test]
fn defaults_match_the_retained_tool() {
    let input = RunInput::new("test");
    assert_eq!(input.timeout_secs, 300);
    assert_eq!(input.budget, 4000);
    assert!(input.args.is_empty());
    assert!(input.validate().is_ok());
}

#[test]
fn schema_misuse_is_refused_before_admission() {
    assert_eq!(
        RunInput::new("").validate().unwrap_err().kind,
        ErrorKind::PatternInvalid
    );
    assert_eq!(
        RunInput::new("test")
            .with_timeout_secs(0)
            .validate()
            .unwrap_err()
            .kind,
        ErrorKind::PatternInvalid
    );
    assert_eq!(
        RunInput::new("test")
            .with_timeout_secs(9999)
            .validate()
            .unwrap_err()
            .kind,
        ErrorKind::PatternInvalid
    );
    assert_eq!(
        RunInput::new("test")
            .with_budget(0)
            .validate()
            .unwrap_err()
            .kind,
        ErrorKind::BudgetTooSmall
    );
}

#[test]
fn builder_keeps_args_literal() {
    let input = RunInput::new("test").with_args(["--filter", "a b"]);
    assert_eq!(input.args, vec!["--filter".to_owned(), "a b".to_owned()]);
}
