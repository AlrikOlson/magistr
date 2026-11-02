//! Import authored identities without treating numbers or prose as identity.
use std::collections::{HashMap, HashSet};

use chrono::Utc;
use magistr_workflow_contracts::Observed;

use super::ReasoningServer;
use crate::decision::domain::{DepEdge, Note, NoteError};

impl ReasoningServer {
    /// Import a coherent batch silently; return a count only after committing it.
    pub fn adopt_notes(&mut self, notes: Vec<Note>) -> Result<Observed<usize>, NoteError> {
        let persistence = self.persistence.clone();
        let project_id = self.project_id.clone();
        self.transact_project(self.active_log.clone(), move |project| {
            let known: HashMap<String, Note> = project.logs.values()
                .flat_map(|history| &history.notes)
                .map(|note| (note.record_id.clone().expect("validated identity"), note.clone()))
                .collect();
            let mut used: HashSet<u32> = known.values().map(|note| note.note).collect();
            let mut highest = used.iter().copied().max().unwrap_or(0);
            let mut by_number: HashMap<u32, HashSet<String>> = HashMap::new();
            let mut unique: HashMap<String, Note> = HashMap::new();
            let mut order = Vec::new();
            for note in notes {
                let id = note.record_id.as_deref().filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| conflict("An imported reasoning item has no record identity."))?
                    .to_owned();
                if note.note == 0 {
                    return Err(conflict("An imported reasoning item has no number."));
                }
                let log = note.log_id.as_deref().unwrap_or(&project_id);
                persistence.validate_log_id(log)
                    .map_err(|error| conflict(&error.to_string()))?;
                by_number.entry(note.note).or_default().insert(id.clone());
                if let Some(prior) = unique.get(&id) {
                    if prior != &note {
                        return Err(conflict("The import contains conflicting copies of one reasoning item."));
                    }
                    continue;
                }
                order.push(id.clone());
                unique.insert(id, note);
            }
            let mut assigned = HashMap::new();
            for id in &order {
                let note = &unique[id];
                let number = if let Some(existing) = known.get(id) {
                    existing.note
                } else if used.insert(note.note) {
                    highest = highest.max(note.note);
                    note.note
                } else {
                    highest = highest.checked_add(1)
                        .ok_or_else(|| conflict("There are no unused reasoning numbers left."))?;
                    used.insert(highest);
                    highest
                };
                assigned.insert(id.clone(), number);
            }
            let remap = |number: u32| -> Result<u32, NoteError> {
                match by_number.get(&number) {
                    Some(ids) if ids.len() == 1 => Ok(assigned[ids.iter().next().expect("one id")]),
                    Some(_) => Err(conflict("An imported reference matches several distinct reasoning items. Include an unambiguous history before retrying.")),
                    None if known.values().any(|note| note.note == number) => Ok(number),
                    None => Err(conflict("An imported reference has no corresponding reasoning item. Include the referenced history before retrying.")),
                }
            };
            let mut adopted = 0;
            let mut changed_logs = HashSet::new();
            for id in order {
                let mut note = unique.remove(&id).expect("ordered identity");
                note.note = assigned[&id];
                note.revises_note = note.revises_note.map(remap).transpose()?;
                note.revised_by = note.revised_by.map(remap).transpose()?;
                note.thread_from = note.thread_from.map(remap).transpose()?;
                if let Some(dependencies) = &mut note.dependencies {
                    for dependency in dependencies {
                        match dependency {
                            DepEdge::Bare(number) | DepEdge::Tagged { note: number, .. } => {
                                *number = remap(*number)?;
                            }
                        }
                    }
                }
                if let Some(existing) = known.get(&id) {
                    if !same_authored_record(existing, &note, &project_id) {
                        return Err(conflict("An imported reasoning identity has different authored content. Resolve the conflicting record before retrying."));
                    }
                    continue;
                }
                let log = note.log_id.clone().unwrap_or_else(|| project_id.clone());
                let history = project.logs.entry(log.clone()).or_insert_with(|| {
                    let mut history = Self::new_history();
                    history.log_id = (log != project_id).then(|| log.clone());
                    history.metadata.as_mut().expect("new history metadata").project_id = Some(project_id.clone());
                    history
                });
                if let Some(thread_id) = &note.thread_id {
                    if let Some(thread) = history.threads.iter().flatten().find(|thread| &thread.id == thread_id) {
                        if note.thread_from != Some(thread.from_note)
                            || note.thread_name.as_deref() != Some(thread.name.as_str())
                        {
                            return Err(conflict("An imported question disagrees with its saved origin or name."));
                        }
                    } else if note.thread_from.is_none() || note.thread_name.as_deref().is_none_or(|name| name.trim().is_empty()) {
                        return Err(conflict("An imported question is missing its origin or name."));
                    }
                }
                history.completed |= note.is_final_note == Some(true);
                history.notes.push(note);
                history.updated_at = Some(Utc::now().to_rfc3339());
                changed_logs.insert(log);
                adopted += 1;
            }
            for log in changed_logs {
                let history = project.logs.get_mut(&log).expect("changed history");
                history.notes.sort_by_key(|note| note.note);
                let mut threads: Vec<_> = Self::restored_threads(history).into_values().collect();
                threads.sort_by(|a, b| a.from_note.cmp(&b.from_note).then(a.id.cmp(&b.id)));
                history.threads = Some(threads);
                let meta = history.metadata.get_or_insert_with(Default::default);
                meta.project_id = Some(project_id.clone());
                let mut tools: Vec<_> = history.notes.iter().flat_map(Self::extract_tools_used).collect();
                tools.sort();
                tools.dedup();
                meta.tools_used = Some(tools);
            }
            Ok(adopted)
        })
    }
}

fn conflict(message: &str) -> NoteError {
    NoteError::rejected(
        message,
        "Resolve the conflicting reasoning identities or references, then retry.",
    )
}

fn same_authored_record(left: &Note, right: &Note, project_id: &str) -> bool {
    fn authored(mut note: Note, project_id: &str) -> Note {
        // These fields have explicit local mutators or are projections of identity.
        note.note = 0;
        note.estimated_total = 0;
        note.pinned = None;
        note.revised_by = None;
        if note.log_id.as_deref() == Some(project_id) {
            note.log_id = None;
        }
        note
    }
    authored(left.clone(), project_id) == authored(right.clone(), project_id)
}
