//! A SAML 2.0 assertion, read and checked.

use std::collections::BTreeMap;

use base64::prelude::*;
use chrono::{DateTime, Duration, Utc};
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::MagistrError;
use crate::team::Identity;

use super::ldap_config::LdapConfig;
use super::saml_config::SamlConfig;

/// The parts of a SAML assertion magistr acts on.
///
/// **The XML signature is not verified.** The checks that *are*
/// performed — [`validate`](Self::validate) — are the ones the
/// recurring real-world failure is about: the validity window, the
/// audience restriction and the issuer. Signature verification needs
/// XMLDSig with exclusive canonicalization and is its own chunk, the
/// sibling of the OIDC `id_token`'s JWKS verification.
///
/// One half of the signature story *is* enforced here, because it costs
/// no crypto: [`parse`](Self::parse) refuses a document that carries
/// more than one `<Assertion>`, and refuses any DOCTYPE. Those two are
/// XML Signature Wrapping's own preconditions — the attack works by
/// leaving a validly-signed assertion in place and adding a second,
/// unsigned one for the data reader to pick up — so an unambiguous
/// single assertion cannot be wrapped whatever the signature situation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SamlAssertion {
    /// The assertion's own `ID` attribute.
    pub id: String,
    /// The `<Issuer>` inside the assertion (never the Response's).
    pub issuer: String,
    /// The `<NameID>` of the subject.
    pub subject: String,
    /// `Conditions/@NotBefore`, when present.
    pub not_before: Option<DateTime<Utc>>,
    /// `Conditions/@NotOnOrAfter`, when present.
    pub not_on_or_after: Option<DateTime<Utc>>,
    /// Every `<Audience>` named by an `<AudienceRestriction>`.
    pub audiences: Vec<String>,
    /// The attribute statement, by attribute `Name`.
    pub attributes: BTreeMap<String, Vec<String>>,
}

impl SamlAssertion {
    /// Read an assertion out of XML.
    ///
    /// # Errors
    ///
    /// When the document is not well-formed, carries a DOCTYPE, or
    /// carries anything other than exactly one `<Assertion>`.
    pub fn parse(xml: &str) -> Result<Self, MagistrError> {
        let mut reader = Reader::from_str(xml);
        let mut state = ParseState::default();
        loop {
            match reader.read_event().map_err(bad_xml)? {
                Event::Eof => break,
                Event::DocType(_) => {
                    return Err(refused(
                        "the document carries a DOCTYPE, which is refused: an entity \
                         declaration in an assertion is an attack, never a feature",
                    ));
                }
                Event::Start(e) => state.open(&e, true)?,
                Event::Empty(e) => state.open(&e, false)?,
                Event::End(e) => state.close(&local_name(e.local_name().as_ref())),
                Event::Text(e) => {
                    let text = e.xml10_content();
                    state.push_text(&text);
                }
                Event::GeneralRef(e) => state.push_text(&entity(&e)?),
                _ => {}
            }
        }
        state.finish()
    }

    /// Read an assertion out of the base64 `SAMLResponse` an HTTP-POST
    /// binding delivers.
    ///
    /// # Errors
    ///
    /// When the payload is not base64, or the XML inside it is refused.
    pub fn decode(response: &str) -> Result<Self, MagistrError> {
        let compact: String = response.split_whitespace().collect();
        let bytes = BASE64_STANDARD
            .decode(compact.as_bytes())
            .map_err(|e| refused(&format!("the SAMLResponse is not base64: {e}")))?;
        let xml = String::from_utf8(bytes)
            .map_err(|e| refused(&format!("the SAMLResponse is not UTF-8: {e}")))?;
        Self::parse(&xml)
    }

