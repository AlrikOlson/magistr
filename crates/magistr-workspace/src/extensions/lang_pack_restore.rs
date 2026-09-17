use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, PoisonError},
    time::Duration,
};

use super::{
    AdmittedLangPack, HostClock, LangPackAdmission, LangPackLoadOutcome, LangPackStatus,
    PluginHealth,
};
use crate::{
    source_languages::SourceLanguages,
    worker::{
        RestoreFuture, WorkerError, WorkerIncarnation, WorkerRequest, WorkerResponse, WorkerRestore,
    },
};

/// Lost-worker incidents while loading the same pack before its breaker
/// trips: one incident may be a foreign kill, three in a row is the pack.
const RESTORE_STRIKES: u32 = 3;

/// The owner's retained language configuration and its replay: every worker
/// incarnation registers the admitted packs before the supervisor dispatches
/// any request to it, breaker-aware (a breached pack is skipped, a pack that
/// fails to register is breached) and idempotent (the registry replaces by
/// language id and the retained rows are recomputed whole). The healthy
/// rows' claims are what host-side language selection consults, so a
/// registration a later incarnation loses is unselected at once. A worker
/// lost while a pack loads fails the restore explicitly — the process is
/// never admitted — and strikes that pack; three strikes breach it so a pack
/// that kills the worker on load cannot keep every incarnation from serving.
pub struct LangPackRestore {
    packs: Vec<AdmittedLangPack>,
    refused: Vec<LangPackStatus>,
    health: PluginHealth,
    languages: SourceLanguages,
    timeout: Duration,
    clock: Box<dyn Fn() -> String + Send + Sync>,
    retained: Mutex<Vec<LangPackStatus>>,
    /// Why the last incarnation was not admitted; cleared by a restore that
    /// completes.
    last_failure: Mutex<Option<String>>,
    strikes: Mutex<HashMap<String, u32>>,
}

impl LangPackRestore {
    /// `state_dir` holds the breaker; `languages` is the owner's selection
    /// handle every consumer shares.
    #[must_use]
    pub fn new(admission: LangPackAdmission, state_dir: &Path, languages: SourceLanguages) -> Self {
        Self {
            retained: Mutex::new(admission.refused.clone()),
            packs: admission.packs,
            refused: admission.refused,
            health: PluginHealth::new(state_dir),
            languages,
            timeout: Duration::from_secs(60),
            clock: Box::new(HostClock::now),
            last_failure: Mutex::new(None),
            strikes: Mutex::new(HashMap::new()),
        }
    }

    /// Inject the wall-clock used for breach records (tests pin it).
    #[must_use]
    pub fn with_clock(mut self, clock: impl Fn() -> String + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// True when nothing was admitted: no incarnation needs restoring.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }

    /// The admitted packs, install order.
    #[must_use]
    pub fn packs(&self) -> &[AdmittedLangPack] {
        &self.packs
    }

    /// The last incarnation's rows (admission refusals first); before any
    /// incarnation, the refusals alone.
    #[must_use]
    pub fn statuses(&self) -> Vec<LangPackStatus> {
        self.retained
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Why the last incarnation could not be restored, until one is.
    #[must_use]
    pub fn last_failure(&self) -> Option<String> {
        self.last_failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Status rows as schema-free notes, plus the last restore failure.
    #[must_use]
    pub fn status_notes(&self) -> Vec<String> {
        let mut notes: Vec<String> = self.statuses().iter().map(LangPackStatus::note).collect();
        if let Some(failure) = self.last_failure() {
            notes.push(format!(
                "lang packs: the last worker incarnation was not admitted ({failure}); the next request restores again"
            ));
        }
        notes
    }

    /// Fold one pack's answer into this incarnation's rows. `Ok(())` means
    /// the incarnation goes on (registered, refused or breakered are all
    /// rows); `Err` means the worker was lost and the incarnation must not
    /// be admitted.
    pub(super) fn absorb(
        &self,
        pack: &AdmittedLangPack,
        answer: Result<WorkerResponse, WorkerError>,
        rows: &mut Vec<LangPackStatus>,
    ) -> Result<(), WorkerError> {
        let now = (self.clock)();
        let outcome = match answer {
            Ok(WorkerResponse::LanguagePackLoaded {
                end,
                fuel_used,
                registration,
            }) => Ok(LangPackLoadOutcome {
                end,
                fuel_used,
                registration,
            }),
            Ok(_) => Err("worker returned a mismatched extension response".to_owned()),
            Err(WorkerError::Remote { code, message }) => Err(format!("{code}: {message}")),
            Err(lost) => {
                let mut strikes = self.strikes.lock().unwrap_or_else(PoisonError::into_inner);
                let count = strikes.entry(pack.plugin.clone()).or_insert(0);
                *count = count.saturating_add(1);
                if *count >= RESTORE_STRIKES {
                    let _ = self.health.record(
                        &pack.plugin,
                        &pack.component.sha256,
                        &format!("worker lost {count} times while loading the pack: {lost}"),
                        &now,
                    );
                }
                return Err(WorkerError::Unavailable(format!(
                    "lang pack {} could not be restored into the new worker: {lost}",
                    pack.plugin
                )));
            }
        };
        self.strikes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&pack.plugin);
        rows.push(LangPackStatus::registered(
            pack,
            outcome,
            &self.health,
            &now,
        ));
        Ok(())
    }

    /// Publish one incarnation's rows: the retained set and the selection
    /// table both become exactly what this incarnation registered.
    pub(super) fn publish(&self, rows: Vec<LangPackStatus>) {
        self.languages
            .retain(rows.iter().filter_map(LangPackStatus::claim).collect());
        *self.retained.lock().unwrap_or_else(PoisonError::into_inner) = rows;
        *self
            .last_failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }

    fn note_failure(&self, error: &WorkerError) {
        *self
            .last_failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(error.to_string());
    }
}

impl WorkerRestore for LangPackRestore {
    fn restore<'a>(&'a self, incarnation: &'a mut WorkerIncarnation<'_>) -> RestoreFuture<'a> {
        Box::pin(async move {
            let mut rows = self.refused.clone();
            for pack in &self.packs {
                if let Some(breach) = self.health.breach(&pack.plugin, &pack.component.sha256) {
                    rows.push(LangPackStatus::breakered(pack, &breach));
                    continue;
                }
                let answer = incarnation
                    .call(
                        WorkerRequest::LoadLanguagePack {
                            guest: pack.component.bytes.clone(),
                            capabilities: pack.capabilities.clone(),
                            policy: pack.policy.into(),
                            blessed: pack.blessed,
                        },
                        self.timeout,
                    )
                    .await;
                if let Err(error) = self.absorb(pack, answer, &mut rows) {
                    self.note_failure(&error);
                    return Err(error);
                }
            }
            self.publish(rows);
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "lang_pack_restore.tests.rs"]
mod tests;
