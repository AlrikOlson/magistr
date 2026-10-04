//! Combined browsing and rich reading on an actual Unix tty and production renderer.
#![cfg(unix)]

#[path = "support/journey_fixture.rs"]
mod fixture;
#[path = "support/journey_pty.rs"]
mod tty;

use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_cockpit::cockpit::{Cmd, FrameClock, Inspection, Model, Msg, Selected, Size, update};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::harness::{Scenes, fixtures};
use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};
use magistr_tui::graphics::Env;
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::screenshot::{Profile, frame};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use tty::Pty;

const WIDE: Size = Size::new(120, 40);
const NARROW: Size = Size::new(80, 24);
const LIMIT: Duration = Duration::from_secs(4);

#[tokio::test]
async fn real_tty_browse_read_request_and_return_preserve_the_journey() {
    tokio::task::LocalSet::new()
        .run_until(async {
            for theme in [ThemeMode::Dark, ThemeMode::Light] {
                journey(theme).await;
            }
        })
        .await;
}

#[tokio::test]
async fn graphics_tty_reads_links_resizes_and_preserves_a_draft() {
    tokio::task::LocalSet::new()
        .run_until(async {
            for theme in [ThemeMode::Dark, ThemeMode::Light] {
                let prefix = if theme == ThemeMode::Dark {
                    "graphics-dark"
                } else {
                    "graphics-light"
                };
                let mut pty = Pty::new(WIDE);
                let quit = CancellationToken::new();
                let runtime = Runtime::new(
                    pty.terminal(),
                    Options::new(Env::new("xterm-kitty", false, None))
                        .probe_timeout(Duration::from_secs(1))
                        .quit_on(quit.clone()),
                );
                let inbox = runtime.inbox();
                let model = fixture::model(WIDE)
                    .with_theme(Theme::forced(theme).with_motion(MotionMode::Reduced));
                // Startup reads synchronously; queue the emulator reply before yielding to it.
                pty.type_bytes(b"\x1b_Gi=31;OK\x1b\\\x1b[6;20;10t\x1b[?62;4c");
                let running = tokio::task::spawn_local(runtime.run(model));
                pty.wire_shows(0, b"\x1b_Ga=p").await;
                pty.type_bytes(b"\x1b/Publish\r\x1b[B\r");
                tokio::time::sleep(Duration::from_millis(100)).await;
                pty.graphics_capture(&format!("{prefix}-selected"), WIDE);
                pty.type_bytes(b"\x1b[C\r\x1b[C\r");
                tokio::time::sleep(Duration::from_millis(100)).await;
                pty.graphics_capture(&format!("{prefix}-cross-track"), WIDE);
                pty.type_bytes(b"\x1bv\x1b/Publish\r\x1b[B\r\x1b[H\x1b[6~");
                tokio::time::sleep(Duration::from_millis(100)).await;
                pty.graphics_capture(&format!("{prefix}-reading"), WIDE);
                pty.resize(NARROW);
                tokio::time::sleep(Duration::from_millis(150)).await;
                pty.graphics_capture(&format!("{prefix}-narrow"), NARROW);
                pty.resize(WIDE);
                tokio::time::sleep(Duration::from_millis(150)).await;
                // A real cell mouse report hits the graphics composer at its measured rectangle.
                let rect = magistr_tui::screenshot::layout(
                    &fixture::model(WIDE),
                    &FrameClock::new(0, Duration::ZERO),
                    Profile::Kitty,
                )
                .expect("graphics layout")
                .rect(magistr_cockpit::cockpit::Widget::Prompt.id())
                .expect("composer");
                pty.type_bytes(
                    format!(
                        "\x1b[<0;{};{}M\x1b[<0;{};{}m",
                        rect.x + 2,
                        rect.y + 1,
                        rect.x + 2,
                        rect.y + 1
                    )
                    .as_bytes(),
                );
                pty.type_bytes(fixture::DRAFT.as_bytes());
                tokio::time::sleep(Duration::from_millis(100)).await;
                pty.graphics_capture(&format!("{prefix}-returned"), WIDE);
                let mut snapshot = fixture::snapshot();
                snapshot.chunks.retain(|chunk| chunk.id != "waiting");
                snapshot.progress.total_tasks -= 1;
                inbox.send(Msg::Engine(EngineEvent::Seed(snapshot)));
                tokio::time::sleep(Duration::from_millis(100)).await;
                pty.graphics_capture(&format!("{prefix}-deleted"), WIDE);
                pty.resize(NARROW);
                tokio::time::sleep(Duration::from_millis(150)).await;
                pty.type_bytes(b"\x1b[<0;29;4M\x1b[<0;29;4m");
                tokio::time::sleep(Duration::from_millis(100)).await;
                pty.graphics_capture(&format!("{prefix}-deleted-narrow"), NARROW);
                quit.cancel();
                let model = tokio::time::timeout(LIMIT, running)
                    .await
                    .expect("bounded graphics shutdown")
                    .expect("runtime task")
                    .expect("runtime");
                assert!(model.uses_flight_recorder());
                assert_eq!(model.selected(), Some(&Selected::Chunk("waiting".into())));
                assert_eq!(model.prompt().text(), fixture::DRAFT);
                assert_eq!(model.size(), NARROW);
                assert_eq!(
                    Inspection::of(&model).expect("missing selection").status,
                    "Unavailable"
                );
                pty.persist(prefix);
            }
        })
        .await;
}

