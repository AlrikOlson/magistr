use super::*;
use crate::{
    extensions::{PluginComponent, PluginGuestPolicy},
    worker::{WorkerGuestEnd, WorkerLanguageLane, WorkerLanguageRegistration},
};

fn pack(plugin: &str) -> AdmittedLangPack {
    AdmittedLangPack {
        plugin: plugin.into(),
        component: PluginComponent {
            abs_path: "/nowhere/guest.wasm".into(),
            sha256: format!("{plugin}-sha"),
            bytes: b"(component)".to_vec(),
        },
        capabilities: Vec::new(),
        notes: vec!["admission note".into()],
        policy: PluginGuestPolicy::ceiling(),
        blessed: false,
    }
}

fn loaded(id: &str, extensions: &[&str]) -> Result<WorkerResponse, WorkerError> {
    Ok(WorkerResponse::LanguagePackLoaded {
        end: WorkerGuestEnd::Ok,
        fuel_used: 1,
        registration: Some(WorkerLanguageRegistration {
            id: id.into(),
            extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
            lane: WorkerLanguageLane::Wasm,
            queries_ok: true,
            notes: vec!["lane note".into()],
        }),
    })
}

struct Rig {
    dir: tempfile::TempDir,
    languages: SourceLanguages,
    restore: LangPackRestore,
}

impl Rig {
    fn new(packs: Vec<AdmittedLangPack>, refused: Vec<LangPackStatus>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let languages = SourceLanguages::bundled();
        let restore = LangPackRestore::new(
            LangPackAdmission { packs, refused },
            dir.path(),
            languages.clone(),
        )
        .with_clock(|| "2026-09-17T00:00:00Z".to_owned());
        Self {
            dir,
            languages,
            restore,
        }
    }

    fn breaches(&self) -> Vec<PluginHealthRecord> {
        PluginHealth::new(self.dir.path()).read()
    }
}

use super::super::PluginHealthRecord;

#[test]
fn healthy_rows_are_retained_and_their_claims_select_languages_until_a_later_incarnation_loses_them()
 {
    let gleam = pack("magistr.lang-pack-gleam");
    let refused =
        LangPackStatus::refused("magistr.lang-pack-broken", "missing-component", "x", vec![]);
    let rig = Rig::new(vec![gleam.clone()], vec![refused.clone()]);
    assert_eq!(
        rig.restore.statuses(),
        vec![refused.clone()],
        "refusals before any incarnation"
    );
    assert!(!rig.restore.is_empty());

    let mut rows = vec![refused.clone()];
    rig.restore
        .absorb(&gleam, loaded("gleam", &["gleam"]), &mut rows)
        .expect("a registered pack keeps the incarnation");
    rig.restore.publish(rows);
    let statuses = rig.restore.statuses();
    assert_eq!(statuses.len(), 2);
    assert!(statuses[1].ok);
    assert_eq!(statuses[1].notes, vec!["admission note", "lane note"]);
    assert_eq!(
        rig.languages.language("src/main.gleam").as_deref(),
        Some("gleam")
    );
    assert_eq!(
        rig.restore.status_notes()[1],
        "lang pack magistr.lang-pack-gleam: gleam registered on the wasm lane (extensions: gleam)"
    );

    // The next incarnation refuses the registration: the row flips, the
    // claim is unselected and the breaker records the refusal.
    let mut rows = vec![refused];
    rig.restore
        .absorb(
            &gleam,
            Err(WorkerError::Remote {
                code: "LANGUAGE_UNAVAILABLE".into(),
                message: "shadows a bundled grammar".into(),
            }),
            &mut rows,
        )
        .expect("a refused registration is a row, not a lost worker");
    rig.restore.publish(rows);
    let statuses = rig.restore.statuses();
    assert!(!statuses[1].ok);
    assert_eq!(statuses[1].outcome, "registration-refused");
    assert!(statuses[1].detail.contains("shadows a bundled grammar"));
    assert_eq!(rig.languages.language("src/main.gleam"), None);
    let breaches = rig.breaches();
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].component_hash, "magistr.lang-pack-gleam-sha");
    assert_eq!(breaches[0].at, "2026-09-17T00:00:00Z");
    assert!(
        rig.restore.status_notes()[1].starts_with(
            "lang pack magistr.lang-pack-gleam: not registered (registration-refused:"
        )
    );
}

#[test]
fn a_guest_that_does_not_register_breaches_and_a_mismatched_answer_is_a_refusal() {
    let gleam = pack("magistr.lang-pack-gleam");
    let rig = Rig::new(vec![gleam.clone()], Vec::new());
    let mut rows = Vec::new();
    rig.restore
        .absorb(
            &gleam,
            Ok(WorkerResponse::LanguagePackLoaded {
                end: WorkerGuestEnd::GuestTrap("boom".into()),
                fuel_used: 3,
                registration: None,
            }),
            &mut rows,
        )
        .unwrap();
    rig.restore
        .absorb(&gleam, Ok(WorkerResponse::Pong), &mut rows)
        .unwrap();
    assert_eq!(rows[0].outcome, "guest-trap");
    assert_eq!(rows[0].detail, "boom");
    assert_eq!(rows[1].outcome, "registration-refused");
    assert!(rows[1].detail.contains("mismatched"));
    assert_eq!(rig.breaches().len(), 2);
}

#[test]
fn a_lost_worker_fails_the_restore_explicitly_and_three_strikes_breach_the_pack() {
    let gleam = pack("magistr.lang-pack-gleam");
    let rig = Rig::new(vec![gleam.clone()], Vec::new());
    for strike in 1..=3 {
        let mut rows = Vec::new();
        let error = rig
            .restore
            .absorb(
                &gleam,
                Err(WorkerError::Unavailable("broken pipe".into())),
                &mut rows,
            )
            .expect_err("a lost worker never admits the incarnation");
        assert_eq!(
            error,
            WorkerError::Unavailable(
                "lang pack magistr.lang-pack-gleam could not be restored into the new worker: worker unavailable: broken pipe".into()
            )
        );
        assert!(rows.is_empty());
        assert_eq!(
            rig.breaches().len(),
            usize::from(strike == 3),
            "strike {strike}"
        );
    }
    assert!(rig.breaches()[0].reason.contains("worker lost 3 times"));
    // A registration that answers clears the strikes.
    let other = pack("magistr.lang-pack-other");
    let mut rows = Vec::new();
    rig.restore
        .absorb(
            &other,
            Err(WorkerError::Protocol("garbled".into())),
            &mut rows,
        )
        .unwrap_err();
    rig.restore
        .absorb(&other, loaded("other", &["oth"]), &mut rows)
        .unwrap();
    rig.restore
        .absorb(
            &other,
            Err(WorkerError::Protocol("garbled".into())),
            &mut rows,
        )
        .unwrap_err();
    rig.restore
        .absorb(
            &other,
            Err(WorkerError::Protocol("garbled".into())),
            &mut rows,
        )
        .unwrap_err();
    assert_eq!(
        rig.breaches().len(),
        1,
        "two strikes after a clear are not a breach"
    );
}
