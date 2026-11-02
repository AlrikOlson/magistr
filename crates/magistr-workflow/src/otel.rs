//! OTel GenAI export (`otel-genai-export`) — speak the converged language.
//!
//! Maps the local stores onto the OTLP/HTTP JSON body (`{resourceSpans: …}`)
//! using the OpenTelemetry GenAI semantic conventions' agent spans, so a
//! workspace trace imports into Jaeger / Grafana / Datadog / any OTLP
//! endpoint with one curl (see README "OpenTelemetry export").
//!
//! Mapping (one deterministic trace per project):
//! - root span `workspace <project>`;
//! - the run cycle nests `goal → step → progress/check` (a failed check
//!   sets span status ERROR — the gate failed, not the step);
//! - decision notes become reasoning spans parented to the step named by their
//!   `run_ref` (`step:<id>`) when present, else to the root.
//!
//! Attribute policy: `gen_ai.operation.name` is set only where a semconv
//! value genuinely fits (`invoke_agent` for goal/steps, `execute_tool`
//! for progress/checks); domain truth rides namespaced `magistr.workflow.*`
//! attributes. Ids are sha256-derived from record identities — the same
//! corpus always exports the same trace.
//!
//! Honest limits: the local run store holds the CURRENT cycle only (one
//! goal tree per export); decision notes without timestamps are skipped
//! and counted, never fabricated; checks are zero-duration spans at their
//! recorded instant.

use chrono::DateTime;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::decision::domain::note::Note;
use crate::run::domain::goal::Goal;
use crate::run::domain::step::Step;
use crate::trace_context::InboundTrace;

/// A built export plus what was (honestly) left out.
#[derive(Debug)]
pub struct OtelExport {
    pub body: Value,
    pub spans: usize,
    pub skipped_steps: usize,
}

fn hex_bytes(input: &str, len: usize) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest[..len].iter().map(|b| format!("{b:02x}")).collect()
}

/// 16-byte (32 hex char) deterministic trace id for a project.
pub fn trace_id(project: &str) -> String {
    hex_bytes(&format!("trace:{project}"), 16)
}

/// 8-byte (16 hex char) deterministic span id for a record identity.
pub fn span_id(kind: &str, id: &str) -> String {
    hex_bytes(&format!("span:{kind}:{id}"), 8)
}

fn nanos(rfc3339: &str) -> Option<u64> {
    DateTime::parse_from_rfc3339(rfc3339)
        .ok()
        .and_then(|t| u64::try_from(t.timestamp_nanos_opt()?).ok())
}

pub(crate) fn attr(key: &str, value: &str) -> Value {
    json!({ "key": key, "value": { "stringValue": value } })
}

