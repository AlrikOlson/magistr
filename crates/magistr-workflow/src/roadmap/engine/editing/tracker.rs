use super::*;

impl Edit<'_> {
    #[allow(clippy::too_many_arguments)]
    pub fn record_tracker_link(
        &mut self,
        chunk_id: &str,
        provider: &str,
        external_id: &str,
        content_hash: &str,
        version: Option<&str>,
    ) -> Result<&TrackerLink, String> {
        // Validate the chunk exists before mutating anything.
        self.index_of(chunk_id)?;
        let provider = provider.trim().to_ascii_lowercase();
        let external_id = external_id.trim().to_string();
        if provider.is_empty() || external_id.is_empty() {
            return Err("tracker link needs a provider and an external id".to_string());
        }
        let now = Self::now();

        match self
            .roadmap
            .links
            .iter_mut()
            .find(|l| l.chunk_id == chunk_id && l.provider == provider)
        {
            Some(link) => {
                link.external_id = external_id.clone();
                link.our_last_write_hash = content_hash.to_string();
                link.last_seen_version = version.map(str::to_string);
                link.updated_at = now;
            }
            None => self.roadmap.links.push(TrackerLink {
                chunk_id: chunk_id.to_string(),
                provider: provider.clone(),
                external_id: external_id.clone(),
                our_last_write_hash: content_hash.to_string(),
                last_seen_version: version.map(str::to_string),
                our_last_relations_hash: None,
                our_last_authored_hash: None,
                created_at: now.clone(),
                updated_at: now,
            }),
        }

        // Idempotent: push_cross_ref dedupes, so re-linking the same twin does
        // not accumulate refs.
        let wire = CrossRef::external(&provider, &external_id).to_wire();
        self.push_cross_ref(chunk_id, &wire)?;
        self.touch_chunk_for_link(chunk_id)?;

        let idx = self.index_of(chunk_id)?;
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });

        self.tracker_link(chunk_id, &provider)
            .ok_or_else(|| "tracker link vanished after write".to_string())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_container_link(
        &mut self,
        kind: ContainerKind,
        name: &str,
        provider: &str,
        external_id: &str,
        created_by_us: bool,
    ) -> Result<(), String> {
        let provider = provider.trim().to_ascii_lowercase();
        let external_id = external_id.trim().to_string();
        if name.trim().is_empty() || provider.is_empty() || external_id.is_empty() {
            return Err("container link needs a name, a provider and an external id".to_string());
        }
        let now = Self::now();

        match self
            .roadmap
            .container_links
            .iter_mut()
            .find(|c| c.kind == kind && c.name == name && c.provider == provider)
        {
            Some(link) => {
                link.external_id = external_id;
                link.created_by_us = link.created_by_us || created_by_us;
                link.updated_at = now;
            }
            None => self.roadmap.container_links.push(ContainerLink {
                kind,
                name: name.to_string(),
                provider,
                external_id,
                created_by_us,
                created_at: now.clone(),
                updated_at: now,
            }),
        }
        Ok(())
    }

    pub fn record_tracker_relations(
        &mut self,
        chunk_id: &str,
        provider: &str,
        relations_hash: &str,
    ) -> Result<&TrackerLink, String> {
        self.index_of(chunk_id)?;
        let provider = provider.trim().to_ascii_lowercase();
        let now = Self::now();

        let Some(link) = self
            .roadmap
            .links
            .iter_mut()
            .find(|l| l.chunk_id == chunk_id && l.provider == provider)
        else {
            return Err(format!(
                "no tracker link for chunk '{chunk_id}' on provider '{provider}' — \
                 relations are declared between items that already exist"
            ));
        };
        link.our_last_relations_hash = Some(relations_hash.to_string());
        link.updated_at = now;

        self.touch_chunk_for_link(chunk_id)?;

        let idx = self.index_of(chunk_id)?;
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });

        self.tracker_link(chunk_id, &provider)
            .ok_or_else(|| "tracker link vanished after write".to_string())
    }

    pub fn record_tracker_authored(
        &mut self,
        chunk_id: &str,
        provider: &str,
        authored_hash: &str,
    ) -> Result<&TrackerLink, String> {
        self.index_of(chunk_id)?;
        let provider = provider.trim().to_ascii_lowercase();
        let now = Self::now();

        let Some(link) = self
            .roadmap
            .links
            .iter_mut()
            .find(|l| l.chunk_id == chunk_id && l.provider == provider)
        else {
            return Err(format!(
                "no tracker link for chunk '{chunk_id}' on provider '{provider}' — \
                 the authored digest describes a write, which creates the link first"
            ));
        };
        link.our_last_authored_hash = Some(authored_hash.to_string());
        link.updated_at = now;

        self.touch_chunk_for_link(chunk_id)?;

        let idx = self.index_of(chunk_id)?;
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });

        self.tracker_link(chunk_id, &provider)
            .ok_or_else(|| "tracker link vanished after write".to_string())
    }

    pub fn set_tracker_opt_in(
        &mut self,
        chunk_id: &str,
        provider: &str,
        enabled: bool,
    ) -> Result<&TrackerOptIn, String> {
        self.index_of(chunk_id)?;
        let provider = provider.trim().to_ascii_lowercase();
        if provider.is_empty() {
            return Err("tracker opt-in needs a provider".to_string());
        }
        let now = Self::now();

        match self
            .roadmap
            .tracker_opt_ins
            .iter_mut()
            .find(|o| o.chunk_id == chunk_id && o.provider == provider)
        {
            Some(opt_in) => {
                opt_in.enabled = enabled;
                opt_in.updated_at = now;
            }
            None => self.roadmap.tracker_opt_ins.push(TrackerOptIn {
                chunk_id: chunk_id.to_string(),
                provider: provider.clone(),
                enabled,
                updated_at: now,
            }),
        }

        self.touch_chunk_for_link(chunk_id)?;

        let idx = self.index_of(chunk_id)?;
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });

        self.tracker_opt_in(chunk_id, &provider)
            .ok_or_else(|| "tracker opt-in vanished after write".to_string())
    }

    pub fn adopt_tracker_state(
        &mut self,
        links: Vec<TrackerLink>,
        opt_ins: Vec<TrackerOptIn>,
    ) -> Result<usize, String> {
        for chunk_id in links
            .iter()
            .map(|link| &link.chunk_id)
            .chain(opt_ins.iter().map(|opt_in| &opt_in.chunk_id))
        {
            self.index_of(chunk_id)?;
        }
        let mut adopted = 0;
        for link in links {
            match self
                .roadmap
                .links
                .iter_mut()
                .find(|l| l.key() == link.key())
            {
                Some(existing) => {
                    if crate::roadmap::domain::rfc3339_newer(&link.updated_at, &existing.updated_at)
                    {
                        *existing = link;
                        adopted += 1;
                    }
                }
                None => {
                    self.roadmap.links.push(link);
                    adopted += 1;
                }
            }
        }
        for opt_in in opt_ins {
            match self
                .roadmap
                .tracker_opt_ins
                .iter_mut()
                .find(|o| o.key() == opt_in.key())
            {
                Some(existing) => {
                    if crate::roadmap::domain::rfc3339_newer(
                        &opt_in.updated_at,
                        &existing.updated_at,
                    ) {
                        *existing = opt_in;
                        adopted += 1;
                    }
                }
                None => {
                    self.roadmap.tracker_opt_ins.push(opt_in);
                    adopted += 1;
                }
            }
        }
        Ok(adopted)
    }

    fn touch_chunk_for_link(&mut self, chunk_id: &str) -> Result<(), String> {
        let idx = self.index_of(chunk_id)?;
        self.roadmap.chunks[idx].updated_at = Self::now();
        Ok(())
    }

    pub fn tracker_link(&self, chunk_id: &str, provider: &str) -> Option<&TrackerLink> {
        let provider = provider.trim().to_ascii_lowercase();
        self.roadmap
            .links
            .iter()
            .find(|l| l.chunk_id == chunk_id && l.provider == provider)
    }

    pub fn tracker_opt_in(&self, chunk_id: &str, provider: &str) -> Option<&TrackerOptIn> {
        let provider = provider.trim().to_ascii_lowercase();
        self.roadmap
            .tracker_opt_ins
            .iter()
            .find(|o| o.chunk_id == chunk_id && o.provider == provider)
    }
}
