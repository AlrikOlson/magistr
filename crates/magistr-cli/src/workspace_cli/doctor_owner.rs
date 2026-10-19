//! The doctor's `owner` section: what the workspace's durable ownership
//! records say and what the owner endpoint itself answers. Nothing here
//! takes the lock, opens a store or admits anything; the probe is one
//! bounded handshake, and only the owner's own answer is reported as
//! liveness.

use std::{path::Path, time::Duration};

use magistr_engine::native::{
    AttachKind, AttachRefusal, BuildIdentity, CallerIdentity, OwnerLiveness, OwnerProbe,
};
use magistr_workspace::{
    diagnostics::{DoctorDataClass, DoctorFinding, DoctorSection, DoctorSectionId},
    service::{LockRecord, ServerLock},
    snapshot::WorkspaceIdentity,
};
use serde_json::{Value, json};

/// How long the probe waits for the owner's greeting and report together.
pub(crate) const PROBE_DEADLINE: Duration = Duration::from_secs(1);

const SOURCE: &str = ".magistr/workspace/server.generation + server.lock record (read-only; the lock is never taken) + one bounded probe handshake on the owner endpoint (admits nothing, mints nothing, is not counted as a session)";
const RESTORE: &str = "restore workspace state from a known-good backup while all workspace owners are stopped; never remove a live owner\'s lock";

/// Observe ownership of the workspace at `root` whose state lives in
/// `state_dir`, with the probe bounded by `deadline`.
pub(crate) fn owner_section(root: &Path, state_dir: &Path, deadline: Duration) -> DoctorSection {
    let mut findings = Vec::new();
    let durable = durable_generation(root, &mut findings);
    let record = lock_record(&state_dir.join(ServerLock::FILE_NAME), &mut findings);
    let probe = OwnerProbe::workspace(
        root,
        CallerIdentity::this_process("workspace doctor"),
        deadline,
    );
    let endpoint = probe.endpoint.display().to_string();
    probe_findings(&probe.liveness, &endpoint, durable, &mut findings);
    let detail = json!({
        "endpoint": endpoint,
        "durableGeneration": durable,
        "lockRecord": record,
        "probe": probe_detail(&probe.liveness, deadline),
        "certifies": "only the probe's `live` and `refused` outcomes come from an owner process; the durable generation, the lock record and a socket file are records of past admissions and certify nothing about a live owner",
    });
    DoctorSection {
        id: DoctorSectionId::Owner,
        source: SOURCE.to_owned(),
        findings,
        detail,
    }
}

/// The durable counter beside the lock: the last admission, never a sign
/// of life. A malformed counter is the durable finding admission would
/// refuse on.
fn durable_generation(root: &Path, findings: &mut Vec<DoctorFinding>) -> Option<u64> {
    match WorkspaceIdentity::observe(root) {
        Ok(identity) => identity.generation,
        Err(error) => {
            findings.push(DoctorFinding::unhealthy(
                "server.generation",
                DoctorDataClass::Durable,
                format!("the durable generation cannot be read: {error}"),
                "restore the generation state offline; it must never be reset while the workspace may be owned",
            ));
            None
        }
    }
}

fn lock_record(lock: &Path, findings: &mut Vec<DoctorFinding>) -> Value {
    match ServerLock::inspect(lock) {
        Ok(LockRecord::Absent) => {
            findings.push(DoctorFinding::info(
                "server.lock",
                DoctorDataClass::Durable,
                "no lock record: no native owner has admitted this workspace yet",
            ));
            json!({ "kind": "absent" })
        }
        Ok(LockRecord::Native) => {
            findings.push(DoctorFinding::healthy(
                "server.lock",
                "native marker; ownership is the OS lock on this inode, which only the owner endpoint can attest to",
            ));
            json!({ "kind": "native" })
        }
        Ok(LockRecord::Unrecognized) => {
            findings.push(DoctorFinding::unhealthy(
                "server.lock",
                DoctorDataClass::Durable,
                "empty or unrecognized lock record; admission is refused",
                RESTORE,
            ));
            json!({ "kind": "unrecognized" })
        }
        Err(error) => {
            findings.push(DoctorFinding::unhealthy(
                "server.lock",
                DoctorDataClass::Durable,
                format!("the lock record cannot be read: {error}"),
                "repair .magistr/workspace/server.lock offline; never replace or remove a live owner's lock",
            ));
            json!({ "kind": "unreadable", "error": error.to_string() })
        }
    }
}

