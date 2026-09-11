use std::path::Path;

use serde_json::json;

use super::{
    TckChapter, TckChapterId, TckFuzz, TckProbe, TckReport,
    fixtures::{GROW_WAT, SPIN_WAT},
};
use crate::extensions::{
    ExtensionLane, PluginComponent, PluginGuestPolicy, PluginInvokeOutcome, PluginManifest,
};

/// Manifest tool name that marks a plugin as a language pack.
const LANG_PACK_TOOL: &str = "classify-file";
/// Starvation fuel for the resource chapter's plugin-side probe.
const STARVATION_FUEL: u64 = 10;
/// Tiny memory cap for the resource chapter's plugin-side probe.
const TINY_MEMORY_CAP: usize = 32 * 1024;

/// What the kit knows once the interface chapter admitted the plugin.
struct Subject {
    manifest: PluginManifest,
    component: PluginComponent,
    policy: PluginGuestPolicy,
}

/// Run the full conformance kit against one plugin directory. Never fails
/// for a non-conforming plugin — every failure is a failed probe in the
/// report; a dead lane is a failed probe too.
pub async fn run_plugin_tck(
    plugin_dir: &Path,
    lane: &dyn ExtensionLane,
    fuzz: TckFuzz,
) -> TckReport {
    let mut notes = Vec::new();
    let (interface, subject) = interface_chapter(plugin_dir, lane, &mut notes).await;
    let applicable = subject.is_some();
    let capabilities = match &subject {
        Some(subject) => capabilities_chapter(subject, lane).await,
        None => Vec::new(),
    };
    let resources = match &subject {
        Some(subject) => resources_chapter(subject, lane).await,
        None => Vec::new(),
    };
    let lang_pack = subject.as_ref().filter(|subject| {
        subject
            .manifest
            .tools
            .iter()
            .any(|tool| tool.name == LANG_PACK_TOOL)
    });
    let fuzz_probes = match lang_pack {
        Some(subject) => fuzz_chapter(subject, lane, fuzz).await,
        None => Vec::new(),
    };
    let chapters = vec![
        TckChapter::new(TckChapterId::Interface, true, interface),
        TckChapter::new(TckChapterId::Capabilities, applicable, capabilities),
        TckChapter::new(TckChapterId::Resources, applicable, resources),
        TckChapter::new(TckChapterId::LangPackFuzz, lang_pack.is_some(), fuzz_probes),
    ];
    TckReport {
        plugin_dir: plugin_dir.display().to_string(),
        plugin: subject.as_ref().map(|subject| subject.manifest.label()),
        passed: chapters.iter().all(|chapter| chapter.passed),
        chapters,
        notes,
    }
}

fn payload(tool: &str, args: serde_json::Value) -> String {
    json!({"v": 1, "tool": tool, "args": args}).to_string()
}

async fn invoke(
    lane: &dyn ExtensionLane,
    guest: &[u8],
    export: &str,
    payload: String,
    capabilities: &[String],
    policy: PluginGuestPolicy,
) -> Result<PluginInvokeOutcome, String> {
    lane.invoke_tool(
        guest.to_vec(),
        export.to_owned(),
        payload,
        capabilities.to_vec(),
        policy,
    )
    .await
    .map_err(|error| error.message)
}

fn is_json(text: Option<&str>) -> bool {
    text.is_some_and(|text| serde_json::from_str::<serde_json::Value>(text).is_ok())
}

