use super::*;
use magistr_application::sessions::AgentEvent;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// A controlled project host: it records the commands it was given, posts
/// what it is told to on its own inbox, and notes when it was settled.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<String>>>);

impl Log {
    fn note(&self, line: impl Into<String>) {
        self.0.lock().unwrap().push(line.into());
    }
    fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

struct Hosted {
    stop: tokio::sync::oneshot::Sender<()>,
    task: JoinHandle<Result<(), String>>,
}

impl Settle for Hosted {
    fn settle(self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async move {
            let _ = self.stop.send(());
            self.task.await.map_err(|error| error.to_string())?
        })
    }
}

fn opener(log: Log) -> Open<Hosted> {
    Box::new(move |opening: Opening| {
        let log = log.clone();
        Box::pin(async move {
            let name = opening
                .dir
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if name == "held" {
                return Err("Another Magistr session has this project open.".to_owned());
            }
            log.note(format!("open {name} switched={}", opening.switched));
            let seed = ProjectSeed {
                name: name.clone(),
                dir: opening.dir.display().to_string(),
                ..ProjectSeed::default()
            };
            let Opening {
                inbox,
                mut commands,
                ..
            } = opening;
            let (stop, mut stopped) = tokio::sync::oneshot::channel::<()>();
            let served = log.clone();
            let task = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = &mut stopped => break,
                        command = commands.next() => match command {
                            Some(Cmd::Prompt(text)) => {
                                served.note(format!("{name} got {text}"));
                                inbox.send(Msg::Agent(AgentEvent::Text(format!("{name}: {text}"))));
                            }
                            Some(_) => {}
                            None => break,
                        },
                    }
                }
                // A settling host may still speak; the window must not hear it.
                inbox.send(Msg::Agent(AgentEvent::Text(format!("{name} closing"))));
                served.note(format!("settled {name}"));
                Ok(())
            });
            Ok((Hosted { stop, task }, seed))
        })
    })
}

struct Window {
    host: ProjectWindow,
    commands: mpsc::UnboundedSender<Cmd>,
    events: mpsc::UnboundedReceiver<Msg>,
    log: Log,
    _tmp: tempfile::TempDir,
    root: std::path::PathBuf,
}

async fn window() -> Window {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    for name in ["first", "second", "held"] {
        std::fs::create_dir_all(root.join(name).join(".magistr")).unwrap();
        std::fs::write(root.join(name).join(".magistr/project.json"), "{}").unwrap();
    }
    std::fs::create_dir(root.join("bare")).unwrap();
    let log = Log::default();
    let (inbox, events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (host, seed) =
        ProjectWindow::start_with(opener(log.clone()), root.join("first"), inbox, receiver)
            .await
            .unwrap();
    assert_eq!(seed.name, "first");
    Window {
        host,
        commands,
        events,
        log,
        _tmp: tmp,
        root,
    }
}

async fn next(events: &mut mpsc::UnboundedReceiver<Msg>) -> Msg {
    tokio::time::timeout(Duration::from_secs(3), events.recv())
        .await
        .expect("an event in time")
        .expect("the window's inbox stays open")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_switch_admits_the_next_project_then_settles_the_previous_one_behind_a_fence() {
    let mut w = window().await;
    let second = w.root.join("second").display().to_string();
    w.commands.send(Cmd::OpenProject(second)).unwrap();
    let Msg::Rehosted(seed) = next(&mut w.events).await else {
        panic!("the swap is the first thing the window hears");
    };
    assert_eq!(seed.name, "second");
    assert_eq!(
        w.log.lines(),
        [
            "open first switched=false",
            "open second switched=true",
            "settled first"
        ],
        "admitted before the previous host was settled"
    );
    w.commands.send(Cmd::Prompt("hello".into())).unwrap();
    assert_eq!(
        next(&mut w.events).await,
        Msg::Agent(AgentEvent::Text("second: hello".into())),
        "commands reach the new host and the settled host's last words never arrive"
    );
    w.host.shutdown().await.unwrap();
    assert_eq!(w.log.lines().last().unwrap(), "settled second");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_switch_says_why_and_the_current_project_keeps_serving() {
    let mut w = window().await;
    for (name, words) in [
        ("held", "Another Magistr session has this project open."),
        ("bare", "has no Magistr setup"),
        ("missing", "is not a directory"),
    ] {
        let dir = w.root.join(name).display().to_string();
        w.commands.send(Cmd::OpenProject(dir.clone())).unwrap();
        let Msg::CommandFailed { cmd, error } = next(&mut w.events).await else {
            panic!("a refusal in words");
        };
        assert_eq!(cmd, Cmd::OpenProject(dir));
        assert!(error.contains(words), "{name}: {error}");
    }
    w.commands.send(Cmd::Prompt("still here".into())).unwrap();
    assert_eq!(
        next(&mut w.events).await,
        Msg::Agent(AgentEvent::Text("first: still here".into()))
    );
    assert!(!w.log.lines().iter().any(|line| line == "settled first"));
    w.host.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_request_queued_while_the_project_changed_returns_to_the_editor() {
    let mut w = window().await;
    let second = w.root.join("second").display().to_string();
    // Both are queued before the window host reads either.
    w.commands.send(Cmd::OpenProject(second)).unwrap();
    w.commands
        .send(Cmd::Prompt("for the first".into()))
        .unwrap();
    assert!(matches!(next(&mut w.events).await, Msg::Rehosted(_)));
    let Msg::CommandFailed { cmd, error } = next(&mut w.events).await else {
        panic!("the queued request comes back");
    };
    assert_eq!(cmd, Cmd::Prompt("for the first".into()));
    assert!(error.contains("project changed"), "{error}");
    assert!(!w.log.lines().iter().any(|line| line.contains("got")));
    w.host.shutdown().await.unwrap();
}
