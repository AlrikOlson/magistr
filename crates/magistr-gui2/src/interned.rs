//! Project names given the `'static` life a mark's identity needs.

use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};

/// `name`, interned: the same `'static` string for the same name, for
/// as long as the process runs.
///
/// A [`Mark::Sigil`](crate::Mark::Sigil) carries the project's *name*
/// as its identity — that is what lets the atlas key a raster on the
/// mark itself — and the element tree asks for a `'static` one. The
/// brand's name is a literal; a project the Galaxy learned from the
/// session database is not. Interning gives such a name the life the
/// mark needs, once per distinct name: the set is bounded by the number
/// of projects a machine has ever worked on, which is the kind of leak
/// an interner is for.
pub fn interned(name: &str) -> &'static str {
    static NAMES: OnceLock<Mutex<BTreeSet<&'static str>>> = OnceLock::new();
    let names = NAMES.get_or_init(|| Mutex::new(BTreeSet::new()));
    let mut names = names
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(known) = names.get(name) {
        return known;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.insert(leaked);
    leaked
}

#[cfg(test)]
#[path = "interned.tests.rs"]
mod tests;
