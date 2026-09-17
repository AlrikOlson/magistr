use serde::Serialize;

use super::{AdmittedLangPack, LangPackLoadOutcome, PluginHealth, PluginHealthRecord};
use crate::worker::{WorkerGuestEnd, WorkerLanguageLane};

/// One pack's boot verdict — the host-side half of the lane's observability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LangPackStatus {
    /// Wire name, e.g. "workbench.lang-pack-gleam".
    pub plugin: String,
    pub ok: bool,
    /// Sandbox outcome kind or a boot-stage refusal ("missing-component",
    /// "manifest-unreadable", "breakered", "registration-refused").
    pub outcome: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    /// "native" (blessed promotion) | "wasm" — the lane the registry chose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queries_ok: Option<bool>,
    /// Every shadowed claim or degradation, verbatim.
    pub notes: Vec<String>,
}

impl LangPackStatus {
    /// A refusal decided by the host: no language was registered.
    #[must_use]
    pub fn refused(
        plugin: impl Into<String>,
        outcome: &str,
        detail: impl Into<String>,
        notes: Vec<String>,
    ) -> Self {
        Self {
            plugin: plugin.into(),
            ok: false,
            outcome: outcome.into(),
            detail: detail.into(),
            language: None,
            extensions: None,
            lane: None,
            queries_ok: None,
            notes,
        }
    }

    /// The pack is breakered under exactly these component bytes.
    #[must_use]
    pub fn breakered(pack: &AdmittedLangPack, breach: &PluginHealthRecord) -> Self {
        Self::refused(
            &pack.plugin,
            "breakered",
            format!(
                "unhealthy since {} ({}); a changed component clears the breaker",
                breach.at, breach.reason
            ),
            pack.notes.clone(),
        )
    }

    /// The worker's answer to one registration, as a status row. A lane
    /// error or a guest that did not register breaches the breaker under
    /// the pack's component hash (recorded at `now`); a failed breach record
    /// is itself a note.
    #[must_use]
    pub fn registered(
        pack: &AdmittedLangPack,
        outcome: Result<LangPackLoadOutcome, String>,
        health: &PluginHealth,
        now: &str,
    ) -> Self {
        let mut notes = pack.notes.clone();
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(message) => {
                notes.extend(health.record(&pack.plugin, &pack.component.sha256, &message, now));
                return Self::refused(&pack.plugin, "registration-refused", message, notes);
            }
        };
        let (Some(registration), WorkerGuestEnd::Ok) = (outcome.registration, &outcome.end) else {
            let reason = super::PluginInvokeOutcome::ended(outcome.end, outcome.fuel_used);
            let detail = if reason.detail().is_empty() {
                reason.kind().to_owned()
            } else {
                reason.detail()
            };
            notes.extend(health.record(&pack.plugin, &pack.component.sha256, &detail, now));
            return Self::refused(&pack.plugin, reason.kind(), detail, notes);
        };
        notes.extend(registration.notes);
        Self {
            plugin: pack.plugin.clone(),
            ok: true,
            outcome: "ok".into(),
            detail: String::new(),
            language: Some(registration.id),
            extensions: Some(registration.extensions),
            lane: Some(
                match registration.lane {
                    WorkerLanguageLane::Native => "native",
                    WorkerLanguageLane::Wasm => "wasm",
                }
                .into(),
            ),
            queries_ok: Some(registration.queries_ok),
            notes,
        }
    }

    /// The claim a healthy row contributes to host-side language selection.
    #[must_use]
    pub fn claim(&self) -> Option<(String, Vec<String>)> {
        match (&self.language, &self.extensions) {
            (Some(language), Some(extensions)) if self.ok => {
                Some((language.clone(), extensions.clone()))
            }
            _ => None,
        }
    }

    /// One schema-free line for a status envelope's notes.
    #[must_use]
    pub fn note(&self) -> String {
        if self.ok {
            format!(
                "lang pack {}: {} registered on the {} lane (extensions: {})",
                self.plugin,
                self.language.as_deref().unwrap_or_default(),
                self.lane.as_deref().unwrap_or_default(),
                self.extensions.as_deref().unwrap_or_default().join(", ")
            )
        } else {
            format!(
                "lang pack {}: not registered ({}: {})",
                self.plugin, self.outcome, self.detail
            )
        }
    }
}
