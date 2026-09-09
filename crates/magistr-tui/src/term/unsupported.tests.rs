use super::*;

#[test]
fn the_terminal_cannot_be_opened_and_says_why() {
    let err = RealTerminal::stdio().expect_err("no terminal backend here");
    assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    assert!(
        err.to_string().contains("no terminal backend"),
        "the refusal names the missing backend: {err}"
    );
}

#[test]
fn stderr_is_refused_the_same_way() {
    let err = super::super::OwnedStderr::take().expect_err("no redirect here");
    assert_eq!(err.kind(), io::ErrorKind::Unsupported);
}
