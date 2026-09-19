//! Host entry points open concrete clients before taking over a surface.

use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The entry point observes the engines before it takes the terminal over:
/// a store that cannot be opened is reported on a normal stdout, never
/// inside the alternate screen. A viewer observes rather than admits itself
/// as the project's host, so it never contends with a running loop.
#[test]
fn the_example_observes_the_engines_before_the_terminal() {
    let main = crate_root().join("examples/cockpit/main.rs");
    let source = std::fs::read_to_string(main).expect("examples/cockpit/main.rs");
    assert!(
        !source.contains("EngineClient::open("),
        "a viewer must not take the host lease"
    );
    let engines = source
        .find("EngineClient::observe(")
        .expect("the example observes the engines");
    let terminal = source
        .find("RealTerminal::stdio()")
        .expect("the example takes the terminal over");
    assert!(engines < terminal, "the engines open before raw mode");
    let report = source[engines..terminal]
        .find("println!")
        .expect("a failure to open is printed before the terminal is touched");
    assert!(engines < engines + report);
}

#[test]
fn the_host_library_has_no_terminal_dependency_when_terminal_support_is_disabled() {
    let output = std::process::Command::new("cargo")
        .current_dir(crate_root())
        .args([
            "tree",
            "--locked",
            "-p",
            "magistr-cli",
            "--no-default-features",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ])
        .output()
        .expect("cargo tree");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph = String::from_utf8(output.stdout).unwrap();
    assert!(graph.contains("magistr-cockpit-runtime "));
    assert!(!graph.contains("magistr-tui "), "{graph}");
}

#[cfg(feature = "terminal")]
#[tokio::test]
async fn terminal_consumes_startup_events_and_sends_commands_on_host_channels() {
    use magistr_application::sessions::{AgentEvent, TurnEnd};
    use magistr_cockpit::cockpit::{Cmd, Model, Msg, Size, StageTab};
    use magistr_cockpit_runtime::channel::{Commands, Inbox};
    use magistr_tui::graphics::Env;
    use magistr_tui::harness::{FakeTerminal, Script};
    use magistr_tui::runtime::{Options, Runtime};
    use std::time::Duration;

    let (inbox, messages) = Inbox::channel();
    let (sender, mut commands) = Commands::channel();
    inbox.send(Msg::Agent(AgentEvent::Text(
        "Connected before opening".into(),
    )));
    inbox.send(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    let terminal = FakeTerminal::new(
        Size::new(100, 30),
        Script::new()
            .startup_replies(Duration::ZERO)
            .text(Duration::from_millis(200), "Continue\r")
            .close(Duration::from_millis(500)),
    );
    let mut runtime = Runtime::with_channels(
        terminal,
        Options::new(Env::new("xterm", false, None).with_screen_reader(true))
            .probe_timeout(Duration::from_millis(100)),
        inbox,
        messages,
        sender,
    );
    assert!(
        runtime.commands().is_none(),
        "the host already owns the receiver"
    );
    let model = runtime
        .run(Model::new(Size::new(80, 24)).with_tab(StageTab::Turn))
        .await
        .unwrap();
    assert!(
        model
            .turn()
            .lines()
            .iter()
            .any(|line| line.text().contains("Connected before opening"))
    );
    let mut sent = Vec::new();
    while let Some(command) = commands.next().await {
        sent.push(command);
    }
    assert!(
        sent.iter()
            .any(|command| matches!(command, Cmd::Prompt(text) if text == "Continue")),
        "{sent:?}"
    );
}
