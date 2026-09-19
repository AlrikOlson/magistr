//! The `<!-- archetypes -->` section a polyglot init keeps in `CLAUDE.md`.
//!
//! `CLAUDE.md` stays the user's file; only the marked block listing each
//! archetype's language, root and gate commands is magistr's, spliced
//! between its markers when they exist and appended when they don't.

use magistr_core::types::{Archetype, Topology};

const START_MARKER: &str = "<!-- archetypes -->";
const END_MARKER: &str = "<!-- /archetypes -->";

/// `document` with its archetype section written or refreshed.
pub(crate) fn with_archetype_section(
    document: &str,
    archetypes: &[Archetype],
    topology: Topology,
) -> String {
    upsert_marked_section(
        document,
        START_MARKER,
        END_MARKER,
        &build_archetype_section(archetypes, topology),
    )
}

/// Build the archetype section content for CLAUDE.md.
fn build_archetype_section(archetypes: &[Archetype], topology: Topology) -> String {
    let mut s = String::new();
    s.push_str("## Project Archetypes\n\n");

    let topo_label = match topology {
        Topology::SingleProject => "Single Project",
        Topology::FlatMonorepo => "Flat Monorepo",
        Topology::NestedFullstack => "Nested Fullstack",
        Topology::Workspace => "Workspace",
    };
    s.push_str(&format!("**Topology:** {topo_label}\n\n"));

    s.push_str("| Archetype | Language | Root | Check | Test | Lint |\n");
    s.push_str("|-----------|----------|------|-------|------|------|\n");

    for arch in archetypes {
        let fw = arch
            .framework
            .as_deref()
            .map(|f| format!(" + {f}"))
            .unwrap_or_default();
        s.push_str(&format!(
            "| {} | {}{} | `{}` | `{}` | `{}` | `{}` |\n",
            arch.id,
            arch.language,
            fw,
            arch.root.display(),
            arch.quality_gate.check,
            arch.quality_gate.test,
            arch.quality_gate.lint,
        ));
    }
    s
}

/// Upsert a marked section in a document.
///
/// If the markers exist, replaces the content between them.
/// If the markers don't exist, appends the block at the end.
fn upsert_marked_section(doc: &str, start_marker: &str, end_marker: &str, content: &str) -> String {
    let block = format!("{start_marker}\n{content}{end_marker}");

    if let Some(start) = doc.find(start_marker)
        && let Some(end) = doc[start..].find(end_marker)
    {
        let end_abs = start + end + end_marker.len();
        let mut result = String::with_capacity(doc.len());
        result.push_str(&doc[..start]);
        result.push_str(&block);
        result.push_str(&doc[end_abs..]);
        return result;
    }

    // Markers not found — append.
    let mut result = doc.to_string();
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result.push('\n');
    result.push_str(&block);
    result.push('\n');
    result
}

#[cfg(test)]
#[path = "archetype_section.tests.rs"]
mod tests;
