//! Workflow libraries never own a terminal presentation.
//!
//! Routine narration and capability negotiation use opt-in debug tracing.
//! Findings, plans and gate results remain in native records and tool replies;
//! terminal clients render them from their ordered session stream. Genuine
//! effect failures still use warn tracing and reach the operational log lane.
//! Protocol stdout stays reserved for protocol frames.

use std::path::{Path, PathBuf};

#[test]
fn routine_reasoning_records_do_not_write_to_the_hosts_stderr() {
    const CHILD: &str = "MAGISTR_TERMINAL_LOG_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_writer(std::io::stderr)
            .without_time()
            .init();
        let mut config = magistr_workflow::think::config::ThinkConfig::default();
        config.system.max_history_size = 1;
        let mut owner = magistr_workflow::think::engine::core::ReasoningServer::new(config);
        for _ in 0..3 {
            let step = serde_json::from_value(serde_json::json!({
                "estimated_total":6,"purpose":"Record a material finding",
                "context":"Observed session output","thought":"The shared owner retains the current facts.",
                "outcome":"The result is available to every current reader.",
                "rationale":"The contract is exercised through the actual reasoning owner.",
                "next_action":"Verify the terminal presentation"
            })).unwrap();
            owner.process_step_native(step).unwrap();
        }
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "routine_reasoning_records_do_not_write_to_the_hosts_stderr",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "routine records polluted stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn src(rel: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Lines that are genuinely a `macro!(` CALL, not a mention inside a string or
/// a doc comment. Matching WITH the leading indentation is the whole point: a
/// source-text gate that also matches prose fails open on the case it exists to
/// catch.
fn call_lines(text: &str, macro_name: &str) -> Vec<usize> {
    let call = format!("{macro_name}!");
    text.lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim_start();
            t.starts_with(&call) || t.contains(&format!("=> {call}"))
        })
        .map(|(i, _)| i)
        .collect()
}

/// Workflow owners and their protocol adapters.
const OPERATIONAL: &[&str] = &[
    "infra/persistence.rs",
    "think/persistence.rs",
    "think/config.rs",
    "think/engine/revisions.rs",
    "think/engine/validation.rs",
    "think/engine/branching.rs",
    "think/engine/sessions.rs",
    "think/engine/process.rs",
    "think/engine/mutations.rs",
    "think/engine/numbering.rs",
    "ship/persistence.rs",
    "ship/engine/mod.rs",
    "roadmap/engine/mod.rs",
    "signal/engine.rs",
    "mcp/elicit.rs",
    "mcp/tasks.rs",
];

/// Words that only appear when a line is reporting that something did not work.
/// Chosen from the actual converted set, not invented.
const FAILURE_MARKERS: &[&str] = &[
    "Failed to",
    "failed:",
    "failed to",
    "could not",
    "Cannot ",
    "Refusing",
    "Skipping",
    "Invalid ",
    "Ignoring",
];

/// The completeness half: no failure report is left behind on `eprintln!`,
/// where the log lane is structurally blind to it.
#[test]
fn every_failure_report_outside_the_cli_reaches_the_subscriber() {
    let mut stranded = Vec::new();
    for rel in OPERATIONAL {
        let text = src(rel);
        let lines: Vec<&str> = text.lines().collect();
        for i in call_lines(&text, "eprintln") {
            // A macro call can span lines; the format string is within the next
            // few of them.
            let site = lines[i..(i + 4).min(lines.len())].join(" ");
            if let Some(marker) = FAILURE_MARKERS.iter().find(|m| site.contains(**m)) {
                stranded.push(format!("{rel}:{} (matched {marker:?})", i + 1));
            }
        }
    }
    assert!(
        stranded.is_empty(),
        "these sites report a failure but still use eprintln!, so the OTLP log \
         lane cannot see them — convert them to tracing::warn! or, if one is \
         genuinely not a failure, reword it:\n  {}",
        stranded.join("\n  ")
    );
}

/// An embedded workflow engine must not print into its host's terminal.
#[test]
fn workflow_libraries_do_not_print_routine_narration() {
    for rel in OPERATIONAL {
        let text = src(rel);
        assert!(
            call_lines(&text, "eprintln").is_empty(),
            "{rel} prints directly; use tracing for diagnostics and return user-visible facts through native records"
        );
    }
}

/// Loads and repairs keep their actual severity in tracing.
#[test]
fn library_load_narration_stays_on_tracing() {
    let text = src("roadmap/engine/mod.rs");
    let lines: Vec<&str> = text.lines().collect();
    for (needle, level) in [
        ("loaded roadmap with", "tracing::info!"),
        ("minted an id for", "tracing::warn!"),
        ("contentless chunk(s) that had no id", "tracing::warn!"),
        ("chunk(s) that had no name", "tracing::warn!"),
        ("whose derived labels collided", "tracing::warn!"),
    ] {
        let at = lines
            .iter()
            .position(|l| l.contains(needle) && !l.trim_start().starts_with("//"))
            .unwrap_or_else(|| panic!("roadmap/engine/mod.rs no longer emits {needle:?}"));
        let site = lines[at.saturating_sub(2)..=at].join(" ");
        assert!(
            site.contains(level) && !site.contains("eprintln!"),
            "roadmap/engine/mod.rs emits {needle:?} through something other than \
             {level}. The embedding client owns terminal presentation."
        );
    }
}

/// The behavioural half, so the whole file is not source text: the target a
/// converted site emits under really does clear the fence, and the level a
/// narration site would have used really does not.
#[test]
fn the_fence_admits_a_converted_site_and_rejects_the_level_narration_would_use() {
    use magistr_workflow::otel_logs::passes_fence;
    use tracing::Level;

    // `tracing`'s default target is the module path, so a warn! inside the crate
    // arrives as `magistr_workflow::…` with no explicit target argument needed.
    for target in [
        "magistr_workflow::think::persistence",
        "magistr_workflow::ship::persistence",
        "magistr_workflow::signal::engine",
        "magistr_workflow::mcp::elicit",
    ] {
        assert!(
            passes_fence(target, &Level::WARN),
            "{target} at WARN must reach the lane — every converted site is WARN"
        );
        assert!(
            !passes_fence(target, &Level::INFO),
            "{target} at INFO is routine diagnostic data and must not reach the failure lane"
        );
    }
    assert!(
        !passes_fence("hyper::client", &Level::WARN),
        "the fence must stay closed to dependencies' warnings"
    );
}
