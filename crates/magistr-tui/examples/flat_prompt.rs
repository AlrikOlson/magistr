//! A real-terminal accessibility check without an agent or project store.
//!
//! Run `cargo run -p magistr-tui --example flat_prompt` in a terminal
//! with a screen reader enabled; the reply below is entirely local.

use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_cockpit::cockpit::{Cmd, Model, Msg};
use magistr_tui::graphics::Env;
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::{RealTerminal, Terminal as _};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terminal = RealTerminal::stdio()?;
    let size = terminal.size()?;
    let mut runtime = Runtime::new(
        terminal,
        Options::new(Env::from_os().with_screen_reader(true)),
    );
    let inbox = runtime.inbox();
    let mut commands = runtime.commands().expect("the sole command consumer");
    inbox.send(Msg::Agent(AgentEvent::Text(
        "Type and edit before Enter; each character should be readable.\n\
         Try é, é, 漢 and 👨‍👩‍👧, then Backspace and Delete.\n\
         Paste several lines; Up and Down move between them.\n\
         Enter submits locally; Up recalls history and Down restores your draft.\n\
         Type read @ca then Tab to complete the sample file src/café.rs.\n\
         Ctrl-D quits on an empty prompt; Ctrl-C also quits while idle.\n"
            .to_owned(),
    )));
    inbox.send(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    let responder = tokio::spawn(async move {
        let mut received = 0usize;
        while let Some(command) = commands.next().await {
            let end = match command {
                Cmd::Prompt(_) => {
                    received += 1;
                    inbox.send(Msg::Agent(AgentEvent::Text(format!(
                        "Received prompt {received}.\n"
                    ))));
                    TurnEnd::EndTurn
                }
                Cmd::Cancel => TurnEnd::Cancelled,
                _ => continue,
            };
            inbox.send(Msg::Agent(AgentEvent::TurnEnded(end)));
        }
    });
    let result = runtime
        .run(
            Model::new(size)
                .with_title("Flat prompt accessibility check")
                .with_workspace_files(vec!["src/café.rs".to_owned(), "src/prompt.rs".to_owned()]),
        )
        .await;
    responder.await?;
    result?;
    Ok(())
}
