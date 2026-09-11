use super::*;
use crate::{
    extensions::{LaneFuture, RunnerLaneOutcome},
    worker::{WorkerRunFailure, WorkerRunReport},
};
use std::sync::Mutex;

struct FakeRunner {
    answer: Result<RunnerLaneOutcome, String>,
    calls: Mutex<Vec<(RunnerInput, PluginGuestPolicy)>>,
}

impl ExtensionLane for FakeRunner {
    fn parse_run_output(
        &self,
        _guest: Vec<u8>,
        input: RunnerInput,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, RunnerLaneOutcome> {
        assert!(
            capabilities.is_empty(),
            "a runner declares zero capabilities"
        );
        self.calls.lock().unwrap().push((input, policy));
        let answer = self.answer.clone();
        Box::pin(async move {
            answer.map_err(|message| {
                crate::contracts::WorkspaceError::new(ErrorKind::EngineUnavailable, message)
            })
        })
    }
}

fn input(command: &str, exit_code: i32, stdout: &str) -> RunnerInput {
    RunnerInput {
        command: command.into(),
        exit_code,
        stdout: stdout.into(),
        stderr: String::new(),
    }
}

fn report() -> WorkerRunReport {
    WorkerRunReport {
        failures: vec![WorkerRunFailure {
            test_name: "TestX".into(),
            file: Some("x_test.go".into()),
            assertion: "nope".into(),
            trace: "t".into(),
        }],
        summary: Some("FAIL\tm\t0.1s".into()),
    }
}

fn lane(answer: Result<RunnerLaneOutcome, String>) -> FakeRunner {
    FakeRunner {
        answer,
        calls: Mutex::new(Vec::new()),
    }
}

#[tokio::test]
async fn a_healthy_lane_answers_the_lifted_report_and_null_is_the_honest_no_structure() {
    let healthy = lane(Ok(RunnerLaneOutcome {
        end: WorkerGuestEnd::Ok,
        fuel_used: 10,
        claimed: true,
        report: Some(report()),
    }));
    let adapter = RunnerAdapter::new("go", b"(component)".to_vec(), &healthy);
    assert_eq!(adapter.name(), "go");
    let parsed = adapter
        .parse(input("go test", 1, "--- FAIL: TestX (0.00s)"))
        .await
        .unwrap();
    assert_eq!(parsed, Some(report()));
    let first = healthy.calls.lock().unwrap()[0].clone();
    assert_eq!(first.0.command, "go test");
    assert_eq!(first.1.fuel, 200_000_000);
    assert_eq!(first.1.memory_cap_bytes, 32 * 1024 * 1024);

    let empty = lane(Ok(RunnerLaneOutcome {
        end: WorkerGuestEnd::Ok,
        fuel_used: 10,
        claimed: true,
        report: None,
    }));
    let adapter = RunnerAdapter::new("go", Vec::new(), &empty);
    assert_eq!(adapter.parse(input("go test", 2, "")).await.unwrap(), None);
}

#[tokio::test]
async fn a_dead_lane_a_trap_and_an_unclaimed_command_are_typed_errors_for_the_floor() {
    let dead = lane(Err("engine child down".into()));
    let adapter = RunnerAdapter::new("go", Vec::new(), &dead);
    let error = adapter.parse(input("go test", 1, "")).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::EngineUnavailable);

    let trapped = lane(Ok(RunnerLaneOutcome {
        end: WorkerGuestEnd::GuestTrap("unreachable".into()),
        fuel_used: 5,
        claimed: false,
        report: None,
    }));
    let adapter =
        RunnerAdapter::new("go", Vec::new(), &trapped).with_policy(PluginGuestPolicy::ceiling());
    let error = adapter.parse(input("go test", 1, "")).await.unwrap_err();
    assert!(matches!(error.kind, ErrorKind::PluginCrashed { ref plugin } if plugin == "go"));
    let policy = trapped.calls.lock().unwrap()[0].1;
    assert_eq!(policy, PluginGuestPolicy::ceiling());

    let unclaimed = lane(Ok(RunnerLaneOutcome {
        end: WorkerGuestEnd::Ok,
        fuel_used: 1,
        claimed: false,
        report: None,
    }));
    let adapter = RunnerAdapter::new("go", Vec::new(), &unclaimed);
    let error = adapter.parse(input("cargo test", 1, "")).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::CommandNotDeclared);
}