async fn interface_chapter(
    plugin_dir: &Path,
    lane: &dyn ExtensionLane,
    notes: &mut Vec<String>,
) -> (Vec<TckProbe>, Option<Subject>) {
    let mut probes = Vec::new();
    let manifest = match std::fs::read_to_string(plugin_dir.join("plugin.toml")) {
        Ok(source) => match PluginManifest::parse(&source) {
            Ok(parsed) => {
                notes.extend(parsed.warnings);
                probes.push(TckProbe::new(
                    "manifest-valid",
                    true,
                    format!(
                        "plugin {}@{}, {} tool(s)",
                        parsed.manifest.label(),
                        parsed.manifest.version,
                        parsed.manifest.tools.len()
                    ),
                ));
                Some(parsed.manifest)
            }
            Err(errors) => {
                probes.push(TckProbe::new("manifest-valid", false, errors.join("; ")));
                None
            }
        },
        Err(error) => {
            probes.push(TckProbe::new(
                "manifest-readable",
                false,
                format!("plugin.toml unreadable: {error}"),
            ));
            None
        }
    };
    let Some(manifest) = manifest else {
        return (probes, None);
    };
    let candidate = plugin_dir.join(&manifest.component_path);
    let component = match PluginComponent::read(&candidate) {
        Some(component) => {
            probes.push(TckProbe::new(
                "component-present",
                true,
                format!(
                    "{} ({} bytes)",
                    manifest.component_path,
                    component.bytes.len()
                ),
            ));
            component
        }
        None => {
            probes.push(TckProbe::new(
                "component-present",
                false,
                format!(
                    "{} is missing, not a regular file or oversized",
                    manifest.component_path
                ),
            ));
            return (probes, None);
        }
    };
    let subject = Subject {
        policy: PluginGuestPolicy::for_manifest(&manifest),
        manifest,
        component,
    };
    for tool in &subject.manifest.tools {
        let id = format!("tool-{}-invokes", tool.name);
        let run = invoke(
            lane,
            &subject.component.bytes,
            &subject.manifest.invoke_export,
            payload(&tool.name, json!({})),
            &subject.manifest.imports,
            subject.policy,
        )
        .await;
        probes.push(match run {
            Err(error) => TckProbe::new(id, false, format!("invoke lane failed: {error}")),
            Ok(outcome) if outcome.kind() != "ok" => {
                TckProbe::new(id, false, format!("expected ok, got {}", outcome.reason()))
            }
            Ok(outcome) if is_json(outcome.result.as_deref()) => {
                TckProbe::new(id, true, format!("ok, {} fuel", outcome.fuel_used))
            }
            Ok(_) => TckProbe::new(
                id,
                false,
                "result is not JSON — a plugin tool's result must be JSON text (wit/tool.wit)",
            ),
        });
    }
    (probes, Some(subject))
}

/// Declared-sufficiency: under grants = declared imports the run is never
/// cap-denied. Least-privilege: under zero grants a plugin declaring imports
/// must be cap-denied before instantiation.
async fn capabilities_chapter(subject: &Subject, lane: &dyn ExtensionLane) -> Vec<TckProbe> {
    let declared = &subject.manifest.imports;
    let first_tool = subject
        .manifest
        .tools
        .first()
        .map_or("probe", |tool| tool.name.as_str());
    let body = payload(first_tool, json!({}));
    let mut probes = Vec::new();
    let sufficient = invoke(
        lane,
        &subject.component.bytes,
        &subject.manifest.invoke_export,
        body.clone(),
        declared,
        subject.policy,
    )
    .await;
    probes.push(match &sufficient {
        Err(error) => TckProbe::new(
            "declared-imports-suffice",
            false,
            format!("invoke lane failed: {error}"),
        ),
        Ok(outcome) if outcome.kind() == "cap-denied" => TckProbe::new(
            "declared-imports-suffice",
            false,
            format!(
                "cap-denied under the full declared grant set (needs {:?}) — the component requires an import the manifest does not declare; no grant can fix this",
                outcome.detail()
            ),
        ),
        Ok(outcome) => TckProbe::new(
            "declared-imports-suffice",
            true,
            format!(
                "outcome {} under grants=[{}]",
                outcome.kind(),
                declared.join(", ")
            ),
        ),
    });
    let zero = invoke(
        lane,
        &subject.component.bytes,
        &subject.manifest.invoke_export,
        body,
        &[],
        subject.policy,
    )
    .await;
    probes.push(match zero {
        Err(error) => TckProbe::new("least-privilege", false, format!("invoke lane failed: {error}")),
        Ok(outcome) if declared.is_empty() => TckProbe::new(
            "least-privilege",
            outcome.kind() != "cap-denied",
            if outcome.kind() == "cap-denied" {
                format!(
                    "component imports {:?} but the manifest declares no imports",
                    outcome.detail()
                )
            } else {
                format!(
                    "import-free plugin runs under zero grants (outcome {})",
                    outcome.kind()
                )
            },
        ),
        Ok(outcome) => TckProbe::new(
            "least-privilege",
            outcome.kind() == "cap-denied",
            if outcome.kind() == "cap-denied" {
                format!(
                    "zero grants denied before instantiation ({:?})",
                    outcome.detail()
                )
            } else {
                format!(
                    "declares imports [{}] yet ran with zero grants (outcome {}) — it declares capabilities it does not use",
                    declared.join(", "),
                    outcome.kind()
                )
            },
        ),
    });
    probes
}