    /// Check the assertion against what this team trusts, as of `now`.
    ///
    /// Refuses on the four grounds that matter without a signature: an
    /// issuer that is not the configured IdP, an audience that does not
    /// name us, a validity window that has not opened or has closed,
    /// and — deliberately stricter than the specification — an
    /// assertion carrying *no* audience restriction or *no* expiry at
    /// all. An assertion with no audience is one any relying party can
    /// replay at us, and one with no expiry never stops being
    /// replayable; neither is something to accept from an IdP we cannot
    /// otherwise verify.
    ///
    /// # Errors
    ///
    /// When any of the above fails.
    pub fn validate(&self, config: &SamlConfig, now: DateTime<Utc>) -> Result<(), MagistrError> {
        if self.subject.trim().is_empty() {
            return Err(refused("the assertion names no subject"));
        }
        if self.issuer != config.idp_entity_id {
            return Err(refused(&format!(
                "the assertion was issued by '{}', not by this team's IdP '{}'",
                self.issuer, config.idp_entity_id
            )));
        }
        if self.audiences.is_empty() {
            return Err(refused(
                "the assertion carries no AudienceRestriction, so it is not scoped \
                 to this service provider",
            ));
        }
        if !self.audiences.iter().any(|a| a == &config.entity_id) {
            return Err(refused(&format!(
                "the assertion is addressed to {:?}, not to this service provider '{}'",
                self.audiences, config.entity_id
            )));
        }
        self.validate_window(config, now)
    }

    /// The validity window, with the configured skew allowed at both
    /// ends.
    fn validate_window(&self, config: &SamlConfig, now: DateTime<Utc>) -> Result<(), MagistrError> {
        let skew = Duration::seconds(i64::from(config.clock_skew_secs));
        let Some(expiry) = self.not_on_or_after else {
            return Err(refused(
                "the assertion has no NotOnOrAfter, so it would never stop being \
                 replayable",
            ));
        };
        if now - skew >= expiry {
            return Err(refused(&format!(
                "the assertion expired at {expiry} (now {now})"
            )));
        }
        if let Some(start) = self.not_before
            && now + skew < start
        {
            return Err(refused(&format!(
                "the assertion is not valid before {start} (now {now})"
            )));
        }
        Ok(())
    }

    /// The values of one attribute, by `Name`.
    #[must_use]
    pub fn attribute(&self, name: &str) -> &[String] {
        self.attributes.get(name).map_or(&[], Vec::as_slice)
    }

    /// The groups this assertion claims, as names the mapping matches.
    ///
    /// Each value goes through [`LdapConfig::group_name`], because an
    /// Active Directory federation sends group DNs in a SAML attribute
    /// exactly as it does in `memberOf`, and a mapping row is written
    /// against the group's own name. A bare name passes through
    /// unchanged, so one table serves both federations.
    #[must_use]
    pub fn groups(&self, config: &SamlConfig) -> Vec<String> {
        self.attribute(&config.groups_attribute)
            .iter()
            .map(|value| LdapConfig::group_name(value))
            .collect()
    }

    /// The roster identity this assertion asserts.
    ///
    /// The `provider` is `saml:<idp entity id>`, so an identity the IdP
    /// vouched for is never mistaken for a self-asserted one.
    #[must_use]
    pub fn identity(&self, config: &SamlConfig) -> Identity {
        let email = config
            .email_attribute
            .as_deref()
            .and_then(|name| self.attribute(name).first())
            .map(String::as_str)
            .or_else(|| looks_like_email(&self.subject).then_some(self.subject.as_str()))
            .map(str::to_string);
        let name = config
            .name_attribute
            .as_deref()
            .and_then(|name| self.attribute(name).first())
            .cloned()
            .or_else(|| {
                email
                    .as_deref()
                    .and_then(|e| e.split('@').next())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| self.subject.clone());
        Identity {
            name,
            email,
            provider: Some(config.provider_key()),
        }
    }
}

fn looks_like_email(value: &str) -> bool {
    matches!(value.split_once('@'), Some((user, host)) if !user.is_empty() && host.contains('.'))
}

fn bad_xml(e: impl std::fmt::Display) -> MagistrError {
    MagistrError::parse("saml", "assertion", e)
}

fn refused(why: &str) -> MagistrError {
    MagistrError::Permission(format!("SAML assertion refused: {why}"))
}

fn local_name(name: &str) -> String {
    name.to_owned()
}

/// The walk over the document: an element stack, the text piling up
/// inside the element on top of it, and where the one assertion begins.
#[derive(Default)]
struct ParseState {
    stack: Vec<String>,
    /// Character data seen since the innermost element opened.
    ///
    /// It accumulates rather than being read event by event because a
    /// value carrying an entity — `Ops &amp; Platform` is an ordinary
    /// group name — arrives as three events, and taking only the first
    /// would silently truncate it at the ampersand.
    text: String,
    /// The stack depth the assertion opened at; `None` outside it.
    inside: Option<usize>,
    seen: usize,
    attribute: Option<String>,
    out: SamlAssertion,
}

impl ParseState {
    /// An element opened. `push` is false for a self-closing one.
    fn open(&mut self, e: &BytesStart<'_>, push: bool) -> Result<(), MagistrError> {
        self.text.clear();
        let name = local_name(e.local_name().as_ref());
        match name.as_str() {
            "Assertion" => {
                self.seen += 1;
                if self.seen > 1 {
                    return Err(refused(
                        "the document carries more than one Assertion; only an \
                         unambiguous single assertion is accepted",
                    ));
                }
                self.out.id = attr(e, "ID")?.unwrap_or_default();
                self.inside = Some(self.stack.len());
            }
            "Conditions" if self.inside.is_some() => {
                self.out.not_before = timestamp(e, "NotBefore")?;
                self.out.not_on_or_after = timestamp(e, "NotOnOrAfter")?;
            }
            "Attribute" if self.inside.is_some() => self.attribute = attr(e, "Name")?,
            _ => {}
        }
        if push {
            self.stack.push(name);
        }
        Ok(())
    }

