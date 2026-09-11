//! Pinned guest fixtures exercise resource limits from a normal Rust binary.

#![cfg(feature = "sandbox")]

use magistr_workspace::sandbox::{
    ComponentOutcome, GuestEnd, SandboxPolicy, run_component, run_wasi_module,
};

const POLICY: SandboxPolicy = SandboxPolicy {
    fuel: 1_000_000,
    memory_cap_bytes: 10 * 65536,
};

const ADD: &[u8] = include_bytes!("../../../subsystems/workspace/conformance/add.wat");
const SPIN: &[u8] = include_bytes!("../../../subsystems/workspace/conformance/spin.wat");
const HOG: &[u8] = include_bytes!("../../../subsystems/workspace/conformance/hog.wat");
const CRASH: &[u8] = include_bytes!("../../../subsystems/workspace/conformance/crash.wat");
const OBSERVE: &[u8] = include_bytes!("../../../subsystems/workspace/conformance/observe.wat");
const PREOPEN_PROBE: &[u8] =
    include_bytes!("../../../subsystems/workspace/conformance/preopen_probe.wat");
const HELLO_WASI: &[u8] =
    include_bytes!("../../../subsystems/workspace/conformance/hello_wasi.wat");

fn run(guest: &[u8], export: &str, args: &[i64], capabilities: &[String]) -> ComponentOutcome {
    run_component(guest, export, args, capabilities, POLICY)
        .expect("guest errors remain ordinary outcome data")
}

#[test]
fn consumer_survives_deterministic_guest_limits_and_traps() {
    for (guest, export, expected) in [
        (SPIN, "spin", GuestEnd::OutOfFuel),
        (HOG, "hog", GuestEnd::MemoryCap),
    ] {
        let first = run(guest, export, &[], &[]);
        let second = run(guest, export, &[], &[]);
        assert_eq!(first.end, expected);
        assert_eq!(second.end, expected);
        assert_eq!(first.fuel_used, second.fuel_used);
    }
    assert!(matches!(
        run(CRASH, "crash", &[], &[]).end,
        GuestEnd::GuestTrap(_)
    ));
    let alive = run(ADD, "add", &[19, 23], &[]);
    assert_eq!(alive.end, GuestEnd::Ok);
    assert_eq!(alive.results, ["U32(42)"]);
    assert!(alive.fuel_used > 0);
}

#[test]
fn capabilities_are_denied_before_execution_and_granted_explicitly() {
    let denied = run(OBSERVE, "run", &[], &[]);
    assert_eq!(denied.end, GuestEnd::CapDenied("observe".to_owned()));
    assert_eq!(denied.fuel_used, 0);
    let allowed = run(OBSERVE, "run", &[], &["observe".to_owned()]);
    assert_eq!(allowed.end, GuestEnd::Ok);
    assert_eq!(allowed.observations, [7, 11, 13]);
    assert_eq!(allowed.results, ["U32(3)"]);
    assert!(matches!(
        run(b"not wasm", "run", &[], &[]).end,
        GuestEnd::InvalidGuest(_)
    ));
}

#[test]
fn wasi_has_zero_preopens_and_captures_output() {
    let probe = run_wasi_module(PREOPEN_PROBE, &["probe".to_owned()], POLICY)
        .expect("WASI guest condition is data");
    assert_eq!(probe.end, GuestEnd::GuestExit(8));
    assert_eq!(probe.exit_code, Some(8));
    let hello = run_wasi_module(HELLO_WASI, &["hello".to_owned()], POLICY)
        .expect("WASI guest runs within the policy");
    assert_eq!(hello.end, GuestEnd::Ok);
    assert_eq!(hello.stdout, "jailed");
    assert!(hello.stderr.is_empty());
}