#[tokio::test]
async fn flat_tty_retains_complete_rich_details_and_planning_replies() {
    tokio::task::LocalSet::new()
        .run_until(async {
            for size in [WIDE, NARROW] {
                let mut pty = Pty::new(size);
                let quit = CancellationToken::new();
                let mut runtime = Runtime::new(
                    pty.terminal(),
                    Options::new(Env::new("xterm", false, None).with_screen_reader(true))
                        .probe_timeout(Duration::ZERO)
                        .quit_on(quit.clone()),
                );
                let inbox = runtime.inbox();
                let mut commands = runtime.commands().expect("commands");
                let running = tokio::task::spawn_local(runtime.run(fixture::model(size)));
                pty.shows("(flat mode)").await;
                pty.type_bytes(b"\x1b/Publish\r\x1b[B\r");
                pty.wire_shows(0, b"Keep the original receipt and command.")
                    .await;
                for source in Scenes::RICH_BODY.lines().filter(|line| !line.is_empty()) {
                    pty.wire_shows(0, source.as_bytes()).await;
                }
                pty.capture("selected");
                pty.type_bytes(fixture::REQUEST.as_bytes());
                pty.shows(&format!("input: {}", fixture::REQUEST)).await;
                pty.type_bytes(b"\r");
                let submitted = tokio::time::timeout(LIMIT, async {
                    loop {
                        if let Cmd::RoadmapPrompt(text) = commands.next().await.expect("command") {
                            break text;
                        }
                    }
                })
                .await
                .expect("flat planning submit");
                assert_eq!(submitted, fixture::REQUEST);
                inbox.send(Msg::RoadmapRequestStarted);
                inbox.send(Msg::Agent(AgentEvent::Text(Scenes::RICH_BODY.into())));
                inbox.send(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
                pty.wire_shows(0, b"agent: # Delivery findings").await;
                pty.type_bytes(fixture::DRAFT.as_bytes());
                pty.shows(&format!("input: {}", fixture::DRAFT)).await;
                pty.capture("reply-and-draft");
                quit.cancel();
                let model = tokio::time::timeout(LIMIT, running)
                    .await
                    .expect("bounded flat shutdown")
                    .expect("runtime task")
                    .expect("runtime");
                pty.drain();
                assert_eq!(model.selected(), Some(&Selected::Chunk("waiting".into())));
                assert_eq!(model.prompt().text(), fixture::DRAFT);
                assert_eq!(model.prompt().history(), [fixture::REQUEST]);
                for forbidden in [
                    b"\x1b[?1049h".as_slice(),
                    b"\x1b_Ga=t",
                    b"\x1b_Ga=p",
                    b"\x1b[2J",
                ] {
                    assert!(
                        !pty.wire
                            .windows(forbidden.len())
                            .any(|part| part == forbidden)
                    );
                }
                pty.persist(&format!("flat-{}x{}", size.cols, size.rows));
            }
        })
        .await;
}

async fn journey(theme: ThemeMode) {
    let mut pty = Pty::new(WIDE);
    let quit = CancellationToken::new();
    let mut runtime = Runtime::new(
        pty.terminal(),
        Options::new(Env::new("xterm", false, None))
            .probe_timeout(Duration::ZERO)
            .quit_on(quit.clone()),
    );
    let inbox = runtime.inbox();
    let mut commands = runtime.commands().expect("commands");
    let model =
        fixture::model(WIDE).with_theme(Theme::forced(theme).with_motion(MotionMode::Reduced));
    assert_eq!(fixture::snapshot().chunks.len(), fixture::RECORDS);
    let running = tokio::task::spawn_local(runtime.run(model));
    pty.shows("Publish invitations").await;
    pty.shows("Waiting 1").await;
    pty.capture("tracks");
    pty.click("Publish invitations").await;
    pty.click("▾ Track 00").await;
    pty.shows("▸ Track 00").await;

    // Collapse, search through the collapsed track, then clear/reveal.
    pty.click("▾ Delivery").await;
    pty.shows("▸ Delivery").await;
    pty.capture("hidden-selection");
    pty.key(b"\x1b/Publish", "Publish invitations").await;
    pty.click("Publish invitations").await;
    pty.shows("Waiting").await;
    pty.type_bytes(b"\x1b");
    tokio::time::sleep(Duration::from_millis(80)).await;
    pty.key(b"\x1bv", "▾ Delivery").await;
    pty.capture("revealed-selection");

    // A named external link copies, and the next link opens another track.
    pty.click("Publish invitations").await;
    pty.type_bytes(b"\x1b[C\r");
    tokio::time::sleep(Duration::from_millis(80)).await;
    pty.drain();
    assert!(
        pty.wire
            .windows(b"]52;c;aHR0cHM6Ly9leGFtcGxlLmNvbS9ndWlkZQ==".len())
            .any(|bytes| bytes == b"]52;c;aHR0cHM6Ly9leGFtcGxlLmNvbS9ndWlkZQ=="),
        "named address copied through OSC 52"
    );
    pty.type_bytes(b"\x1b[C\r");
    pty.shows("Complete").await;
    pty.capture("cross-track-link");
    pty.click("Publish invitations").await;

    // Dependency browsing never chooses a different item merely by opening it.
    pty.key(b"\x1bd", "Dependencies").await;
    pty.capture("dependencies");
    pty.key(b"\x1bd", "Tracks").await;
    pty.click("Publish invitations").await;
    pty.click("Reading checkpoint").await;
    pty.type_bytes(b"\x1b[H\x1b[6~");
    tokio::time::sleep(Duration::from_millis(80)).await;
    let checkpoint = reading_checkpoint(&mut pty);
    pty.capture("long-reading");
    // Completed history remains a normal record, then reveal restores active work.
    pty.type_bytes(b"\x1bf\x1bf\x1bf\x1bf\x1bf");
    pty.shows("Previous delivery").await;
    pty.click("Previous delivery").await;
    pty.capture("completed-history");
    pty.type_bytes(b"\x1b/");
    pty.type_bytes(b"Publish");
    pty.click("Publish invitations").await;
    pty.key(b"\x1bv", "Status: Active").await;
    pty.click("Publish invitations").await;
    pty.click("Reading checkpoint").await;

    // Narrow drilldown and return use real winsize polling, not Msg::Resize.
    let began = Instant::now();
    pty.resize(NARROW);
    pty.shows("Back to roadmap (Esc)").await;
    pty.resize_latency.push(began.elapsed());
    pty.capture("narrow-reading");
    let began = Instant::now();
    pty.resize(WIDE);
    pty.shows("Tracks").await;
    pty.resize_latency.push(began.elapsed());
    pty.shows("Reading checkpoint").await;

    // Focus the composer, submit through real input, admit the response at its owner.
    pty.click("Request a change").await;
    pty.type_bytes(b"\x15");
    pty.type_bytes(fixture::REQUEST.as_bytes());
    pty.type_bytes(b"\r");
    let submitted = tokio::time::timeout(LIMIT, async {
        loop {
            if let Cmd::RoadmapPrompt(text) = commands.next().await.expect("runtime command") {
                break text;
            }
        }
    })
    .await
    .expect("planning request submitted");
    assert_eq!(submitted, fixture::REQUEST);
    inbox.send(Msg::RoadmapRequestStarted);
    inbox.send(Msg::Agent(AgentEvent::Text(Scenes::RICH_BODY.into())));
    pty.shows("Delivery findings").await;
    pty.capture("rich-request");
    // Hold older replies while later lines arrive; browse recorded reasoning concurrently.
    pty.type_bytes(b"\x1b[H");
    let producer = tokio::task::spawn_local({
        let inbox = inbox.clone();
        async move {
            for n in 0..200 {
                inbox.send(Msg::Agent(AgentEvent::Text(format!(
                    "Streaming receipt {n:03}.\n"
                ))));
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            inbox.send(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
        }
    });
    pty.click("›").await;
    for (n, c) in fixture::DRAFT.char_indices() {
        let expected = &fixture::DRAFT[..n + c.len_utf8()];
        pty.key(c.to_string().as_bytes(), expected).await;
    }
    pty.click("Session").await;
    pty.shows("Review delivery findings").await;
    pty.click("Review delivery findings").await;
    pty.capture("recorded-reading");
    pty.click("Roadmap").await;
    pty.click("Publish invitations").await;
    pty.key(b"\x1bm", "Requests").await;
    pty.shows("Review the invitation delivery").await;
    pty.capture("held-request");
    producer.await.expect("stream producer");
    pty.key(b"\x1bm", &checkpoint).await;
    assert_eq!(reading_checkpoint(&mut pty), checkpoint);
    pty.capture("returned-reading");

    // A source update keeps identity, and completing its prerequisite changes readiness.
    inbox.send(Msg::Engine(EngineEvent::local(fixtures::frame("roadmap","chunk_completed",
        serde_json::json!({"chunk":{"id":"storage","name":"Receipt storage","title":"Receipt storage","status":"done","priority":50,"group":"Storage"}})))));
    pty.shows("Ready 2").await;
    pty.capture("updated-readiness");
    quit.cancel();
    let mut model = tokio::time::timeout(LIMIT, running)
        .await
        .expect("bounded shutdown")
        .expect("runtime task")
        .expect("runtime");
    assert_eq!(model.selected(), Some(&Selected::Chunk("waiting".into())));
    assert!(
        model.roadmap().track_rows().iter().any(|row| matches!(
            row,
            magistr_cockpit::cockpit::TrackRow::Heading {
                key: "Track 00",
                expanded: false,
                ..
            }
        )),
        "unrelated collapse survives the journey"
    );
    assert_eq!(model.prompt().text(), fixture::DRAFT);
    assert_eq!(model.prompt().history(), [fixture::REQUEST]);
    assert_eq!(model.roadmap_requests().len(), 1);
    assert_eq!(
        model.roadmap_requests()[0]
            .response(model.turn())
            .last()
            .unwrap()
            .text(),
        "Streaming receipt 199."
    );
    assert!(
        model.roadmap_request_scroll() != usize::MAX,
        "older request position stays held"
    );
    let source = Inspection::of(&model).expect("selected work").original();
    for text in [
        "Keep the original receipt and command.",
        "let receipt = 7;",
        "| Restart | Needs work |",
        "https://example.com/guide",
    ] {
        assert!(
            source.contains(text),
            "complete authored text contains {text}"
        );
    }

    captures(&model, theme, "return");
    let quit = CancellationToken::new();
    let runtime = Runtime::new(
        pty.terminal(),
        Options::new(Env::new("xterm", false, None))
            .probe_timeout(Duration::ZERO)
            .quit_on(quit.clone()),
    );
    let inbox = runtime.inbox();
    let running = tokio::task::spawn_local(runtime.run(model));
    pty.shows(&checkpoint).await;
    let mut snapshot = fixture::snapshot();
    snapshot
        .chunks
        .iter_mut()
        .find(|chunk| chunk.id == "storage")
        .expect("completed prerequisite")
        .status = magistr_engine::ChunkStatus::Done;
    snapshot.progress.checked_tasks += 1;
    snapshot.chunks.retain(|chunk| chunk.id != "waiting");
    snapshot.progress.total_tasks -= 1;
    inbox.send(Msg::Engine(EngineEvent::Seed(snapshot)));
    pty.shows("Item unavailable").await;
    pty.capture("deleted-selection");
    quit.cancel();
    model = tokio::time::timeout(LIMIT, running)
        .await
        .expect("bounded deletion shutdown")
        .expect("runtime task")
        .expect("runtime");
    assert_eq!(model.selected(), Some(&Selected::Chunk("waiting".into())));
    assert!(
        Inspection::of(&model)
            .expect("unavailable record")
            .original()
            .contains("no longer available")
    );
    captures(&model, theme, "deleted");
    pty.persist(match theme {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
    });
    latency(&pty);
}

fn reading_checkpoint(pty: &mut Pty) -> String {
    let screen = pty.screen();
    let number = screen
        .split("Reading checkpoint ")
        .nth(1)
        .expect("visible reading checkpoint")
        .chars()
        .take(3)
        .collect::<String>();
    format!("Reading checkpoint {number}")
}

fn captures(model: &Model, theme: ThemeMode, state: &str) {
    // Live runtime state includes measured activity and elapsed time.
    // Preserve these as evidence; deterministic goldens have their own suite.
    let temporary = tempfile::tempdir().expect("capture directory");
    let directory = std::env::var_os("MAGISTR_JOURNEY_EVIDENCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| temporary.path().to_owned());
    std::fs::create_dir_all(&directory).expect("capture directory");
    for size in [WIDE, NARROW] {
        for profile in [Profile::Cells, Profile::Kitty] {
            let mut model = model.clone();
            update(&mut model, Msg::Resize(size));
            if profile == Profile::Kitty {
                update(
                    &mut model,
                    Msg::Mode(magistr_cockpit::capability::Mode::Graphics {
                        pointer_shapes: true,
                    }),
                );
            }
            let image = frame(&model, &FrameClock::new(0, Duration::ZERO), profile)
                .expect("production capture");
            let name = format!(
                "journey-{state}-{}-{}x{}{}",
                if theme == ThemeMode::Dark {
                    "mocha"
                } else {
                    "latte"
                },
                size.cols,
                size.rows,
                profile.suffix()
            );
            image
                .save_png(directory.join(format!("{name}.png")))
                .expect("save production capture");
        }
    }
}

fn latency(pty: &Pty) {
    let mut samples = pty.input_latency.clone();
    samples.sort();
    let p90 = samples[samples.len() * 9 / 10];
    let max = *samples.last().unwrap();
    eprintln!(
        "real PTY: {} records, {} input observations, input p90 {p90:?}, max {max:?}; resize {:?}; release={} (observed bytes, 1ms polling)",
        fixture::RECORDS,
        samples.len(),
        pty.resize_latency,
        !cfg!(debug_assertions)
    );
    // Existing streaming baseline: release input p90 20 ms, max 40 ms.
    if !cfg!(debug_assertions) {
        assert!(p90 < Duration::from_millis(20), "input p90 {p90:?}");
        assert!(max < Duration::from_millis(40), "input max {max:?}");
    }
}