fn probe_findings(
    liveness: &OwnerLiveness,
    endpoint: &str,
    durable: Option<u64>,
    findings: &mut Vec<DoctorFinding>,
) {
    let last_admission = durable.map_or_else(
        || "no admission is recorded".to_owned(),
        |generation| {
            format!("the durable counter records generation {generation} as the last admission, which is not a liveness proof")
        },
    );
    match liveness {
        OwnerLiveness::Live(report) => {
            let generation = report.generation.get();
            findings.push(DoctorFinding::healthy(
                "owner",
                format!(
                    "live owner generation {generation}, pid {}, {}; {} live connection(s): {} mcp, {} commands; endpoint {endpoint}",
                    report.owner_pid,
                    report.build,
                    report.connections.len(),
                    report.attached(AttachKind::Mcp),
                    report.attached(AttachKind::Commands),
                ),
            ));
            if let Some(counter) = durable
                && counter != generation
            {
                findings.push(DoctorFinding::unhealthy(
                    "server.generation",
                    DoctorDataClass::Durable,
                    format!("the durable counter says {counter} but the live owner was admitted under {generation}"),
                    "end the owner's session and restore the generation state offline; the counter only advances under the held lock",
                ));
            }
        }
        OwnerLiveness::Refused(AttachRefusal::Build { served, offered }) => {
            findings.push(DoctorFinding::info(
                "owner",
                DoctorDataClass::NotApplicable,
                format!("Magistr is running at {endpoint} as {served}; this installation is {offered}. Detailed session information is unavailable from this build. This diagnostic does not determine whether MCP tools can connect."),
            ));
        }
        OwnerLiveness::Refused(refusal) => {
            findings.push(DoctorFinding::unhealthy(
                "owner",
                DoctorDataClass::NotApplicable,
                format!("a live owner at {endpoint} refused the probe: {refusal}"),
                "the owner is alive but not this workspace's or not this user's; check the root and the owning user",
            ));
        }
        OwnerLiveness::Silent { deadline } => {
            findings.push(DoctorFinding::unhealthy(
                "owner",
                DoctorDataClass::NotApplicable,
                format!("an endpoint at {endpoint} accepted the connection but did not greet within {} ms; liveness is not certified ({last_admission})", deadline.as_millis()),
                "retry; an owner that stays silent is hung — end its process, and the next bridge request admits the workspace",
            ));
        }
        OwnerLiveness::Absent => {
            findings.push(DoctorFinding::info(
                "owner",
                DoctorDataClass::NotApplicable,
                format!("no owner reachable: no endpoint at {endpoint}; {last_admission}"),
            ));
        }
        OwnerLiveness::Unanswered { reason } => {
            findings.push(DoctorFinding::info(
                "owner",
                DoctorDataClass::NotApplicable,
                format!("no owner reachable: an endpoint file remains at {endpoint} but nothing answered ({reason}); a stale socket certifies nothing and the next admitted owner reclaims it; {last_admission}"),
            ));
        }
    }
}

fn probe_detail(liveness: &OwnerLiveness, deadline: Duration) -> Value {
    let deadline_ms = deadline.as_millis();
    match liveness {
        OwnerLiveness::Live(report) => json!({
            "liveness": "live",
            "generation": report.generation.get(),
            "ownerPid": report.owner_pid,
            "build": build_detail(&report.build),
            "identity": report.identity,
            "connections": report.connections,
            "attached": {
                "mcp": report.attached(AttachKind::Mcp),
                "commands": report.attached(AttachKind::Commands),
                "total": report.connections.len(),
            },
            "deadlineMs": deadline_ms,
        }),
        OwnerLiveness::Refused(refusal) => json!({
            "liveness": "refused",
            "refusal": refusal,
            "deadlineMs": deadline_ms,
        }),
        OwnerLiveness::Silent { deadline } => json!({
            "liveness": "silent",
            "deadlineMs": deadline.as_millis(),
        }),
        OwnerLiveness::Absent => json!({ "liveness": "absent", "deadlineMs": deadline_ms }),
        OwnerLiveness::Unanswered { reason } => json!({
            "liveness": "unanswered",
            "reason": reason,
            "deadlineMs": deadline_ms,
        }),
    }
}

fn build_detail(build: &BuildIdentity) -> Value {
    json!({
        "package": build.package(),
        "contract": build.contract(),
        "profile": build.profile(),
        "target": build.target(),
    })
}

#[cfg(all(test, unix))]
#[path = "doctor_owner.tests.rs"]
mod tests;
