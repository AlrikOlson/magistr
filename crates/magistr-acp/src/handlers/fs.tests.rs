use super::*;

const TEXT: &str = "one\ntwo\nthree\nfour";

#[test]
fn no_params_returns_everything() {
    assert_eq!(apply_line_params(TEXT, None, None), TEXT);
}

#[test]
fn line_is_one_based() {
    assert_eq!(apply_line_params(TEXT, Some(2), None), "two\nthree\nfour");
}

#[test]
fn limit_caps_the_line_count() {
    assert_eq!(apply_line_params(TEXT, Some(2), Some(2)), "two\nthree");
}

#[test]
fn limit_past_the_end_is_clamped() {
    assert_eq!(apply_line_params(TEXT, Some(4), Some(10)), "four");
}

#[test]
fn start_past_the_end_is_empty() {
    assert_eq!(apply_line_params(TEXT, Some(9), None), "");
}

#[test]
fn line_zero_is_treated_as_one() {
    assert_eq!(apply_line_params(TEXT, Some(0), Some(1)), "one");
}

#[tokio::test]
async fn read_and_write_round_trip_and_announce_themselves() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested").join("note.txt");
    let (tx, mut rx) = mpsc::unbounded_channel();

    write_text_file(
        WriteTextFileRequest::new("s", path.clone(), "hello\nworld"),
        &tx,
    )
    .await
    .expect("write");
    let read = read_text_file(ReadTextFileRequest::new("s", path.clone()).line(2u32), &tx)
        .await
        .expect("read");
    assert_eq!(read.content, "world");

    let subtypes: Vec<String> = std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|e| match e {
            Ok(AcpEvent::System { subtype, .. }) => Some(subtype),
            _ => None,
        })
        .collect();
    assert_eq!(subtypes, vec!["fs_write", "fs_read"]);
}

#[tokio::test]
async fn reading_a_missing_file_is_a_protocol_error_not_a_panic() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let err = read_text_file(
        ReadTextFileRequest::new("s", "/definitely/not/here.txt"),
        &tx,
    )
    .await
    .expect_err("missing file");
    assert_eq!(i32::from(err.code), INTERNAL_ERROR);
}
