use super::*;

#[test]
fn output_format_display() {
    assert_eq!(OutputFormat::Json.to_string(), "json");
    assert_eq!(OutputFormat::Text.to_string(), "text");
    assert_eq!(OutputFormat::GithubActions.to_string(), "github-actions");
}

#[test]
fn output_format_from_str() {
    assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
    assert_eq!(
        "github-actions".parse::<OutputFormat>().unwrap(),
        OutputFormat::GithubActions
    );
}

#[test]
fn output_format_is_machine_readable() {
    assert!(OutputFormat::Json.is_machine_readable());
    assert!(!OutputFormat::Text.is_machine_readable());
    assert!(!OutputFormat::GithubActions.is_machine_readable());
}

#[test]
fn output_format_default_is_text() {
    assert_eq!(OutputFormat::default(), OutputFormat::Text);
}
