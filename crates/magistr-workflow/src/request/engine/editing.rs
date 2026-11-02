//! Pure candidate edits; persistence and publishers belong to the engine.
use super::*;
use uuid::Uuid;

pub(crate) struct Edit<'a> {
    pub(super) requests: &'a mut Requests,
    pub(super) project_id: String,
    pub(super) events: Vec<RequestFrame>,
}

impl Edit<'_> {
    fn record_event(&mut self, frame: RequestFrame) {
        self.events.push(frame);
    }
    pub fn upsert_request(&mut self, request: Request) -> Result<bool, String> {
        if request.id.trim().is_empty()
            || request.project_id.as_deref() != Some(self.project_id.as_str())
        {
            return Err("The request is incomplete or belongs to another project.".into());
        }
        match self.index_of(&request.id) {
            Ok(idx) => {
                if !crate::request::domain::request_wins(&request, &self.requests.requests[idx]) {
                    return Ok(false);
                }
                self.requests.requests[idx] = request;
            }
            Err(_) => self.requests.requests.push(request),
        }
        Ok(true)
    }

    pub fn capture(
        &mut self,
        kind: RequestKind,
        from: String,
        body: String,
    ) -> Result<&Request, String> {
        self.capture_with_content(kind, from, body, None)
    }

    pub fn capture_with_content(
        &mut self,
        kind: RequestKind,
        from: String,
        body: String,
        content: Option<crate::content::StructuredContent>,
    ) -> Result<&Request, String> {
        if body.trim().is_empty() {
            return Err("request body is empty".into());
        }
        if let Some(content) = &content {
            content.validate()?;
        }
        let now = Self::now();
        self.requests.requests.push(Request {
            id: Uuid::new_v4().to_string(),
            kind,
            from,
            body,
            content,
            created: now,
            status: RequestStatus::New,
            enrichment: Vec::new(),
            refs: Vec::new(),
            raised_at: None,
            snooze_until: None,
            // Stamp origin at the record, so a later prune can prove ownership
            // rather than guess it (store-prune-decision-request).
            project_id: Some(self.project_id.clone()),
        });
        let snapshot = self.requests.requests.last().unwrap().clone();
        self.record_event(RequestFrame::RequestCaptured { request: snapshot });
        Ok(self.requests.requests.last().unwrap())
    }

    pub fn set_status(&mut self, id: &str, to: RequestStatus) -> Result<&Request, String> {
        let idx = self.index_of(id)?;
        let from = self.requests.requests[idx].status;
        if !from.allows(to) {
            return Err(format!(
                "illegal transition for request '{id}': {from:?} -> {to:?}"
            ));
        }
        self.requests.requests[idx].status = to;
        self.record_event(RequestFrame::RequestChanged {
            request: self.requests.requests[idx].clone(),
        });
        Ok(&self.requests.requests[idx])
    }

    pub fn enrich(&mut self, id: &str, enrichment: Enrichment) -> Result<&Request, String> {
        let idx = self.index_of(id)?;
        self.requests.requests[idx].enrichment.push(enrichment);
        self.record_event(RequestFrame::RequestChanged {
            request: self.requests.requests[idx].clone(),
        });
        Ok(&self.requests.requests[idx])
    }

    pub fn research(
        &mut self,
        id: &str,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        note: Option<u32>,
    ) -> Result<&Request, String> {
        if summary.trim().is_empty() {
            return Err("research summary is empty".into());
        }
        let idx = self.index_of(id)?;
        let from = self.requests.requests[idx].status;
        if matches!(from, RequestStatus::Promoted | RequestStatus::Dismissed) {
            return Err(format!(
                "request '{id}' cannot be researched from {from:?} (terminal state)"
            ));
        }

        // Append the enrichment, time-stamped by the engine.
        self.requests.requests[idx].enrichment.push(Enrichment {
            note,
            sources,
            summary,
            confidence: confidence.clamp(0.0, 1.0),
            at: Self::now(),
        });

        // Advance toward Researched without ever moving backward.
        if matches!(from, RequestStatus::New | RequestStatus::Triaged) {
            self.requests.requests[idx].status = RequestStatus::Researched;
        }

        // Cross-ref the motivating decision note (deduped, validated wire form).
        if let Some(n) = note {
            let wire = crate::infra::CrossRef::Note(n).to_wire();
            if !self.requests.requests[idx].refs.contains(&wire) {
                self.requests.requests[idx].refs.push(wire);
            }
        }
        let snapshot = self.requests.requests[idx].clone();
        self.record_event(RequestFrame::RequestChanged { request: snapshot });
        Ok(&self.requests.requests[idx])
    }

    pub fn link(&mut self, id: &str, cross_ref: &str) -> Result<&Request, String> {
        let normalized = crate::infra::CrossRef::from_wire(cross_ref)
            .map(|r| r.to_wire())
            .map_err(|e| format!("invalid cross-ref '{cross_ref}': {e}"))?;
        let idx = self.index_of(id)?;
        if !self.requests.requests[idx].refs.contains(&normalized) {
            self.requests.requests[idx].refs.push(normalized);
            let snapshot = self.requests.requests[idx].clone();
            self.record_event(RequestFrame::RequestChanged { request: snapshot });
        }
        Ok(&self.requests.requests[idx])
    }

    pub fn promote(&mut self, id: &str) -> Result<&Request, String> {
        let idx = self.index_of(id)?;
        let from = self.requests.requests[idx].status;
        if !matches!(from, RequestStatus::Researched | RequestStatus::Raised) {
            return Err(format!(
                "request '{id}' cannot be promoted from {from:?} (must be researched or raised)"
            ));
        }
        self.requests.requests[idx].status = RequestStatus::Promoted;
        let snapshot = self.requests.requests[idx].clone();
        self.record_event(RequestFrame::RequestChanged { request: snapshot });
        Ok(&self.requests.requests[idx])
    }

    pub fn raise(&mut self, id: &str) -> Result<&Request, String> {
        let idx = self.index_of(id)?;
        let from = self.requests.requests[idx].status;
        if from != RequestStatus::Researched {
            return Err(format!(
                "request '{id}' cannot be raised from {from:?} (must be researched)"
            ));
        }
        self.requests.requests[idx].status = RequestStatus::Raised;
        self.requests.requests[idx].raised_at = Some(Self::now());
        let snapshot = self.requests.requests[idx].clone();
        self.record_event(RequestFrame::RequestChanged { request: snapshot });
        Ok(&self.requests.requests[idx])
    }

    pub fn snooze(&mut self, id: &str, minutes: i64) -> Result<&Request, String> {
        let idx = self.index_of(id)?;
        let until = Utc::now() + chrono::Duration::minutes(minutes.max(0));
        self.requests.requests[idx].snooze_until = Some(until.to_rfc3339());
        let snapshot = self.requests.requests[idx].clone();
        self.record_event(RequestFrame::RequestChanged { request: snapshot });
        Ok(&self.requests.requests[idx])
    }

    pub fn dismiss(&mut self, id: &str) -> Result<&Request, String> {
        self.set_status(id, RequestStatus::Dismissed)?;
        let idx = self.index_of(id)?;
        let snapshot = self.requests.requests[idx].clone();
        self.record_event(RequestFrame::RequestDismissed { request: snapshot });
        Ok(&self.requests.requests[idx])
    }

    fn now() -> String {
        Utc::now().to_rfc3339()
    }

    fn index_of(&self, id: &str) -> Result<usize, String> {
        self.requests
            .requests
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| format!("request '{id}' not found"))
    }

    pub fn get(&self, id: &str) -> Result<&Request, String> {
        let idx = self.index_of(id)?;
        Ok(&self.requests.requests[idx])
    }
}
