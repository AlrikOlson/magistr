use super::*;

use crate::team::TeamRole;
use crate::team::directory::{DirectoryOutcome, GroupRole};

const IDP: &str = "https://idp.example.com/metadata";
const SP: &str = "https://magistr.example.dev/sp";

fn config() -> SamlConfig {
    let mut config = SamlConfig::new(SP, IDP);
    config.email_attribute = Some("email".to_string());
    config.name_attribute = Some("displayName".to_string());
    config
}

fn now() -> DateTime<Utc> {
    "2026-08-30T12:00:00Z"
        .parse::<DateTime<Utc>>()
        .expect("fixture instant")
}

/// A realistic Response envelope: the IdP's own Issuer at Response
/// level, then the assertion.
fn response(window: &str, extra: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<samlp:Response xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
                xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion"
                Destination="{SP}/acs">
  <saml:Issuer>https://someone-else.example.com</saml:Issuer>
  <saml:Assertion ID="_a1b2c3" IssueInstant="2026-08-30T11:59:00Z">
    <saml:Issuer>{IDP}</saml:Issuer>
    <saml:Subject>
      <saml:NameID Format="urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress">ada@example.dev</saml:NameID>
    </saml:Subject>
    <saml:Conditions {window}>
      <saml:AudienceRestriction>
        <saml:Audience>{SP}</saml:Audience>
      </saml:AudienceRestriction>
    </saml:Conditions>
    <saml:AttributeStatement>
      <saml:Attribute Name="email">
        <saml:AttributeValue>ada@example.dev</saml:AttributeValue>
      </saml:Attribute>
      <saml:Attribute Name="displayName">
        <saml:AttributeValue>Ada Lovelace</saml:AttributeValue>
      </saml:Attribute>
      <saml:Attribute Name="groups">
        <saml:AttributeValue>CN=eng-platform,OU=Groups,DC=example,DC=com</saml:AttributeValue>
        <saml:AttributeValue>everyone</saml:AttributeValue>
      </saml:Attribute>
      {extra}
    </saml:AttributeStatement>
  </saml:Assertion>
</samlp:Response>"#
    )
}

fn valid_window() -> &'static str {
    r#"NotBefore="2026-08-30T11:55:00Z" NotOnOrAfter="2026-08-30T12:05:00Z""#
}

fn valid() -> String {
    response(valid_window(), "")
}

fn parsed() -> SamlAssertion {
    SamlAssertion::parse(&valid()).expect("the fixture is a well-formed assertion")
}

// ── Parsing ────────────────────────────────────────────────────────────

#[test]
fn the_assertions_own_fields_are_read() {
    let assertion = parsed();
    assert_eq!(assertion.id, "_a1b2c3");
    assert_eq!(assertion.subject, "ada@example.dev");
    assert_eq!(assertion.audiences, vec![SP.to_string()]);
}

/// A Response carries an Issuer of its own; taking that one would be
/// checking the envelope rather than the assertion.
#[test]
fn the_issuer_read_is_the_assertions_and_never_the_responses() {
    assert_eq!(parsed().issuer, IDP);
}

#[test]
fn the_validity_window_is_read_off_the_conditions() {
    let assertion = parsed();
    assert_eq!(
        assertion.not_before,
        Some("2026-08-30T11:55:00Z".parse().expect("instant"))
    );
    assert_eq!(
        assertion.not_on_or_after,
        Some("2026-08-30T12:05:00Z".parse().expect("instant"))
    );
}

#[test]
fn an_attribute_with_several_values_keeps_all_of_them() {
    assert_eq!(
        parsed().attribute("groups"),
        [
            "CN=eng-platform,OU=Groups,DC=example,DC=com".to_string(),
            "everyone".to_string()
        ]
    );
}

/// A self-closing `<Conditions/>` is what an IdP sends when there is no
/// audience restriction; its attributes still have to be read.
#[test]
fn a_self_closing_conditions_element_still_yields_its_window() {
    let xml = format!(
        r#"<saml:Assertion xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="_x">
             <saml:Issuer>{IDP}</saml:Issuer>
             <saml:Conditions NotOnOrAfter="2026-08-30T12:05:00Z"/>
           </saml:Assertion>"#
    );
    let assertion = SamlAssertion::parse(&xml).expect("parse");
    assert!(assertion.not_on_or_after.is_some());
    assert!(assertion.audiences.is_empty());
}

/// quick-xml 0.41 delivers an entity as its own event, so a value
/// carrying one arrives in three pieces; reading only the first would
/// truncate the group name at the ampersand.
#[test]
fn a_value_carrying_an_entity_is_reassembled_whole() {
    let extra = r#"<saml:Attribute Name="dept">
        <saml:AttributeValue>Ops &amp; Platform</saml:AttributeValue>
      </saml:Attribute>"#;
    let assertion = SamlAssertion::parse(&response(valid_window(), extra)).expect("parse");
    assert_eq!(assertion.attribute("dept"), ["Ops & Platform".to_string()]);
}

