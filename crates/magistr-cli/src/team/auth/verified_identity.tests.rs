use super::*;

fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("timestamp")
}

fn identity(name: Option<&str>, email: Option<&str>) -> VerifiedIdentity {
    VerifiedIdentity {
        provider: "github".into(),
        subject: "42".into(),
        name: name.map(String::from),
        email: email.map(String::from),
        issued_at: at(0),
        expires_at: at(100),
    }
}

#[test]
fn expiry_is_inclusive_at_the_boundary() {
    let id = identity(Some("Alice"), None);
    assert!(!id.is_expired(at(99)));
    assert!(id.is_expired(at(100)));
    assert!(id.is_expired(at(101)));
}

#[test]
fn display_name_prefers_the_provider_name() {
    assert_eq!(
        identity(Some("Alice Ng"), Some("alice@example.dev")).display_name(),
        "Alice Ng"
    );
}

#[test]
fn display_name_falls_back_to_the_email_local_part() {
    assert_eq!(
        identity(None, Some("alice@example.dev")).display_name(),
        "alice"
    );
}

#[test]
fn display_name_ignores_a_blank_name() {
    assert_eq!(
        identity(Some("   "), Some("alice@example.dev")).display_name(),
        "alice"
    );
}

#[test]
fn display_name_falls_back_to_the_subject() {
    assert_eq!(identity(None, None).display_name(), "42");
}

#[test]
fn absent_name_and_email_are_omitted_from_json() {
    let json = serde_json::to_string(&identity(None, None)).expect("serialize");
    assert!(!json.contains("name"), "{json}");
    assert!(!json.contains("email"), "{json}");
    let back: VerifiedIdentity = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, identity(None, None));
}
