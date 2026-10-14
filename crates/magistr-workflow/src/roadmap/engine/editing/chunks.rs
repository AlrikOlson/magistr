use super::*;

impl Edit<'_> {
    pub fn upsert_chunk(&mut self, chunk: Chunk) -> Result<bool, String> {
        // The same refusal as `add_chunk_with_content`, at the cloud door: a
        // record with no id cannot be addressed by anything downstream. The
        // push lane already skips these (`cloud::backfill`), so accepting one
        // here would only re-import a defect the other doors now reject.
        if chunk.id.trim().is_empty() {
            return Err("a chunk needs an id".into());
        }
        if chunk.project_id.as_deref() != Some(self.project_id.as_str()) {
            return Err("The work item belongs to another project.".into());
        }
        match self.index_of(&chunk.id) {
            Ok(idx) => {
                if !crate::roadmap::domain::chunk_wins(&chunk, &self.roadmap.chunks[idx]) {
                    return Ok(false); // The stored copy is at least as new.
                }
                self.roadmap.chunks[idx] = chunk;
            }
            Err(_) => self.roadmap.chunks.push(chunk),
        }
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)] // Mirrors the public chunk creation command.
    pub fn add_chunk(
        &mut self,
        id: String,
        title: String,
        status: ChunkStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        shared: bool,
    ) -> Result<&Chunk, String> {
        self.add_chunk_with_content(
            id,
            title,
            String::new(),
            status,
            priority,
            description,
            acceptance,
            deps,
            shared,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_chunk_with_content(
        &mut self,
        id: String,
        title: String,
        name: String,
        status: ChunkStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        shared: bool,
        content: Option<crate::content::StructuredContent>,
        group: Option<String>,
    ) -> Result<&Chunk, String> {
        // The id is the one field every other verb keys on: a chunk without
        // one cannot be linked, mutated, mirrored, or drawn. Two real stores
        // held such a record — both admitted here, back when this door only
        // checked for duplicates. This check is what makes `name::derive`'s
        // "the engine rejects that id upstream" true.
        if id.trim().is_empty() {
            return Err(
                "a chunk needs an id — it is the field every other verb addresses it by"
                    .to_string(),
            );
        }
        if self.roadmap.chunks.iter().any(|c| c.id == id) {
            return Err(format!("chunk '{id}' already exists"));
        }
        // "Required or derived, never absent": a caller may omit the name and
        // get a usable one, but no path exists that creates a nameless chunk.
        // An over-budget name IS rejected — the whole point of the field is that
        // a sentence cannot be what a node wears, and accepting one here would
        // be the leak that reopens the defect.
        let name = match name.trim() {
            "" => crate::roadmap::name::derive(&id),
            given => {
                if let Some(why) = crate::roadmap::name::why_unfit(given) {
                    return Err(format!("chunk '{id}': {why}"));
                }
                given.to_string()
            }
        };
        // The workstream is validated BEFORE anything is written, and rejected
        // as a whole call rather than dropped. A chunk born ungrouped because
        // its group was quietly ignored is invisible to every focused read —
        // `next_in_group` will not offer it and the frontier will not count
        // it — and nothing about the created chunk says why. Refusing here is
        // the only outcome a caller can act on.
        let group = Self::clean_group(group);
        if let Some(name) = &group
            && let Some(why) = self.why_group_unfit(name, Some(&id))
        {
            return Err(format!("cannot group chunk '{id}': {why}"));
        }
        if let Some(content) = &content {
            content.validate()?;
        }
        let now = Self::now();
        // Captured before `id` moves into the chunk: the inherited opt-in below
        // needs to name it, and the status decides whether it qualifies.
        let born = id.clone();
        let born_active = status.is_active();
        self.roadmap.chunks.push(Chunk {
            tier: None,
            id,
            title,
            name,
            status,
            priority,
            description,
            content,
            notes: String::new(),
            group,
            acceptance,
            deps,
            cross_refs: Vec::new(),
            shared,
            reprioritize: None,
            status_proposal: None,
            title_proposal: None,
            obsoleted_reason: None,
            blocked_by: None,
            // Stamped at birth: this engine is this project, so a chunk it
            // records is provably ours and every later copy carries the proof.
            project_id: Some(self.project_id.clone()),
            created_at: now.clone(),
            updated_at: now,
        });
        let snapshot = self.roadmap.chunks.last().unwrap().clone();
        self.record_event(RoadmapFrame::ChunkAdded { chunk: snapshot });

        // THE INHERITANCE, and the only place it happens
        // (tracker-optin-never-grows). A project whose owner explicitly ran
        // `tracker on` should not have to re-consent to every chunk the plan
        // grows; the argument for reversing the silent-by-default rule, and the
        // reason it costs nothing, is written at
        // [`crate::tracker::config::inherited_opt_in`], which is what decides
        // whether this field is set at all.
        //
        // That this is inside `add_chunk` — reachable only by a chunk coming
        // into existence — is what makes "never a retroactive sweep" structural
        // rather than a promise: there is no loop here that could reach a chunk
        // that already exists, so there is nothing to accidentally point at one.
        if born_active && let Some(provider) = self.inherit_opt_in.clone() {
            self.set_tracker_opt_in(&born, &provider, true)?;
        }

        self.roadmap
            .chunks
            .iter()
            .find(|c| c.id == born)
            .ok_or_else(|| "chunk vanished after add".to_string())
    }

    pub fn set_status(&mut self, id: &str, to: ChunkStatus) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        let from = self.roadmap.chunks[idx].status;
        // Starting scheduled work requires readiness. Done -> InProgress is
        // completion revocation for an existing session, not a new start.
        if to == ChunkStatus::InProgress
            && matches!(from, ChunkStatus::Pending | ChunkStatus::Blocked)
        {
            self.require_ready(&self.roadmap.chunks[idx])?;
        }
        if !from.allows(to) {
            return Err(format!(
                "illegal transition for chunk '{id}': {from:?} -> {to:?}"
            ));
        }
        let c = &mut self.roadmap.chunks[idx];
        c.status = to;
        c.status_proposal = None;
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_chunk(
        &mut self,
        id: &str,
        title: Option<String>,
        priority: Option<u32>,
        description: Option<String>,
        acceptance: Option<Vec<String>>,
        deps: Option<Vec<String>>,
        content: Option<crate::content::StructuredContent>,
    ) -> Result<&Chunk, String> {
        if let Some(content) = &content {
            content.validate()?;
        }
        let idx = self.index_of(id)?;
        let c = &mut self.roadmap.chunks[idx];
        if let Some(t) = title {
            // An explicit title edit is the human speaking about exactly the
            // field a title proposal is about, so it disposes the proposal —
            // otherwise the projector would keep sending the conceded value
            // and the edit the human just made could never reach the tracker.
            if c.title != t {
                c.title_proposal = None;
            }
            c.title = t;
        }
        if let Some(p) = priority {
            // Same rule as the title edit above, for the reprioritize sibling:
            // an explicit priority edit is the human acting on exactly the
            // field the proposal is about, so it disposes the proposal.
            if c.priority != p {
                c.reprioritize = None;
            }
            c.priority = p;
        }
        if let Some(d) = description {
            c.description = d;
        }
        if let Some(a) = acceptance {
            c.acceptance = a;
        }
        if let Some(d) = deps {
            c.deps = d;
        }
        if let Some(sc) = content {
            c.content = Some(sc);
        }
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn set_group(&mut self, id: &str, group: Option<String>) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        let cleaned = Self::clean_group(group);
        if let Some(name) = &cleaned
            && let Some(why) = self.why_group_unfit(name, None)
        {
            return Err(format!("cannot group chunk '{id}': {why}"));
        }
        let c = &mut self.roadmap.chunks[idx];
        if c.group == cleaned {
            // No-op writes still cost a persist and an event, and a projector
            // that re-groups every push would churn the tracker for nothing.
            return Ok(&self.roadmap.chunks[idx]);
        }
        c.group = cleaned;
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn set_name(&mut self, id: &str, name: &str) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        let cleaned = match name.trim() {
            "" => crate::roadmap::name::derive(id),
            given => {
                if let Some(why) = crate::roadmap::name::why_unfit(given) {
                    return Err(format!("chunk '{id}': {why}"));
                }
                given.to_string()
            }
        };
        let c = &mut self.roadmap.chunks[idx];
        if c.name == cleaned {
            // No-op writes still cost a persist and an event, and the projector
            // would churn the tracker for nothing — same reasoning as set_group.
            return Ok(&self.roadmap.chunks[idx]);
        }
        c.name = cleaned;
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn focus_set(&mut self, lane: &str, group: &str, mode: FocusMode) -> Result<Focus, String> {
        let lane = validate_lane(lane)?;
        let resolved = match self.workstreams().resolve_current(group) {
            WorkstreamResolution::Current { group } => group,
            other => return Err(other.explain(group)),
        };
        let now = Self::now();
        let focus = match self.roadmap.focuses.iter_mut().find(|f| f.lane == lane) {
            Some(existing) => {
                existing.group = resolved;
                existing.mode = mode;
                existing.updated_at = now;
                existing.clone()
            }
            None => {
                let focus = Focus {
                    lane,
                    group: resolved,
                    mode,
                    set_at: now.clone(),
                    updated_at: now,
                };
                self.roadmap.focuses.push(focus.clone());
                focus
            }
        };
        Ok(focus)
    }

    pub fn focus_clear(&mut self, lane: &str) -> Result<bool, String> {
        let lane = validate_lane(lane)?;
        let before = self.roadmap.focuses.len();
        self.roadmap.focuses.retain(|f| f.lane != lane);
        let removed = self.roadmap.focuses.len() != before;
        Ok(removed)
    }

    pub fn obsolete_chunk(&mut self, id: &str, reason: String) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        let from = self.roadmap.chunks[idx].status;
        if !from.allows(ChunkStatus::Obsoleted) {
            return Err(format!("cannot obsolete chunk '{id}' from status {from:?}"));
        }
        let c = &mut self.roadmap.chunks[idx];
        c.status = ChunkStatus::Obsoleted;
        // The one status write outside `set_status`, so the field-scoped
        // disposal rule applies here too: obsoleting is the strongest possible
        // human statement about the status field.
        c.status_proposal = None;
        c.obsoleted_reason = Some(reason);
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkObsoleted { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn start_chunk(&mut self, id: &str) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        self.require_ready(&self.roadmap.chunks[idx])?;
        self.set_status(id, ChunkStatus::InProgress)
    }

    pub fn complete_chunk(&mut self, id: &str, ship_ref: Option<&str>) -> Result<&Chunk, String> {
        if let Some(raw) = ship_ref {
            // Validate before transitioning so a bad ref doesn't half-apply.
            self.normalize_ref(raw)?;
        }
        self.set_status(id, ChunkStatus::Done)?;
        if let Some(raw) = ship_ref {
            self.push_cross_ref(id, raw)?;
        }
        let idx = self.index_of(id)?;
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkCompleted { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn link_chunk(&mut self, id: &str, cross_ref: &str) -> Result<&Chunk, String> {
        self.push_cross_ref(id, cross_ref)?;
        let idx = self.index_of(id)?;
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn set_blocked_by(&mut self, id: &str, blocked_by: BlockedBy) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        let c = &mut self.roadmap.chunks[idx];
        c.blocked_by = Some(blocked_by);
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn clear_blocked_by(&mut self, id: &str) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        if self.roadmap.chunks[idx].blocked_by.take().is_none() {
            return Err(format!("chunk '{id}' has no blocker to clear"));
        }
        let c = &mut self.roadmap.chunks[idx];
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }
}