/// Record one resource probe: the lane must answer, and the outcome must be
/// one of `allowed` — "died typed" is the whole assertion.
fn died_typed(id: String, run: Result<PluginInvokeOutcome, String>, allowed: &[&str]) -> TckProbe {
    match run {
        Err(error) => TckProbe::new(id, false, format!("invoke lane failed: {error}")),
        Ok(outcome) if allowed.contains(&outcome.kind()) => {
            TckProbe::new(id, true, format!("typed outcome {}", outcome.kind()))
        }
        Ok(outcome) => TckProbe::new(
            id,
            false,
            format!(
                "expected one of [{}], got {}",
                allowed.join(", "),
                outcome.reason()
            ),
        ),
    }
}

const TYPED_OUTCOMES: [&str; 7] = [
    "ok",
    "out-of-fuel",
    "memory-cap",
    "cap-denied",
    "guest-trap",
    "guest-exit",
    "invalid-guest",
];

async fn resources_chapter(subject: &Subject, lane: &dyn ExtensionLane) -> Vec<TckProbe> {
    let mut probes = Vec::new();
    for tool in &subject.manifest.tools {
        let starved = invoke(
            lane,
            &subject.component.bytes,
            &subject.manifest.invoke_export,
            payload(&tool.name, json!({})),
            &subject.manifest.imports,
            PluginGuestPolicy {
                fuel: STARVATION_FUEL,
                memory_cap_bytes: subject.policy.memory_cap_bytes,
            },
        )
        .await;
        probes.push(died_typed(
            format!("tool-{}-starvation-fuel", tool.name),
            starved,
            &["ok", "out-of-fuel"],
        ));
        let squeezed = invoke(
            lane,
            &subject.component.bytes,
            &subject.manifest.invoke_export,
            payload(&tool.name, json!({})),
            &subject.manifest.imports,
            PluginGuestPolicy {
                fuel: subject.policy.fuel,
                memory_cap_bytes: TINY_MEMORY_CAP,
            },
        )
        .await;
        probes.push(died_typed(
            format!("tool-{}-tiny-memory", tool.name),
            squeezed,
            &TYPED_OUTCOMES,
        ));
    }
    // Environment probes: the kit's own abusive guests must die typed.
    let spun = invoke(
        lane,
        SPIN_WAT.as_bytes(),
        "invoke",
        payload("spin", json!({})),
        &[],
        PluginGuestPolicy {
            fuel: 100_000,
            memory_cap_bytes: PluginGuestPolicy::MEMORY_CAP,
        },
    )
    .await;
    probes.push(died_typed(
        "environment-fuel-kill".into(),
        spun,
        &["out-of-fuel"],
    ));
    // Bounded fuel so a limiter that fails to trap still ends the probe
    // quickly — as out-of-fuel, which the assertion rightly rejects.
    let grown = invoke(
        lane,
        GROW_WAT.as_bytes(),
        "invoke",
        payload("grow", json!({})),
        &[],
        PluginGuestPolicy {
            fuel: 10_000_000,
            memory_cap_bytes: 4 * 64 * 1024,
        },
    )
    .await;
    probes.push(died_typed(
        "environment-memory-kill".into(),
        grown,
        &["memory-cap"],
    ));
    probes
}

async fn fuzz_chapter(subject: &Subject, lane: &dyn ExtensionLane, fuzz: TckFuzz) -> Vec<TckProbe> {
    let corpus = fuzz.corpus();
    let mut failures = 0;
    let mut first_failure = String::new();
    for (index, input) in corpus.iter().enumerate() {
        let run = invoke(
            lane,
            &subject.component.bytes,
            &subject.manifest.invoke_export,
            payload(LANG_PACK_TOOL, json!({"path": input})),
            &subject.manifest.imports,
            subject.policy,
        )
        .await;
        let verdict = match &run {
            Err(error) => Some(error.clone()),
            Ok(outcome) if outcome.kind() != "ok" => Some(format!("outcome {}", outcome.kind())),
            Ok(outcome) if !is_json(outcome.result.as_deref()) => Some("result is not JSON".into()),
            Ok(_) => None,
        };
        if let Some(why) = verdict {
            failures += 1;
            if first_failure.is_empty() {
                let shown: String = input.chars().take(40).collect();
                let ellipsis = if input.chars().count() > 40 {
                    "…"
                } else {
                    ""
                };
                first_failure = format!("corpus[{index}] ({shown:?}{ellipsis}): {why}");
            }
        }
    }
    vec![TckProbe::new(
        "fuzz-corpus",
        failures == 0,
        if failures == 0 {
            format!(
                "{} corpus entries (seed {}), all ok with JSON results",
                corpus.len(),
                fuzz.seed
            )
        } else {
            format!(
                "{failures}/{} corpus entries failed; first: {first_failure}",
                corpus.len()
            )
        },
    )]
}

#[cfg(test)]
#[path = "run.tests.rs"]
mod tests;