#[test]
fn a_numeric_character_reference_resolves() {
    let extra = r#"<saml:Attribute Name="dept">
        <saml:AttributeValue>A&#66;C</saml:AttributeValue>
      </saml:Attribute>"#;
    let assertion = SamlAssertion::parse(&response(valid_window(), extra)).expect("parse");
    assert_eq!(assertion.attribute("dept"), ["ABC".to_string()]);
}

// ── The structural half of the signature defence ───────────────────────

/// XML Signature Wrapping leaves the signed assertion in place and adds
/// an unsigned one for the data reader to pick up. Two assertions is
/// therefore refused outright rather than disambiguated.
#[test]
fn a_document_carrying_two_assertions_is_refused() {
    let doubled = valid().replace(
        "</samlp:Response>",
        &format!(
            r#"<saml:Assertion xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="_evil">
                 <saml:Issuer>{IDP}</saml:Issuer>
                 <saml:Subject><saml:NameID>attacker@example.dev</saml:NameID></saml:Subject>
               </saml:Assertion></samlp:Response>"#
        ),
    );
    let error = SamlAssertion::parse(&doubled).expect_err("two assertions must be refused");
    assert!(
        error.to_string().contains("more than one Assertion"),
        "{error}"
    );
}

#[test]
fn a_doctype_is_refused_outright() {
    let xml = format!(
        r#"<?xml version="1.0"?><!DOCTYPE foo [<!ENTITY x "y">]>{}"#,
        valid().lines().skip(1).collect::<Vec<_>>().join("\n")
    );
    let error = SamlAssertion::parse(&xml).expect_err("a DOCTYPE must be refused");
    assert!(error.to_string().contains("DOCTYPE"), "{error}");
}

#[test]
fn an_undeclared_entity_is_refused_rather_than_dropped() {
    let extra = r#"<saml:Attribute Name="dept">
        <saml:AttributeValue>&secret;</saml:AttributeValue>
      </saml:Attribute>"#;
    let error = SamlAssertion::parse(&response(valid_window(), extra))
        .expect_err("an entity nothing declared must be refused");
    assert!(error.to_string().contains("undeclared entity"), "{error}");
}

#[test]
fn a_document_with_no_assertion_is_refused() {
    let error = SamlAssertion::parse("<samlp:Response xmlns:samlp='x'/>")
        .expect_err("no assertion is no assertion");
    assert!(error.to_string().contains("no Assertion"), "{error}");
}

// ── Validation ─────────────────────────────────────────────────────────

#[test]
fn a_well_formed_in_window_assertion_validates() {
    parsed().validate(&config(), now()).expect("valid");
}

#[test]
fn an_assertion_from_another_issuer_is_refused() {
    let mut config = config();
    config.idp_entity_id = "https://other-idp.example.com".to_string();
    let error = parsed()
        .validate(&config, now())
        .expect_err("an assertion from an IdP this team does not trust");
    assert!(
        error.to_string().contains("not by this team's IdP"),
        "{error}"
    );
}

#[test]
fn an_assertion_addressed_to_another_service_provider_is_refused() {
    let mut config = config();
    config.entity_id = "https://someone-elses-app.example".to_string();
    let error = parsed()
        .validate(&config, now())
        .expect_err("audience confusion must be refused");
    assert!(
        error.to_string().contains("not to this service provider"),
        "{error}"
    );
}

/// Stricter than the specification, deliberately: an assertion with no
/// audience is one any relying party can replay at us.
#[test]
fn an_assertion_with_no_audience_restriction_at_all_is_refused() {
    let xml = format!(
        r#"<saml:Assertion xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="_x">
             <saml:Issuer>{IDP}</saml:Issuer>
             <saml:Subject><saml:NameID>ada@example.dev</saml:NameID></saml:Subject>
             <saml:Conditions NotOnOrAfter="2026-08-30T12:05:00Z"/>
           </saml:Assertion>"#
    );
    let error = SamlAssertion::parse(&xml)
        .expect("parse")
        .validate(&config(), now())
        .expect_err("an unscoped assertion must be refused");
    assert!(
        error.to_string().contains("no AudienceRestriction"),
        "{error}"
    );
}

#[test]
fn an_expired_assertion_is_refused() {
    let late = now() + Duration::minutes(30);
    let error = parsed()
        .validate(&config(), late)
        .expect_err("a replayed assertion must be refused");
    assert!(error.to_string().contains("expired"), "{error}");
}

#[test]
fn an_assertion_that_has_not_opened_yet_is_refused() {
    let early = now() - Duration::hours(2);
    let error = parsed()
        .validate(&config(), early)
        .expect_err("an assertion from the future must be refused");
    assert!(error.to_string().contains("not valid before"), "{error}");
}

