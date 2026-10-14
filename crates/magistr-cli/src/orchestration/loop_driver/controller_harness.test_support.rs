//! Loop contract tests use addressed semantic actions and observations, with no window or terminal.
use super::{Cmd, Commands, Inbox, LoopDriver, LoopReport, Msg};
use magistr_application::sessions::{AgentEvent, PermissionKind};
use magistr_cockpit::cockpit::LoopEvent;
use std::time::Duration;
use tokio::sync::mpsc;

enum Action {
    Command(Cmd),
    Control(magistr_cockpit::cockpit::LoopCmd),
    Allow,
}
#[derive(Default)]
pub(super) struct Actions(Vec<(Duration, Action)>);
impl Actions {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn command(mut self, at: Duration, command: Cmd) -> Self {
        self.0.push((at, Action::Command(command)));
        self
    }
    pub(super) fn control(
        mut self,
        at: Duration,
        action: magistr_cockpit::cockpit::LoopCmd,
    ) -> Self {
        self.0.push((at, Action::Control(action)));
        self
    }
    pub(super) fn allow(mut self, at: Duration) -> Self {
        self.0.push((at, Action::Allow));
        self
    }
}
pub(super) struct ControlHarness {
    inbox: Inbox,
    events: mpsc::UnboundedReceiver<Msg>,
    sender: mpsc::UnboundedSender<Cmd>,
    commands: Option<Commands>,
    actions: Actions,
}
impl ControlHarness {
    pub(super) fn new(actions: Actions) -> Self {
        let (inbox, events) = Inbox::channel();
        let (sender, commands) = Commands::channel();
        Self {
            inbox,
            events,
            sender,
            commands: Some(commands),
            actions,
        }
    }
    pub(super) fn inbox(&self) -> Inbox {
        self.inbox.clone()
    }
    pub(super) fn commands(&mut self) -> Option<Commands> {
        self.commands.take()
    }
    pub(super) fn sender(&self) -> mpsc::UnboundedSender<Cmd> {
        self.sender.clone()
    }
}
pub(super) async fn run_observed(
    mut harness: ControlHarness,
    driver: LoopDriver,
) -> (Vec<Msg>, LoopReport) {
    let mut run = driver.spawn();
    let started = tokio::time::Instant::now();
    let mut actions = harness.actions.0.into_iter().peekable();
    let mut permission = None;
    let mut observed = Vec::new();
    let mut run_id = None;
    let report=tokio::time::timeout(Duration::from_secs(120),async {
        loop {
            let delay=actions.peek().map_or(Duration::from_secs(120),|(at,_)| at.saturating_sub(started.elapsed()));
            tokio::select! {
                biased;
                event=harness.events.recv()=>if let Some(event)=event {
                    if let Msg::Agent(AgentEvent::PermissionRequest {id,choices,..})=&event {
                        permission=choices.iter().find(|choice|choice.kind==PermissionKind::AllowOnce).map(|choice|(*id,choice.option_id.clone()));
                    }
                    if let Msg::WorkRun(run) = &event { run_id = Some(run.id.clone()); }
                    observed.push(event);
                },
                report=&mut run=>break report.expect("loop task"),
                _=tokio::time::sleep(delay),if actions.peek().is_some()=>{
                    match actions.next().expect("scheduled action").1 {
                        Action::Command(command)=>{let _=harness.sender.send(command);}
                        Action::Control(action)=>{
                            harness.sender.send(Cmd::Loop(magistr_cockpit::cockpit::RunCommand {
                                run_id: run_id.clone().expect("observed running owner"), action,
                            })).expect("run control");
                        }
                        Action::Allow=>{
                            let (id,option_id)=permission.take().expect("the provider's pending request");
                            harness.sender.send(Cmd::Permission {id,option_id}).expect("session command");
                        }
                    }
                }
            }
        }
    }).await.expect("loop settles");
    while let Ok(event) = harness.events.try_recv() {
        observed.push(event);
    }
    (observed, report)
}
pub(super) fn notes(events: &[Msg]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Msg::Loop(LoopEvent::Note(text)) => Some(text.clone()),
            Msg::Loop(LoopEvent::Failed(text)) => Some(format!("error: {text}")),
            Msg::Loop(LoopEvent::Ended(text)) => Some(if text.is_empty() {
                "Work stopped.".into()
            } else {
                format!("Work stopped: {text}")
            }),
            Msg::Loop(LoopEvent::Paused(text)) => Some(format!("paused: {text}")),
            Msg::Loop(LoopEvent::Verdict {
                iteration,
                ok,
                impossible,
                reason,
            }) => Some(format!(
                "judge, iteration {iteration}: {} — {reason}",
                if *ok {
                    "met"
                } else if *impossible {
                    "impossible"
                } else {
                    "not yet"
                }
            )),
            Msg::Agent(AgentEvent::Notice(text)) => Some(text.clone()),
            Msg::Agent(AgentEvent::Error(text)) | Msg::CommandFailed { error: text, .. } => {
                Some(format!("error: {text}"))
            }
            _ => None,
        })
        .collect()
}
pub(super) fn stage_text(events: &[Msg]) -> String {
    notes(events).join(" ")
}
