//! Pure candidate edits; persistence and publishers belong to the engine.
use super::*;
use uuid::Uuid;

pub(crate) struct Edit<'a> {
    pub(super) signals: &'a mut Signals,
    pub(super) project_id: String,
    pub(super) events: Vec<SignalFrame>,
}

impl Edit<'_> {
    fn record_event(&mut self, frame: SignalFrame) {
        self.events.push(frame);
    }
    pub fn upsert_signal(&mut self, signal: Signal) -> Result<bool, String> {
        if signal.id.trim().is_empty()
            || signal.project_id.as_deref() != Some(self.project_id.as_str())
        {
            return Err("The signal is incomplete or belongs to another project.".into());
        }
        match self.index_of(&signal.id) {
            Ok(idx) => {
                if !crate::signal::domain::signal_wins(&signal, &self.signals.signals[idx]) {
                    return Ok(false);
                }
                self.signals.signals[idx] = signal;
            }
            Err(_) => self.signals.signals.push(signal),
        }
        Ok(true)
    }

    pub fn capture(
        &mut self,
        kind: SignalKind,
        from: String,
        body: String,
    ) -> Result<&Signal, String> {
        self.capture_with_content(kind, from, body, None)
    }

    pub fn capture_with_content(
        &mut self,
        kind: SignalKind,
        from: String,
        body: String,
        content: Option<crate::content::StructuredContent>,
    ) -> Result<&Signal, String> {
        if body.trim().is_empty() {
            return Err("signal body is empty".into());
        }
        if let Some(content) = &content {
            content.validate()?;
        }
        let now = Self::now();
        self.signals.signals.push(Signal {
            id: Uuid::new_v4().to_string(),
            kind,
            from,
            body,
            content,
            created: now,
            status: SignalStatus::New,
            enrichment: Vec::new(),
            cross_refs: Vec::new(),
            surfaced_at: None,
            snooze_until: None,
            // Stamp origin at the record, so a later prune can prove ownership
            // rather than guess it (store-prune-think-signal).
            project_id: Some(self.project_id.clone()),
        });
        let snapshot = self.signals.signals.last().unwrap().clone();
        self.record_event(SignalFrame::SignalCaptured { signal: snapshot });
        Ok(self.signals.signals.last().unwrap())
    }

    pub fn set_status(&mut self, id: &str, to: SignalStatus) -> Result<&Signal, String> {
        let idx = self.index_of(id)?;
        let from = self.signals.signals[idx].status;
        if !from.allows(to) {
            return Err(format!(
                "illegal transition for signal '{id}': {from:?} -> {to:?}"
            ));
        }
        self.signals.signals[idx].status = to;
        self.record_event(SignalFrame::SignalChanged {
            signal: self.signals.signals[idx].clone(),
        });
        Ok(&self.signals.signals[idx])
    }

    pub fn enrich(&mut self, id: &str, enrichment: Enrichment) -> Result<&Signal, String> {
        let idx = self.index_of(id)?;
        self.signals.signals[idx].enrichment.push(enrichment);
        self.record_event(SignalFrame::SignalChanged {
            signal: self.signals.signals[idx].clone(),
        });
        Ok(&self.signals.signals[idx])
    }

    pub fn research(
        &mut self,
        id: &str,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        think_step: Option<u32>,
    ) -> Result<&Signal, String> {
        if summary.trim().is_empty() {
            return Err("research summary is empty".into());
        }
        let idx = self.index_of(id)?;
        let from = self.signals.signals[idx].status;
        if matches!(from, SignalStatus::Promoted | SignalStatus::Dismissed) {
            return Err(format!(
                "signal '{id}' cannot be researched from {from:?} (terminal state)"
            ));
        }

        // Append the enrichment, time-stamped by the engine.
        self.signals.signals[idx].enrichment.push(Enrichment {
            think_step,
            sources,
            summary,
            confidence: confidence.clamp(0.0, 1.0),
            at: Self::now(),
        });

        // Advance toward Researched without ever moving backward.
        if matches!(from, SignalStatus::New | SignalStatus::Triaged) {
            self.signals.signals[idx].status = SignalStatus::Researched;
        }

        // Cross-ref the motivating think step (deduped, validated wire form).
        if let Some(n) = think_step {
            let wire = crate::infra::CrossRef::ThinkStep(n).to_wire();
            if !self.signals.signals[idx].cross_refs.contains(&wire) {
                self.signals.signals[idx].cross_refs.push(wire);
            }
        }
        let snapshot = self.signals.signals[idx].clone();
        self.record_event(SignalFrame::SignalChanged { signal: snapshot });
        Ok(&self.signals.signals[idx])
    }

    pub fn link(&mut self, id: &str, cross_ref: &str) -> Result<&Signal, String> {
        let normalized = crate::infra::CrossRef::from_wire(cross_ref)
            .map(|r| r.to_wire())
            .map_err(|e| format!("invalid cross-ref '{cross_ref}': {e}"))?;
        let idx = self.index_of(id)?;
        if !self.signals.signals[idx].cross_refs.contains(&normalized) {
            self.signals.signals[idx].cross_refs.push(normalized);
            let snapshot = self.signals.signals[idx].clone();
            self.record_event(SignalFrame::SignalChanged { signal: snapshot });
        }
        Ok(&self.signals.signals[idx])
    }

    pub fn promote(&mut self, id: &str) -> Result<&Signal, String> {
        let idx = self.index_of(id)?;
        let from = self.signals.signals[idx].status;
        if !matches!(from, SignalStatus::Researched | SignalStatus::Surfaced) {
            return Err(format!(
                "signal '{id}' cannot be promoted from {from:?} (must be researched or surfaced)"
            ));
        }
        self.signals.signals[idx].status = SignalStatus::Promoted;
        let snapshot = self.signals.signals[idx].clone();
        self.record_event(SignalFrame::SignalChanged { signal: snapshot });
        Ok(&self.signals.signals[idx])
    }

    pub fn surface(&mut self, id: &str) -> Result<&Signal, String> {
        let idx = self.index_of(id)?;
        let from = self.signals.signals[idx].status;
        if from != SignalStatus::Researched {
            return Err(format!(
                "signal '{id}' cannot be surfaced from {from:?} (must be researched)"
            ));
        }
        self.signals.signals[idx].status = SignalStatus::Surfaced;
        self.signals.signals[idx].surfaced_at = Some(Self::now());
        let snapshot = self.signals.signals[idx].clone();
        self.record_event(SignalFrame::SignalChanged { signal: snapshot });
        Ok(&self.signals.signals[idx])
    }

    pub fn snooze(&mut self, id: &str, minutes: i64) -> Result<&Signal, String> {
        let idx = self.index_of(id)?;
        let until = Utc::now() + chrono::Duration::minutes(minutes.max(0));
        self.signals.signals[idx].snooze_until = Some(until.to_rfc3339());
        let snapshot = self.signals.signals[idx].clone();
        self.record_event(SignalFrame::SignalChanged { signal: snapshot });
        Ok(&self.signals.signals[idx])
    }

    pub fn ignore(&mut self, id: &str) -> Result<&Signal, String> {
        self.set_status(id, SignalStatus::Dismissed)?;
        let idx = self.index_of(id)?;
        let snapshot = self.signals.signals[idx].clone();
        self.record_event(SignalFrame::SignalDismissed { signal: snapshot });
        Ok(&self.signals.signals[idx])
    }

    fn now() -> String {
        Utc::now().to_rfc3339()
    }

    fn index_of(&self, id: &str) -> Result<usize, String> {
        self.signals
            .signals
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| format!("signal '{id}' not found"))
    }

    pub fn get(&self, id: &str) -> Result<&Signal, String> {
        let idx = self.index_of(id)?;
        Ok(&self.signals.signals[idx])
    }
}
