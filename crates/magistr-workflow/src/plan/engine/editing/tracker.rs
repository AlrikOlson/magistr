use super::*;

impl Edit<'_> {
    #[allow(clippy::too_many_arguments)]
    pub fn record_tracker_link(
        &mut self,
        task_id: &str,
        provider: &str,
        external_id: &str,
        content_hash: &str,
        version: Option<&str>,
    ) -> Result<&TrackerLink, String> {
        // Validate the task exists before mutating anything.
        self.index_of(task_id)?;
        let provider = provider.trim().to_ascii_lowercase();
        let external_id = external_id.trim().to_string();
        if provider.is_empty() || external_id.is_empty() {
            return Err("tracker link needs a provider and an external id".to_string());
        }
        let now = Self::now();

        match self
            .plan
            .links
            .iter_mut()
            .find(|l| l.task_id == task_id && l.provider == provider)
        {
            Some(link) => {
                link.external_id = external_id.clone();
                link.our_last_write_hash = content_hash.to_string();
                link.last_seen_version = version.map(str::to_string);
                link.updated_at = now;
            }
            None => self.plan.links.push(TrackerLink {
                task_id: task_id.to_string(),
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
        self.push_cross_ref(task_id, &wire)?;
        self.touch_task_for_link(task_id)?;

        let idx = self.index_of(task_id)?;
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });

        self.tracker_link(task_id, &provider)
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
            .plan
            .container_links
            .iter_mut()
            .find(|c| c.kind == kind && c.name == name && c.provider == provider)
        {
            Some(link) => {
                link.external_id = external_id;
                link.created_by_us = link.created_by_us || created_by_us;
                link.updated_at = now;
            }
            None => self.plan.container_links.push(ContainerLink {
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
        task_id: &str,
        provider: &str,
        relations_hash: &str,
    ) -> Result<&TrackerLink, String> {
        self.index_of(task_id)?;
        let provider = provider.trim().to_ascii_lowercase();
        let now = Self::now();

        let Some(link) = self
            .plan
            .links
            .iter_mut()
            .find(|l| l.task_id == task_id && l.provider == provider)
        else {
            return Err(format!(
                "no tracker link for task '{task_id}' on provider '{provider}' — \
                 relations are declared between items that already exist"
            ));
        };
        link.our_last_relations_hash = Some(relations_hash.to_string());
        link.updated_at = now;

        self.touch_task_for_link(task_id)?;

        let idx = self.index_of(task_id)?;
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });

        self.tracker_link(task_id, &provider)
            .ok_or_else(|| "tracker link vanished after write".to_string())
    }

    pub fn record_tracker_authored(
        &mut self,
        task_id: &str,
        provider: &str,
        authored_hash: &str,
    ) -> Result<&TrackerLink, String> {
        self.index_of(task_id)?;
        let provider = provider.trim().to_ascii_lowercase();
        let now = Self::now();

        let Some(link) = self
            .plan
            .links
            .iter_mut()
            .find(|l| l.task_id == task_id && l.provider == provider)
        else {
            return Err(format!(
                "no tracker link for task '{task_id}' on provider '{provider}' — \
                 the authored digest describes a write, which creates the link first"
            ));
        };
        link.our_last_authored_hash = Some(authored_hash.to_string());
        link.updated_at = now;

        self.touch_task_for_link(task_id)?;

        let idx = self.index_of(task_id)?;
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });

        self.tracker_link(task_id, &provider)
            .ok_or_else(|| "tracker link vanished after write".to_string())
    }

    pub fn set_tracker_opt_in(
        &mut self,
        task_id: &str,
        provider: &str,
        enabled: bool,
    ) -> Result<&TrackerOptIn, String> {
        self.index_of(task_id)?;
        let provider = provider.trim().to_ascii_lowercase();
        if provider.is_empty() {
            return Err("tracker opt-in needs a provider".to_string());
        }
        let now = Self::now();

        match self
            .plan
            .tracker_opt_ins
            .iter_mut()
            .find(|o| o.task_id == task_id && o.provider == provider)
        {
            Some(opt_in) => {
                opt_in.enabled = enabled;
                opt_in.updated_at = now;
            }
            None => self.plan.tracker_opt_ins.push(TrackerOptIn {
                task_id: task_id.to_string(),
                provider: provider.clone(),
                enabled,
                updated_at: now,
            }),
        }

        self.touch_task_for_link(task_id)?;

        let idx = self.index_of(task_id)?;
        let snapshot = self.plan.tasks[idx].clone();
        self.record_event(PlanFrame::TaskChanged { task: snapshot });

        self.tracker_opt_in(task_id, &provider)
            .ok_or_else(|| "tracker opt-in vanished after write".to_string())
    }

    pub fn adopt_tracker_state(
        &mut self,
        links: Vec<TrackerLink>,
        opt_ins: Vec<TrackerOptIn>,
    ) -> Result<usize, String> {
        for task_id in links
            .iter()
            .map(|link| &link.task_id)
            .chain(opt_ins.iter().map(|opt_in| &opt_in.task_id))
        {
            self.index_of(task_id)?;
        }
        let mut adopted = 0;
        for link in links {
            match self.plan.links.iter_mut().find(|l| l.key() == link.key()) {
                Some(existing) => {
                    if crate::plan::domain::rfc3339_newer(&link.updated_at, &existing.updated_at) {
                        *existing = link;
                        adopted += 1;
                    }
                }
                None => {
                    self.plan.links.push(link);
                    adopted += 1;
                }
            }
        }
        for opt_in in opt_ins {
            match self
                .plan
                .tracker_opt_ins
                .iter_mut()
                .find(|o| o.key() == opt_in.key())
            {
                Some(existing) => {
                    if crate::plan::domain::rfc3339_newer(&opt_in.updated_at, &existing.updated_at)
                    {
                        *existing = opt_in;
                        adopted += 1;
                    }
                }
                None => {
                    self.plan.tracker_opt_ins.push(opt_in);
                    adopted += 1;
                }
            }
        }
        Ok(adopted)
    }

    fn touch_task_for_link(&mut self, task_id: &str) -> Result<(), String> {
        let idx = self.index_of(task_id)?;
        self.plan.tasks[idx].updated_at = Self::now();
        Ok(())
    }

    pub fn tracker_link(&self, task_id: &str, provider: &str) -> Option<&TrackerLink> {
        let provider = provider.trim().to_ascii_lowercase();
        self.plan
            .links
            .iter()
            .find(|l| l.task_id == task_id && l.provider == provider)
    }

    pub fn tracker_opt_in(&self, task_id: &str, provider: &str) -> Option<&TrackerOptIn> {
        let provider = provider.trim().to_ascii_lowercase();
        self.plan
            .tracker_opt_ins
            .iter()
            .find(|o| o.task_id == task_id && o.provider == provider)
    }
}