    /// An element closed: its accumulated text is its value.
    fn close(&mut self, name: &str) {
        self.take_text();
        self.stack.pop();
        if name == "Assertion" {
            self.inside = None;
        } else if name == "Attribute" {
            self.attribute = None;
        }
    }

    fn push_text(&mut self, text: &str) {
        self.text.push_str(text);
    }

    /// File the text of the element that is closing, by where the stack
    /// says it sits.
    fn take_text(&mut self) {
        let text = std::mem::take(&mut self.text).trim().to_string();
        if self.inside.is_none() || text.is_empty() {
            return;
        }
        let last = self.stack.last().map(String::as_str);
        let parent = self
            .stack
            .len()
            .checked_sub(2)
            .and_then(|i| self.stack.get(i))
            .map(String::as_str);
        match (parent, last) {
            (Some("Assertion"), Some("Issuer")) => self.out.issuer = text,
            (Some("Subject"), Some("NameID")) => self.out.subject = text,
            (Some("AudienceRestriction"), Some("Audience")) => self.out.audiences.push(text),
            (Some("Attribute"), Some("AttributeValue")) => {
                if let Some(name) = self.attribute.clone() {
                    self.out.attributes.entry(name).or_default().push(text);
                }
            }
            _ => {}
        }
    }

    fn finish(self) -> Result<SamlAssertion, MagistrError> {
        if self.seen == 0 {
            return Err(refused("the document carries no Assertion"));
        }
        Ok(self.out)
    }
}

/// Resolve one entity or character reference.
///
/// Only the five predefined XML entities and numeric character
/// references resolve. Anything else would have had to be declared in a
/// DOCTYPE, which [`SamlAssertion::parse`] has already refused — so an
/// unknown name here is a document doing something it was told not to,
/// and it is an error rather than a silently dropped character.
fn entity(e: &BytesRef<'_>) -> Result<String, MagistrError> {
    if let Some(ch) = e.resolve_char_ref().map_err(bad_xml)? {
        return Ok(ch.to_string());
    }
    let name: &str = e;
    resolve_predefined_entity(name)
        .map(str::to_string)
        .ok_or_else(|| {
            refused(&format!(
                "the assertion references an undeclared entity '&{name};'"
            ))
        })
}

/// One attribute of an element, by local name.
fn attr(e: &BytesStart<'_>, name: &str) -> Result<Option<String>, MagistrError> {
    for attribute in e.attributes() {
        let attribute = attribute.map_err(bad_xml)?;
        if attribute.key.local_name().as_ref() == name {
            return Ok(Some(
                attribute
                    .normalized_value(XmlVersion::Implicit1_0)
                    .map_err(bad_xml)?
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

/// One `xs:dateTime` attribute, by local name.
fn timestamp(e: &BytesStart<'_>, name: &str) -> Result<Option<DateTime<Utc>>, MagistrError> {
    let Some(raw) = attr(e, name)? else {
        return Ok(None);
    };
    let parsed = DateTime::parse_from_rfc3339(&raw)
        .map_err(|e| refused(&format!("{name} is not a valid timestamp ('{raw}'): {e}")))?;
    Ok(Some(parsed.with_timezone(&Utc)))
}

#[cfg(test)]
#[path = "saml_assertion.tests.rs"]
mod tests;
