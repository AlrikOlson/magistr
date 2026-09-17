use std::{
    path::Path,
    sync::{Arc, PoisonError, RwLock},
};

/// Host-side language selection for a path: the bundled extension table
/// first, then the lang-pack claims the owner retains from healthy
/// registrations. A bundled extension is never shadowed — the bundled table
/// answers before any claim is consulted, and the worker's registry refuses a
/// shadowing claim at registration — and a workspace without packs pays one
/// read lock on a bundled miss only. Every consumer of a workspace (index
/// scan, live queries, rewrite, parse-verify) holds a clone of the owner's
/// handle, so a registration a later incarnation loses is unselected
/// everywhere at once.
#[derive(Clone, Default)]
pub struct SourceLanguages {
    claims: Arc<RwLock<Vec<Claim>>>,
}

/// One retained registration: the language id and the extensions the
/// registry granted it.
type Claim = (String, Vec<String>);

impl SourceLanguages {
    /// The bundled table alone.
    #[must_use]
    pub fn bundled() -> Self {
        Self::default()
    }

    /// The language `path` parses as, or `None` when neither the bundled
    /// table nor a retained claim owns its extension.
    #[must_use]
    pub fn language(&self, path: &str) -> Option<String> {
        if let Some(bundled) = crate::source_language::language(path) {
            return Some(bundled.to_owned());
        }
        let extension = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
        self.claims
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|(_, extensions)| extensions.contains(&extension))
            .map(|(language, _)| language.clone())
    }

    /// The language whose *pattern* engine (ast-grep structural match and
    /// rewrite) can take `path`: the bundled table only, whatever packs are
    /// retained. A pack grammar parses, verifies and extracts symbols, but no
    /// pattern language exists for it, so structural search and rewrite skip
    /// its files rather than fail the whole scan on a language the engine
    /// cannot pattern-match.
    #[must_use]
    pub fn pattern_language(path: &str) -> Option<String> {
        crate::source_language::language(path).map(str::to_owned)
    }

    /// Replace the retained claims with one incarnation's healthy
    /// registrations: `(language id, extensions as the registry granted
    /// them)`. The registry has already dropped bundled and inter-pack
    /// duplicates, so first claim wins here exactly as it does there.
    pub fn retain(&self, claims: Vec<Claim>) {
        *self.claims.write().unwrap_or_else(PoisonError::into_inner) = claims;
    }

    /// The retained claims, registration order.
    #[must_use]
    pub fn claims(&self) -> Vec<Claim> {
        self.claims
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl std::fmt::Debug for SourceLanguages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceLanguages")
            .field("claims", &self.claims())
            .finish()
    }
}

#[cfg(test)]
#[path = "source_languages.tests.rs"]
mod tests;