pub(crate) fn attr_bool(key: &str, value: bool) -> Value {
    json!({ "key": key, "value": { "boolValue": value } })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn span(
    trace: &str,
    id: String,
    parent: Option<&str>,
    name: &str,
    start: u64,
    end: u64,
    attrs: Vec<Value>,
    error: bool,
) -> Value {
    let mut s = json!({
        "traceId": trace,
        "spanId": id,
        "name": name,
        "kind": 1, // SPAN_KIND_INTERNAL
        "startTimeUnixNano": start.to_string(),
        "endTimeUnixNano": end.max(start).to_string(),
        "attributes": attrs,
        "status": if error { json!({ "code": 2 }) } else { json!({ "code": 0 }) },
    });
    if let Some(p) = parent {
        s["parentSpanId"] = json!(p);
    }
    s
}

/// Build the OTLP/HTTP JSON body from in-memory store contents. Pure and
/// deterministic: same inputs, same bytes.
///
/// `inbound` is a caller's adopted W3C Trace Context (SEP-414). When present,
/// the export JOINS that trace instead of minting its own: every span carries
/// the caller's trace id and the root span parents to the caller's span, so
/// POSTing this body to the backend the host already writes to yields one span
/// tree rather than two. Our sha256 span ids are kept either way — they only
/// need uniqueness within a trace, and keeping them is what preserves
/// determinism.
///
/// With `inbound: None` the output is byte-identical to the pre-SEP-414
/// exporter.
pub fn build_otel<'a>(
    project: &str,
    run: Option<(&Goal, &[Step])>,
    steps: impl Iterator<Item = &'a Note>,
    inbound: Option<&InboundTrace>,
) -> OtelExport {
    let trace = match inbound {
        Some(c) => c.trace_id.clone(),
        None => trace_id(project),
    };
    let root_id = span_id("workspace", project);
    let mut spans: Vec<Value> = Vec::new();
    let mut skipped_steps = 0usize;
    let mut min_start = u64::MAX;
    let mut max_end = 0u64;
    let bump = |s: u64, e: u64, min_start: &mut u64, max_end: &mut u64| {
        *min_start = (*min_start).min(s);
        *max_end = (*max_end).max(e);
    };

    // Run cycle: goal → step → progress/check.
    if let Some((goal, steps)) = run {
        let obj_start = goal.created_at.as_deref().and_then(nanos).unwrap_or(0);
        let obj_end = goal
            .completed_at
            .as_deref()
            .and_then(nanos)
            .unwrap_or(obj_start + 1_000_000);
        bump(obj_start, obj_end, &mut min_start, &mut max_end);
        let obj_id = span_id("goal", goal.created_at.as_deref().unwrap_or("current"));
        spans.push(span(
            &trace,
            obj_id.clone(),
            Some(&root_id),
            "invoke_agent goal",
            obj_start,
            obj_end,
            vec![
                attr("gen_ai.operation.name", "invoke_agent"),
                attr("gen_ai.agent.name", project),
                attr("magistr.workflow.family", "run"),
                attr("magistr.workflow.kind", "goal"),
                attr("magistr.workflow.title", &goal.description),
            ],
            false,
        ));

        for step in steps {
            let t_start = step
                .started_at
                .as_deref()
                .and_then(nanos)
                .unwrap_or(obj_start);
            let t_end = step
                .completed_at
                .as_deref()
                .and_then(nanos)
                .unwrap_or(t_start + 1_000_000);
            bump(t_start, t_end, &mut min_start, &mut max_end);
            let t_id = span_id("step", &step.id);
            spans.push(span(
                &trace,
                t_id.clone(),
                Some(&obj_id),
                &format!("invoke_agent step {}", step.id),
                t_start,
                t_end,
                vec![
                    attr("gen_ai.operation.name", "invoke_agent"),
                    attr("gen_ai.agent.name", project),
                    attr("magistr.workflow.family", "run"),
                    attr("magistr.workflow.kind", "step"),
                    attr("magistr.workflow.id", &step.id),
                    attr("magistr.workflow.title", &step.title),
                ],
                false,
            ));

            for progress in &step.progress {
                let a_start = nanos(&progress.timestamp).unwrap_or(t_start);
                let a_end = a_start + 1_000_000;
                bump(a_start, a_end, &mut min_start, &mut max_end);
                spans.push(span(
                    &trace,
                    span_id("progress", &progress.id.to_string()),
                    Some(&t_id),
                    &format!("execute_tool progress {}", progress.id),
                    a_start,
                    a_end,
                    vec![
                        attr("gen_ai.operation.name", "execute_tool"),
                        attr("magistr.workflow.family", "run"),
                        attr("magistr.workflow.kind", "progress"),
                        attr("magistr.workflow.id", &progress.id.to_string()),
                    ],
                    false,
                ));
            }
            for (seq, check) in step.checks.iter().enumerate() {
                // Zero-duration at the check's own recorded instant.
                let c_at = nanos(&check.timestamp).unwrap_or(t_end);
                bump(c_at, c_at, &mut min_start, &mut max_end);
                spans.push(span(
                    &trace,
                    span_id("check", &format!("{}:{seq}", step.id)),
                    Some(&t_id),
                    &format!("execute_tool check {}", check.name),
                    c_at,
                    c_at,
                    vec![
                        attr("gen_ai.operation.name", "execute_tool"),
                        attr("magistr.workflow.family", "run"),
                        attr("magistr.workflow.kind", "check"),
                        attr("magistr.workflow.check.name", &check.name),
                        attr_bool("magistr.workflow.check.passed", check.passed),
                    ],
                    !check.passed,
                ));
            }
        }
    }

    // Decision notes: reasoning spans, parented via run_ref when it names a step.
    let step_span_of = |exec: Option<&str>| -> Option<String> {
        exec.and_then(|e| e.strip_prefix("step:"))
            .map(|t| span_id("step", t))
    };
    for step in steps {
        let Some(start) = step.timestamp.as_deref().and_then(nanos) else {
            skipped_steps += 1;
            continue;
        };
        let end = start + step.duration_ms.unwrap_or(1).max(1) * 1_000_000;
        bump(start, end, &mut min_start, &mut max_end);
        // Parent only to spans that exist in THIS export (the current run
        // cycle's steps); historical run_refs fall back to the root.
        let parent = step_span_of(step.run_ref.as_deref())
            .filter(|p| spans.iter().any(|s| s["spanId"] == *p))
            .unwrap_or_else(|| root_id.clone());
        let mut attrs = vec![
            attr("gen_ai.agent.name", project),
            attr("magistr.workflow.family", "decision"),
            attr("magistr.workflow.kind", "step"),
            attr("magistr.workflow.id", &step.note.to_string()),
            attr("magistr.workflow.title", &step.purpose),
        ];
        if let Some(c) = step.confidence {
            attrs.push(attr("magistr.workflow.step.confidence", &format!("{c}")));
        }
        spans.push(span(
            &trace,
            span_id("step", &step.note.to_string()),
            Some(&parent),
            &format!("reasoning step {}", step.note),
            start,
            end,
            attrs,
            false,
        ));
    }

    // Root span wraps everything that made it in.
    if min_start == u64::MAX {
        min_start = 0;
        max_end = 1;
    }
    spans.insert(
        0,
        span(
            &trace,
            root_id,
            inbound.map(|c| c.parent_span_id.as_str()),
            &format!("workspace {project}"),
            min_start,
            max_end,
            vec![
                attr("gen_ai.agent.name", project),
                attr("magistr.workflow.family", "workspace"),
            ],
            false,
        ),
    );

    let count = spans.len();
    let body = json!({
        "resourceSpans": [{
            "resource": {
                "attributes": [
                    attr("service.name", "magistr"),
                    attr("gen_ai.system", "magistr"),
                    attr("magistr.workflow.project", project),
                ]
            },
            "scopeSpans": [{
                "scope": { "name": "magistr", "version": env!("CARGO_PKG_VERSION") },
                "spans": spans,
            }]
        }]
    });
    OtelExport {
        body,
        spans: count,
        skipped_steps,
    }
}

