//! Retained source identity and full revisions travel together. Stream chunks and
//! tool updates amend their original record, so reseeding never reassembles a
//! conversation from an arbitrary suffix of transport deltas.
use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::Msg;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StreamKind {
    Text,
    Thought,
    User,
}
impl StreamKind {
    pub(super) fn of(message: &Msg) -> Option<Self> {
        match message {
            Msg::Agent(AgentEvent::Text(_)) => Some(Self::Text),
            Msg::Agent(AgentEvent::Thought(_)) => Some(Self::Thought),
            Msg::Agent(AgentEvent::UserText(_)) => Some(Self::User),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RetainedObservation {
    pub id: u64,
    pub sequence: u64,
    pub message: Msg,
}

impl RetainedObservation {
    pub(super) fn new(sequence: u64, message: Msg) -> Self {
        let message = match message {
            Msg::Agent(AgentEvent::ToolCall {
                id,
                title,
                verb,
                status,
                diff,
                images,
                mut record,
            }) => {
                record.kind = Some(verb);
                Msg::Agent(AgentEvent::ToolCallUpdate {
                    id,
                    title: Some(title),
                    status: Some(status),
                    diff,
                    duration_ms: None,
                    images,
                    record,
                })
            }
            message => message,
        };
        Self {
            id: sequence,
            sequence,
            message,
        }
    }

    pub(super) fn tool_id(&self) -> Option<&str> {
        match &self.message {
            Msg::Agent(AgentEvent::ToolCallUpdate { id, .. }) => Some(id),
            _ => None,
        }
    }

    /// Returns false when this observation starts a different source record.
    pub(super) fn append(&mut self, sequence: u64, message: &Msg) -> bool {
        match (&mut self.message, message) {
            (Msg::Agent(AgentEvent::Text(source)), Msg::Agent(AgentEvent::Text(delta)))
            | (Msg::Agent(AgentEvent::Thought(source)), Msg::Agent(AgentEvent::Thought(delta)))
            | (Msg::Agent(AgentEvent::UserText(source)), Msg::Agent(AgentEvent::UserText(delta))) => {
                source.push_str(delta)
            }
            _ => return false,
        }
        self.sequence = sequence;
        true
    }

    pub(super) fn update_tool(&mut self, sequence: u64, message: Msg) {
        let Msg::Agent(AgentEvent::ToolCallUpdate {
            title,
            status,
            diff,
            duration_ms,
            images,
            record,
            ..
        }) = &mut self.message
        else {
            return;
        };
        let Msg::Agent(AgentEvent::ToolCallUpdate {
            title: next_title,
            status: next_status,
            diff: next_diff,
            duration_ms: next_duration,
            images: next_images,
            record: next_record,
            ..
        }) = message
        else {
            return;
        };
        if next_title.is_some() {
            *title = next_title;
        }
        if next_status.is_some() {
            *status = next_status;
        }
        if next_diff.is_some() {
            *diff = next_diff;
        }
        if next_duration.is_some() {
            *duration_ms = next_duration;
        }
        if !next_images.is_empty() {
            *images = next_images;
        }
        record.absorb(*next_record);
        self.sequence = sequence;
    }
}

#[cfg(test)]
#[path = "retained_observation.tests.rs"]
mod tests;
