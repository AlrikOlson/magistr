//! Dependency-graph walks — `impact_of` and its helpers.
//!
//! `impact_of` is the public entry point that powers the
//! `decision_impact` tool: it answers "if I revise this note,
//! what re-breaks?" by walking the dep graph upstream and downstream,
//! partitioning direct dependents by relation label, and surfacing the
//! revision chain plus any threads that fork off the target.
//!
//! All walks are capped at 256 nodes so a malicious or cyclical log
//! can't pin a CPU.

use std::collections::HashSet;

use serde_json::{Value, json};

use crate::decision::domain::{DepEdge, ThreadStatus};

use super::core::ReasoningServer;

impl ReasoningServer {
    /// Log the dependency and revision graph around a note. Returns
    /// upstream (what this note builds on, transitively), downstream (what
    /// depends on it, direct + transitive, partitioned by relation label
    /// when present), the chain of revisions through this note, and any
    /// threads that fork off it (with their merged_into pointer when set).
    pub fn impact_of(&self, note_number: u32) -> Result<Value, String> {
        let Some(target) = self.note_by_number(note_number) else {
            return Err(format!("No note #{note_number} found in the current log."));
        };

        // Upstream: walk `dependencies` backwards. Capped at 256 nodes so a
        // malicious/circular log can't pin a CPU.
        let upstream = self.walk_deps(note_number, 256);
        let downstream = self.walk_dependents(note_number, 256);
        let revision_chain = self.revision_chain_through(note_number);

        // Partition direct dependents by the relation label they used on
        // their edge to this note. Notes with no relation land in
        // `unlabeled`.
        let mut supported_by: Vec<u32> = Vec::new();
        let mut refuted_by: Vec<u32> = Vec::new();
        let mut depended_on_by: Vec<u32> = Vec::new();
        let mut unlabeled: Vec<u32> = Vec::new();
        for s in self.all_notes() {
            let Some(deps) = &s.dependencies else {
                continue;
            };
            for edge in deps {
                if edge.note() == note_number {
                    match edge.relation() {
                        Some("supports") => supported_by.push(s.note),
                        Some("refutes") => refuted_by.push(s.note),
                        Some("depends_on") => depended_on_by.push(s.note),
                        Some(_) => unlabeled.push(s.note),
                        None => unlabeled.push(s.note),
                    }
                    break;
                }
            }
        }
        for v in [
            &mut supported_by,
            &mut refuted_by,
            &mut depended_on_by,
            &mut unlabeled,
        ] {
            v.sort_unstable();
            v.dedup();
        }

        // Upstream direct deps: emit as note numbers (the structured edge is
        // already on the target note; consumers can re-fetch via note).
        let upstream_direct: Vec<u32> = target
            .dependencies
            .as_ref()
            .map(|deps| deps.iter().map(DepEdge::note).collect())
            .unwrap_or_default();

        let threads_from: Vec<Value> = self
            .threads
            .values()
            .filter(|b| b.from_note == note_number)
            .map(|b| {
                let mut obj = serde_json::Map::new();
                obj.insert("id".into(), json!(b.id));
                obj.insert("name".into(), json!(b.name));
                obj.insert(
                    "status".into(),
                    json!(match b.status {
                        ThreadStatus::Open => "open",
                        ThreadStatus::Merged => "merged",
                        ThreadStatus::Abandoned => "abandoned",
                    }),
                );
                obj.insert("note_count".into(), json!(b.notes.len()));
                if let Some(into) = b.merged_into {
                    obj.insert("merged_into".into(), json!(into));
                }
                Value::Object(obj)
            })
            .collect();

        Ok(json!({
            "note": note_number,
            "purpose": target.purpose,
            "confidence": target.confidence,
            "revised_by": target.revised_by,
            "revises_note": target.revises_note,
            "thread_id": target.thread_id,
            "upstream": {
                "direct": upstream_direct,
                "transitive": upstream,
            },
            "downstream": {
                "direct": self.direct_dependents(note_number),
                "transitive": downstream,
                "by_relation": {
                    "supports": supported_by,
                    "refutes": refuted_by,
                    "depends_on": depended_on_by,
                    "unlabeled": unlabeled,
                },
            },
            "revision_chain": revision_chain,
            "threads_from": threads_from,
        }))
    }

    pub(crate) fn walk_deps(&self, start: u32, cap: usize) -> Vec<u32> {
        let mut seen: HashSet<u32> = HashSet::new();
        let mut stack: Vec<u32> = vec![start];
        let mut out: Vec<u32> = Vec::new();
        while let Some(n) = stack.pop() {
            if seen.len() >= cap {
                break;
            }
            if let Some(note) = self.note_by_number(n) {
                if let Some(deps) = note.dependencies {
                    for edge in deps {
                        let d = edge.note();
                        if seen.insert(d) {
                            out.push(d);
                            stack.push(d);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    pub(crate) fn direct_dependents(&self, target: u32) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .all_notes()
            .filter(|s| {
                s.dependencies
                    .as_ref()
                    .is_some_and(|d| d.iter().any(|e| e.note() == target))
            })
            .map(|s| s.note)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Note numbers of every note whose edge to `target` carries
    /// `relation: "refutes"`. Used by `compute_note_warnings` to flag
    /// building on something that's been refuted elsewhere.
    pub(crate) fn refuters_of(&self, target: u32, exclude: u32) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .all_notes()
            .filter(|s| s.note != exclude)
            .filter(|s| {
                s.dependencies.as_ref().is_some_and(|d| {
                    d.iter()
                        .any(|e| e.note() == target && e.relation() == Some("refutes"))
                })
            })
            .map(|s| s.note)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    pub(crate) fn walk_dependents(&self, start: u32, cap: usize) -> Vec<u32> {
        let mut seen: HashSet<u32> = HashSet::new();
        let mut stack: Vec<u32> = vec![start];
        let mut out: Vec<u32> = Vec::new();
        while let Some(n) = stack.pop() {
            if seen.len() >= cap {
                break;
            }
            for d in self.direct_dependents(n) {
                if seen.insert(d) {
                    out.push(d);
                    stack.push(d);
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Build the revision chain that runs through `note`: walk back
    /// through `revises_note` to the original, then forward through
    /// `revised_by` to the latest. Returns note numbers in chronological
    /// order.
    pub(crate) fn revision_chain_through(&self, note_number: u32) -> Vec<u32> {
        let Some(start) = self.note_by_number(note_number) else {
            return Vec::new();
        };

        let mut origin = note_number;
        let mut cur = start.clone();
        let mut guard = 0u32;
        while let Some(prev) = cur.revises_note {
            if guard >= 256 {
                break;
            }
            guard += 1;
            origin = prev;
            match self.note_by_number(prev) {
                Some(p) => cur = p,
                None => break,
            }
        }

        let mut chain: Vec<u32> = vec![origin];
        let mut cur_n = origin;
        let mut guard = 0u32;
        loop {
            if guard >= 256 {
                break;
            }
            guard += 1;
            let Some(s) = self.note_by_number(cur_n) else {
                break;
            };
            let Some(next) = s.revised_by else {
                break;
            };
            chain.push(next);
            cur_n = next;
        }

        if chain.len() <= 1 { Vec::new() } else { chain }
    }
}