/// Structural OTLP/JSON validation — the contract the README demo depends on.
/// Returns human-readable problems (empty = valid). Checks: hex id shapes,
/// parent resolution within the trace, time ordering, non-empty names, and
/// the `service.name` resource attribute.
///
/// Validates a self-contained export: every `parentSpanId` must resolve inside
/// the body. For a joined export use
/// [`validate_otlp_with_external_parent`].
pub fn validate_otlp(body: &Value) -> Vec<String> {
    validate_otlp_with_external_parent(body, None)
}

/// [`validate_otlp`], plus the one span id that is allowed to live outside the
/// body: the caller's span, when this export has joined an inbound trace.
///
/// The permitted id is a DECLARED fact, not a guess. Relaxing the rule to
/// "a parent may be missing" would retire the check entirely and let a genuine
/// internal dangling-parent bug through forever; naming the single external id
/// keeps every other dangling parent an error.
pub fn validate_otlp_with_external_parent(body: &Value, external: Option<&str>) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(resource_spans) = body["resourceSpans"].as_array() else {
        return vec!["resourceSpans missing or not an array".into()];
    };
    for rs in resource_spans {
        let has_service = rs["resource"]["attributes"]
            .as_array()
            .is_some_and(|a| a.iter().any(|x| x["key"] == "service.name"));
        if !has_service {
            errors.push("resource lacks service.name".into());
        }
        for ss in rs["scopeSpans"].as_array().unwrap_or(&Vec::new()) {
            let spans = ss["spans"].as_array().cloned().unwrap_or_default();
            let ids: Vec<&str> = spans.iter().filter_map(|s| s["spanId"].as_str()).collect();
            for s in &spans {
                let name = s["name"].as_str().unwrap_or("");
                if name.is_empty() {
                    errors.push("span with empty name".into());
                }
                let tid = s["traceId"].as_str().unwrap_or("");
                if tid.len() != 32 || !tid.chars().all(|c| c.is_ascii_hexdigit()) {
                    errors.push(format!("span {name}: bad traceId"));
                }
                let sid = s["spanId"].as_str().unwrap_or("");
                if sid.len() != 16 || !sid.chars().all(|c| c.is_ascii_hexdigit()) {
                    errors.push(format!("span {name}: bad spanId"));
                }
                if let Some(p) = s["parentSpanId"].as_str()
                    && !ids.contains(&p)
                    && external != Some(p)
                {
                    errors.push(format!("span {name}: dangling parentSpanId"));
                }
                let start = s["startTimeUnixNano"]
                    .as_str()
                    .and_then(|v| v.parse::<u64>().ok());
                let end = s["endTimeUnixNano"]
                    .as_str()
                    .and_then(|v| v.parse::<u64>().ok());
                match (start, end) {
                    (Some(a), Some(b)) if a <= b => {}
                    _ => errors.push(format!("span {name}: bad time range")),
                }
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(n: u32, ts: &str, exec: Option<&str>) -> Note {
        let mut s: Note = serde_json::from_str(&format!("{{\"note\":{n}}}")).expect("step");
        s.timestamp = Some(ts.into());
        s.run_ref = exec.map(str::to_owned);
        s.purpose = format!("step {n}");
        s
    }

    fn cycle() -> (Goal, Vec<Step>) {
        let goal: Goal = serde_json::from_value(serde_json::json!({
            "description": "run the thing",
            "acceptance_criteria": [],
            "constraints": [],
            "scope": "",
            "status": "active",
            "project_id": "proj",
            "created_at": "2026-06-10T10:00:00Z",
        }))
        .expect("goal");
        let step: Step = serde_json::from_value(serde_json::json!({
            "id": "implement",
            "title": "Implement",
            "type": "implement",
            "status": "done",
            "estimate": null,
            "started_at": "2026-06-10T10:05:00Z",
            "completed_at": "2026-06-10T11:00:00Z",
            "progress": [{
                "id": 1, "step_id": "implement", "timestamp": "2026-06-10T10:10:00Z",
                "type": "code", "description": "did it", "result": ""
            }],
            "checks": [{
                "type": "test", "name": "cargo test", "passed": false,
                "details": "1 failed", "required": true, "verified": false,
                "timestamp": "2026-06-10T10:55:00Z"
            }],
        }))
        .expect("step");
        (goal, vec![step])
    }

    #[test]
    fn exports_a_valid_goal_step_progress_check_tree() {
        let (obj, run_steps) = cycle();
        let steps = [step(7, "2026-06-10T10:06:00Z", Some("step:implement"))];
        let out = build_otel("proj", Some((&obj, &run_steps)), steps.iter(), None);
        assert_eq!(validate_otlp(&out.body), Vec::<String>::new());
        assert_eq!(out.skipped_steps, 0);

        let spans = out.body["resourceSpans"][0]["scopeSpans"][0]["spans"]
            .as_array()
            .unwrap()
            .clone();
        // root + goal + step + progress + check + step
        assert_eq!(spans.len(), 6);
        let by_name = |n: &str| {
            spans
                .iter()
                .find(|s| s["name"].as_str().unwrap().contains(n))
                .unwrap()
        };
        let step_span = by_name("step implement");
        let progress = by_name("progress 1");
        let check = by_name("check cargo test");
        let reasoning = by_name("reasoning step 7");
        assert_eq!(progress["parentSpanId"], step_span["spanId"]);
        assert_eq!(check["parentSpanId"], step_span["spanId"]);
        // run_ref step:implement parents the reasoning span to the step.
        assert_eq!(reasoning["parentSpanId"], step_span["spanId"]);
        // Failed gate → ERROR status on the check span only.
        assert_eq!(check["status"]["code"], 2);
        assert_eq!(step_span["status"]["code"], 0);
    }

    #[test]
    fn ids_are_deterministic_and_steps_without_timestamps_are_counted_not_fabricated() {
        let (obj, run_steps) = cycle();
        let mut no_ts: Note = serde_json::from_str("{\"note\":9}").unwrap();
        no_ts.timestamp = None;
        let steps = [step(7, "2026-06-10T10:06:00Z", None), no_ts];
        let a = build_otel("proj", Some((&obj, &run_steps)), steps.iter(), None);
        let b = build_otel("proj", Some((&obj, &run_steps)), steps.iter(), None);
        assert_eq!(a.body, b.body, "same inputs must export identical bytes");
        assert_eq!(a.skipped_steps, 1);
        assert_ne!(
            trace_id("proj"),
            trace_id("other"),
            "trace ids are per-project"
        );
    }

    // ---- SEP-414 join (mcp-trace-context-propagation) ----
    //
    // One test per claim. A deliberate breakage that leaves any of these green
    // means that test is lying about the claim in its name.

    const CALLER_TRACE: &str = "0af7651916cd43dd8448eb211c80319c";
    const CALLER_SPAN: &str = "00f067aa0ba902b7";

    fn caller_context() -> InboundTrace {
        InboundTrace::from_meta(
            Some("00-0af7651916cd43dd8448eb211c80319c-00f067aa0ba902b7-01"),
            None,
            None,
            "2026-07-27T00:00:00Z".into(),
        )
        .expect("fixture traceparent must parse")
    }

    fn spans_of(out: &OtelExport) -> Vec<Value> {
        out.body["resourceSpans"][0]["scopeSpans"][0]["spans"]
            .as_array()
            .expect("spans")
            .clone()
    }

    /// CLAIM 1: an adopted caller context replaces OUR trace id on EVERY span.
    #[test]
    fn joined_export_carries_the_callers_trace_id_on_every_span() {
        let (obj, run_steps) = cycle();
        let ctx = caller_context();
        let out = build_otel(
            "proj",
            Some((&obj, &run_steps)),
            std::iter::empty(),
            Some(&ctx),
        );
        let spans = spans_of(&out);
        assert!(!spans.is_empty(), "fixture must produce spans");
        for s in &spans {
            assert_eq!(
                s["traceId"].as_str(),
                Some(CALLER_TRACE),
                "span {} kept a non-caller trace id",
                s["name"]
            );
        }
        assert_ne!(
            trace_id("proj"),
            CALLER_TRACE,
            "fixture is only meaningful if the caller id differs from ours"
        );
    }

    /// CLAIM 2: the root span parents to the caller's span — this is what makes
    /// the two trees ONE tree rather than two roots sharing an id.
    #[test]
    fn joined_export_parents_the_root_to_the_callers_span() {
        let (obj, run_steps) = cycle();
        let ctx = caller_context();
        let out = build_otel(
            "proj",
            Some((&obj, &run_steps)),
            std::iter::empty(),
            Some(&ctx),
        );
        let spans = spans_of(&out);
        let root = spans
            .iter()
            .find(|s| s["name"] == "workspace proj")
            .expect("root span");
        assert_eq!(root["parentSpanId"].as_str(), Some(CALLER_SPAN));
    }

    /// CLAIM 3: joining changes ONLY the trace id and the root's parent. Every
    /// span id — including the root's own — is unchanged, which is what keeps
    /// the export deterministic.
    #[test]
    fn joining_leaves_every_span_id_untouched() {
        let (obj, run_steps) = cycle();
        let ctx = caller_context();
        let plain = build_otel("proj", Some((&obj, &run_steps)), std::iter::empty(), None);
        let joined = build_otel(
            "proj",
            Some((&obj, &run_steps)),
            std::iter::empty(),
            Some(&ctx),
        );
        let ids = |o: &OtelExport| -> Vec<String> {
            spans_of(o)
                .iter()
                .map(|s| s["spanId"].as_str().unwrap_or_default().to_string())
                .collect()
        };
        assert_eq!(ids(&plain), ids(&joined));
    }

    /// CLAIM 4: with NO adopted context the export is exactly what it was
    /// before SEP-414 — our own trace id, and a root that parents to nothing.
    #[test]
    fn unjoined_export_is_unchanged_by_the_sep414_work() {
        let (obj, run_steps) = cycle();
        let out = build_otel("proj", Some((&obj, &run_steps)), std::iter::empty(), None);
        let spans = spans_of(&out);
        let root = spans
            .iter()
            .find(|s| s["name"] == "workspace proj")
            .expect("root span");
        assert!(
            root.get("parentSpanId").is_none(),
            "an unjoined root must have no parent at all, not an empty one"
        );
        for s in &spans {
            assert_eq!(s["traceId"].as_str(), Some(trace_id("proj").as_str()));
        }
    }

    /// CLAIM 5: the strict validator REJECTS a joined body. This is the island
    /// assumption made visible — without the external-parent declaration the
    /// CLI would bail on every joined export.
    #[test]
    fn strict_validator_rejects_a_joined_export() {
        let (obj, run_steps) = cycle();
        let ctx = caller_context();
        let out = build_otel(
            "proj",
            Some((&obj, &run_steps)),
            std::iter::empty(),
            Some(&ctx),
        );
        let problems = validate_otlp(&out.body);
        assert!(
            problems.iter().any(|p| p.contains("dangling parentSpanId")),
            "expected the strict validator to flag the caller's span, got {problems:?}"
        );
    }

    /// CLAIM 6: declaring the caller's span id makes the same body valid.
    #[test]
    fn declaring_the_external_parent_accepts_the_joined_export() {
        let (obj, run_steps) = cycle();
        let ctx = caller_context();
        let out = build_otel(
            "proj",
            Some((&obj, &run_steps)),
            std::iter::empty(),
            Some(&ctx),
        );
        assert_eq!(
            validate_otlp_with_external_parent(&out.body, Some(CALLER_SPAN)),
            Vec::<String>::new()
        );
    }

    /// CLAIM 7: the rule was NOT weakened. Declaring one external parent must
    /// not amnesty a genuinely dangling internal parent.
    #[test]
    fn declaring_an_external_parent_still_catches_other_dangling_parents() {
        let body = serde_json::json!({
            "resourceSpans": [{
                "resource": { "attributes": [attr("service.name", "magistr")] },
                "scopeSpans": [{ "spans": [
                    {
                        "traceId": CALLER_TRACE, "spanId": "aaaaaaaaaaaaaaaa",
                        "name": "root", "parentSpanId": CALLER_SPAN,
                        "startTimeUnixNano": "1", "endTimeUnixNano": "2"
                    },
                    {
                        "traceId": CALLER_TRACE, "spanId": "bbbbbbbbbbbbbbbb",
                        "name": "orphan", "parentSpanId": "cccccccccccccccc",
                        "startTimeUnixNano": "1", "endTimeUnixNano": "2"
                    }
                ]}]
            }]
        });
        let problems = validate_otlp_with_external_parent(&body, Some(CALLER_SPAN));
        assert_eq!(
            problems,
            vec!["span orphan: dangling parentSpanId".to_string()],
            "only the declared external id is amnestied"
        );
    }

    #[test]
    fn gen_ai_attributes_ride_the_agent_spans() {
        let (obj, run_steps) = cycle();
        let out = build_otel("proj", Some((&obj, &run_steps)), std::iter::empty(), None);
        let text = out.body.to_string();
        assert!(text.contains("gen_ai.operation.name"));
        assert!(text.contains("invoke_agent"));
        assert!(text.contains("execute_tool"));
        assert!(text.contains("gen_ai.system"));
    }

    #[test]
    fn validator_catches_dangling_parents_and_bad_ids() {
        let bad = serde_json::json!({
            "resourceSpans": [{
                "resource": { "attributes": [] },
                "scopeSpans": [{ "scope": {}, "spans": [{
                    "traceId": "xyz", "spanId": "123", "name": "",
                    "parentSpanId": "feedfacefeedface",
                    "startTimeUnixNano": "5", "endTimeUnixNano": "1"
                }]}]
            }]
        });
        let errors = validate_otlp(&bad);
        assert!(errors.iter().any(|e| e.contains("service.name")));
        assert!(errors.iter().any(|e| e.contains("bad traceId")));
        assert!(errors.iter().any(|e| e.contains("dangling")));
        assert!(errors.iter().any(|e| e.contains("bad time range")));
        assert!(errors.iter().any(|e| e.contains("empty name")));
    }
}