/// The skew is a tolerance, so a clock a minute past the expiry still
/// passes at the default two minutes and fails once it is tightened.
#[test]
fn the_configured_skew_is_what_decides_a_marginal_clock() {
    let just_past = now() + Duration::minutes(6);
    let assertion = parsed();
    assertion
        .validate(&config(), just_past)
        .expect("one minute past expiry is inside the default two-minute skew");

    let mut strict = config();
    strict.clock_skew_secs = 0;
    assertion
        .validate(&strict, just_past)
        .expect_err("with no skew allowed the same assertion is expired");
}

#[test]
fn an_assertion_that_never_expires_is_refused() {
    let xml = format!(
        r#"<saml:Assertion xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="_x">
             <saml:Issuer>{IDP}</saml:Issuer>
             <saml:Subject><saml:NameID>ada@example.dev</saml:NameID></saml:Subject>
             <saml:Conditions>
               <saml:AudienceRestriction><saml:Audience>{SP}</saml:Audience></saml:AudienceRestriction>
             </saml:Conditions>
           </saml:Assertion>"#
    );
    let error = SamlAssertion::parse(&xml)
        .expect("parse")
        .validate(&config(), now())
        .expect_err("an assertion with no expiry is replayable for ever");
    assert!(error.to_string().contains("no NotOnOrAfter"), "{error}");
}

#[test]
fn an_assertion_with_no_subject_is_refused() {
    let mut assertion = parsed();
    assertion.subject.clear();
    let error = assertion
        .validate(&config(), now())
        .expect_err("an assertion about nobody");
    assert!(error.to_string().contains("names no subject"), "{error}");
}

// ── Mapping to a roster identity ───────────────────────────────────────

#[test]
fn the_assertion_maps_to_a_roster_identity() {
    let identity = parsed().identity(&config());
    assert_eq!(identity.email.as_deref(), Some("ada@example.dev"));
    assert_eq!(identity.name, "Ada Lovelace");
    assert_eq!(identity.id(), "ada@example.dev");
}

/// The identity has to be told apart from a self-asserted one, or
/// `require_verified` could not tell them apart either.
#[test]
fn the_identity_carries_the_idp_as_its_provider() {
    assert_eq!(
        parsed().identity(&config()).provider.as_deref(),
        Some("saml:https://idp.example.com/metadata")
    );
}

#[test]
fn an_email_shaped_name_id_is_the_email_when_no_attribute_names_one() {
    let bare = SamlConfig::new(SP, IDP);
    let identity = parsed().identity(&bare);
    assert_eq!(identity.email.as_deref(), Some("ada@example.dev"));
    assert_eq!(identity.name, "ada");
}

#[test]
fn an_opaque_name_id_is_carried_as_the_name_with_no_email_invented() {
    let mut assertion = parsed();
    assertion.subject = "a7f3c9e1-opaque".to_string();
    let identity = assertion.identity(&SamlConfig::new(SP, IDP));
    assert_eq!(identity.email, None);
    assert_eq!(identity.name, "a7f3c9e1-opaque");
}

/// AD federations send group DNs in a SAML attribute exactly as they do
/// in `memberOf`, so one mapping table must serve both.
#[test]
fn a_group_sent_as_a_dn_maps_by_its_own_name() {
    let groups = parsed().groups(&config());
    assert_eq!(groups, ["eng-platform".to_string(), "everyone".to_string()]);
}

#[test]
fn the_assertions_groups_resolve_to_a_role() {
    let mapping = vec![GroupRole {
        group: "eng-*".into(),
        role: TeamRole::Developer,
    }];
    assert_eq!(
        DirectoryOutcome::from_groups(&parsed().groups(&config()), &mapping),
        DirectoryOutcome::Granted(TeamRole::Developer)
    );
}

// ── The HTTP-POST binding ──────────────────────────────────────────────

#[test]
fn a_base64_saml_response_decodes_and_parses() {
    let encoded = BASE64_STANDARD.encode(valid().as_bytes());
    let assertion = SamlAssertion::decode(&encoded).expect("decode");
    assert_eq!(assertion.subject, "ada@example.dev");
}

/// A form field wraps; whitespace in the payload is the norm, not an
/// error.
#[test]
fn a_wrapped_base64_payload_still_decodes() {
    let encoded = BASE64_STANDARD.encode(valid().as_bytes());
    let wrapped: String = encoded
        .as_bytes()
        .chunks(64)
        .map(|c| format!("{}\n", String::from_utf8_lossy(c)))
        .collect();
    assert_eq!(
        SamlAssertion::decode(&wrapped).expect("decode").subject,
        "ada@example.dev"
    );
}

#[test]
fn a_payload_that_is_not_base64_is_refused() {
    let error = SamlAssertion::decode("!!! not base64 !!!").expect_err("refused");
    assert!(error.to_string().contains("not base64"), "{error}");
}
