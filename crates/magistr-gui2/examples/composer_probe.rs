//! Exercise the native editor, files, navigation, Send and Stop; close with a retained draft.
use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_cockpit::cockpit::{Cmd, Model, Msg, Size, StageTab};
use magistr_gui2::Native;

fn main() {
    let model = Model::new(Size::new(120, 40))
        .with_tab(StageTab::Turn)
        .with_workspace_files(vec!["src/main.rs".into(), "résumé draft.md".into()]);
    let mut native = Native::new("Magistr composer");
    let mut commands = native.commands().expect("command owner");
    let inbox = native.inbox();
    let producer = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("fixture runtime");
        runtime.block_on(async move {
            let mut sent = Vec::new();
            let mut stops = 0;
            while let Some(command) = commands.next().await {
                match command {
                    Cmd::Prompt(text) => {
                        sent.push(text);
                        inbox.send(Msg::Agent(AgentEvent::Text(
                            "Reviewing the attached file.".into(),
                        )));
                    }
                    Cmd::Cancel => {
                        stops += 1;
                        inbox.send(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::Cancelled)));
                    }
                    _ => {}
                }
            }
            (sent, stops)
        })
    });
    let model = native.run(model).expect("native window");
    let (sent, stops) = producer.join().expect("producer");
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains("@\"résumé draft.md\""));
    assert!(sent[0].contains('\n'));
    assert_eq!(stops, 1);
    assert_eq!(model.tab(), StageTab::Turn);
    assert_eq!(model.prompt().text(), "Keep this draft");
    println!("Native multiline, file choice, navigation, Send, Stop and retained draft verified.");
}
