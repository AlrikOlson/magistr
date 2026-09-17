//! Real ACP round trips driven by raw terminal pixel reports.

#[tokio::test]
async fn stop_at_its_native_pixel_position_cancels_acp_and_keeps_the_draft() {
    let mut expected = locator();
    update(
        &mut expected,
        Msg::Agent(AgentEvent::UserText("Keep working".into())),
    );
    update(&mut expected, Msg::Paste("next request".into()));
    let stop = bounds(&expected, Widget::PromptAction);
    let script = Script::new()
        .bytes(MS(0), STARTUP)
        .text(MS(300), "next request")
        .bytes(MS(650), pixel_click(stop))
        .close(MS(1100));
    let (model, observed, capture, deliveries, _) = run_seeded(
        graphics_options(),
        script,
        FakeAcpAgent::new().stalling(),
        SessionOptions::new().opening_prompt("Keep working"),
        Box::new(AskEverything),
        None,
        None,
        None,
        Model::new(GRAPHICS_SIZE),
    )
    .await;
    assert_eq!(observed.prompts(), ["Keep working"]);
    assert_eq!(observed.cancels(), 1);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    assert_eq!(model.prompt().text(), "next request");
    assert_eq!(capture.count(b"\x1b[?1016h"), 1);
    assert!(!capture.pixels_before(deliveries.instants()[2]).is_empty());
    assert!(model.recent().any(|message| matches!(message, Msg::Mouse {event, target} if event.px.is_some() && *target == Some(Widget::PromptAction.id()))));
}

use super::*;
use magistr_application::sessions::{PermissionChoice, PermissionKind};
use magistr_cockpit::layout::PxRect;
use magistr_tui::screenshot::{Profile, layout};

const GRAPHICS_SIZE: Size = Size::new(128, 40);
const STARTUP: &[u8] = b"\x1b_Gi=31;OK\x1b\\\x1b[6;29;13t\x1b[?1016;2$y\x1b[?62;4c";

fn locator() -> Model {
    let mut model = Model::new(GRAPHICS_SIZE);
    update(
        &mut model,
        Msg::CellSize(Some(magistr_cockpit::pixel::CellSize {
            width: 13,
            height: 29,
        })),
    );
    update(
        &mut model,
        Msg::Mode(magistr_cockpit::capability::Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    model
}

fn bounds(model: &Model, button: Widget) -> PxRect {
    layout(model, &FrameClock::new(0, Duration::ZERO), Profile::Kitty)
        .unwrap()
        .px(button.id())
        .expect("the production control is visible")
}

fn pixel_click(rect: PxRect) -> String {
    let x = (rect.x + rect.width / 2.0) as u32 + 1;
    let y = (rect.y + rect.height / 2.0) as u32 + 1;
    format!("\x1b[<0;{x};{y}M\x1b[<0;{x};{y}m")
}

fn permission(model: &mut Model) {
    update(model, Msg::Agent(AgentEvent::Text("Hello there".into())));
    update(
        model,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 1,
            title: "Run recovery checks".into(),
            choices: vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("allow_always", PermissionKind::AllowAlways),
                PermissionChoice::new("reject", PermissionKind::RejectOnce),
            ],
            edit: false,
            risk: RiskTier::Medium,
            plan: None,
        }),
    );
}

#[tokio::test]
async fn multiline_send_and_permission_click_reach_acp_at_drawn_pixel_positions() {
    let mut draft = locator();
    update(
        &mut draft,
        Msg::Paste("Check retries\nInclude restart recovery".into()),
    );
    let send = bounds(&draft, Widget::PromptAction);
    let mut asking = locator();
    permission(&mut asking);
    let allow = bounds(&asking, Widget::Allow);
    let script = Script::new()
        .bytes(MS(0), STARTUP)
        .text(
            MS(300),
            "\x1b[200~Check retries\nInclude restart recovery\x1b[201~",
        )
        .bytes(MS(600), pixel_click(send))
        .bytes(MS(1000), pixel_click(allow))
        .text(MS(1300), "\x1b[200~keep this draft\x1b[201~")
        .close(MS(1600));
    let (model, observed, capture, deliveries, arrivals) = run_seeded(
        graphics_options(),
        script,
        FakeAcpAgent::new()
            .reply("Hello there")
            .asking_permission("Run recovery checks"),
        SessionOptions::new(),
        Box::new(AskEverything),
        None,
        None,
        None,
        Model::new(GRAPHICS_SIZE),
    )
    .await;
    assert_eq!(
        observed.prompts(),
        ["Check retries\nInclude restart recovery"]
    );
    assert_eq!(observed.outcomes(), [Some("allow".into())]);
    assert_eq!(model.permissions_pending(), 0);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    assert_eq!(model.prompt().text(), "keep this draft");
    assert_eq!(
        model.prompt().history(),
        ["Check retries\nInclude restart recovery"]
    );
    assert_eq!(capture.count(b"\x1b[?1016h"), 1);
    assert_eq!(arrivals.permission_requests().len(), 1);
    let before = capture.pixels_before(deliveries.instants()[3]);
    let after = capture.pixels_before(*deliveries.instants().last().unwrap());
    assert!(
        !before.is_empty() && !after.is_empty(),
        "both states were drawn through native graphics"
    );
    assert!(
        before != after,
        "answering the permission redraws the native frame"
    );
    assert!(rows(&model).iter().any(|(_, text)| *text == "Hello there"));
    assert!(model.recent().any(|message| matches!(message, Msg::Mouse {event, target} if event.px.is_some() && *target == Some(Widget::Allow.id()))));
}
