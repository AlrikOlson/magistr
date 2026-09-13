use super::*;
use alloc::string::ToString;

#[test]
fn a_missing_owner_is_an_unavailable_capability_and_no_authority_is_denied() {
    assert!(matches!(
        SourceRefusal::OwnerElsewhere.effect_error(),
        EffectError::Unavailable(reason) if reason.contains("another owner holds the workspace")
    ));
    assert!(matches!(
        SourceRefusal::NotAdmitted("lock needs migration".into()).effect_error(),
        EffectError::Unavailable(reason) if reason.contains("lock needs migration")
    ));
    assert!(matches!(
        SourceRefusal::NoAuthority.effect_error(),
        EffectError::Denied(reason) if reason.contains("no source-write authority")
    ));
}

#[test]
fn every_refusal_has_a_stable_wire_word_and_says_it_refused_the_write() {
    let refusals = [
        SourceRefusal::OwnerElsewhere,
        SourceRefusal::NotAdmitted("x".into()),
        SourceRefusal::NoAuthority,
    ];
    let kinds: alloc::vec::Vec<&str> = refusals.iter().map(SourceRefusal::kind).collect();
    assert_eq!(kinds, ["owner_elsewhere", "not_admitted", "no_authority"]);
    for refusal in &refusals {
        assert!(refusal.to_string().starts_with("source write refused: "));
    }
}
