#[path = "../../../magistr-cli/tests/support/in_env.rs"]
mod in_env;

use super::*;

const TRANSCRIPT: &str = include_str!("../../tests/fixtures/claude/transcript.jsonl");
const SESSION: &str = "00000000-0000-4000-8000-0000000001e3";

fn text_of(update: &SessionUpdate) -> (&'static str, String) {
    let (role, chunk) = match update {
        SessionUpdate::UserMessageChunk(c) => ("user", c),
        SessionUpdate::AgentMessageChunk(c) => ("agent", c),
        other => panic!("unexpected replay update {other:?}"),
    };
    match &chunk.content {
        AcpContent::Text(t) => (role, t.text.clone()),
        other => panic!("unexpected content {other:?}"),
    }
}

#[test]
fn the_path_encodes_the_cwd_with_dashes_under_projects() {
    let path = transcript_path(
        Path::new("/home/me/.claude"),
        Path::new("/private/tmp/scratch/probe/work"),
        SESSION,
    );
    assert_eq!(
        path,
        PathBuf::from(format!(
            "/home/me/.claude/projects/-private-tmp-scratch-probe-work/{SESSION}.jsonl"
        ))
    );
}

#[test]
fn the_recorded_transcript_replays_prompts_and_answers_only() {
    let config = tempfile::tempdir().expect("tempdir");
    let cwd = Path::new("/private/tmp/scratch/probe/work");
    let path = transcript_path(config.path(), cwd, SESSION);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, TRANSCRIPT).expect("write");

    let replayed: Vec<_> = replay(config.path(), cwd, SESSION)
        .iter()
        .map(text_of)
        .collect();
    assert_eq!(replayed.len(), 6, "{replayed:?}");
    assert_eq!(replayed[0].0, "user");
    assert!(
        replayed[0].1.starts_with("Think for a moment"),
        "{}",
        replayed[0].1
    );
    assert_eq!(replayed[1], ("agent", "pong".to_owned()));
    assert_eq!(
        replayed[2],
        ("user", "Reply with exactly one word: ping".to_owned())
    );
    assert_eq!(replayed[3], ("agent", "ping".to_owned()));
    assert_eq!(
        replayed[5],
        ("agent", "pong".to_owned()),
        "the thinking block is not replayed"
    );
}

#[test]
fn a_missing_transcript_replays_nothing() {
    let config = tempfile::tempdir().expect("tempdir");
    assert!(replay(config.path(), Path::new("/nowhere"), SESSION).is_empty());
}

#[test]
fn the_default_config_dir_is_under_home_unless_overridden() {
    let home = std::ffi::OsStr::new("/fixture/home");
    if !in_env::in_env(&[("HOME", Some(home)), ("CLAUDE_CONFIG_DIR", None)]) {
        return;
    }
    assert_eq!(default_config_dir(), Path::new(home).join(".claude"));
}

#[test]
fn an_explicit_config_directory_overrides_home() {
    let config = std::ffi::OsStr::new("/fixture/claude");
    if !in_env::in_env(&[("CLAUDE_CONFIG_DIR", Some(config))]) {
        return;
    }
    assert_eq!(default_config_dir(), Path::new(config));
}
