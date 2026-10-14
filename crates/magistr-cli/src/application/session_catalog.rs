//! Transcript-free supervision facts and their coalesced cursor change together.
//! The registry publishes this projection; subscriptions never own providers.
use super::session_observation::{SessionSnapshot, SessionStatus};
use magistr_application::sessions::SessionActivity;
use magistr_cockpit::cockpit::Msg;
use magistr_domain::identity::SessionAddress;
use tokio::sync::watch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingRequest {
    pub id: u64,
    pub title: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SessionSummary {
    pub address: SessionAddress,
    pub name: String,
    pub status: SessionStatus,
    pub closed: bool,
    pub model: Option<String>,
    pub usage: Option<Msg>,
    pub requests: Vec<WaitingRequest>,
    pub activity: Option<SessionActivity>,
}
impl SessionSummary {
    pub(super) fn from_snapshot(snapshot: &SessionSnapshot) -> Self {
        Self {
            address: snapshot.address.clone(),
            name: snapshot.seed.name.clone(),
            status: snapshot.status,
            closed: snapshot.closed,
            model: snapshot.model.clone(),
            usage: snapshot.latest_usage().cloned(),
            requests: snapshot
                .pending_requests()
                .filter_map(|message| {
                    let Msg::Agent(magistr_application::sessions::AgentEvent::PermissionRequest {
                        id,
                        title,
                        ..
                    }) = message
                    else {
                        return None;
                    };
                    Some(WaitingRequest {
                        id: *id,
                        title: title.chars().take(256).collect(),
                    })
                })
                .collect(),
            activity: snapshot.activity.clone(),
        }
    }
    #[must_use]
    pub fn label(&self) -> &'static str {
        if self.closed {
            "Closing"
        } else if self
            .activity
            .as_ref()
            .is_some_and(|activity| activity.cancellation_requested)
        {
            "Stopping"
        } else if !self.requests.is_empty() {
            "Waiting for you"
        } else {
            self.status.label()
        }
    }
}
impl From<&SessionSnapshot> for SessionSummary {
    fn from(snapshot: &SessionSnapshot) -> Self {
        Self::from_snapshot(snapshot)
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionCatalog {
    pub sequence: u64,
    pub sessions: Vec<SessionSummary>,
}
pub struct CatalogSubscription {
    pub(super) receiver: watch::Receiver<SessionCatalog>,
}
impl CatalogSubscription {
    pub fn snapshot(&mut self) -> SessionCatalog {
        self.receiver.borrow_and_update().clone()
    }
    pub async fn changed(&mut self) -> Option<SessionCatalog> {
        self.receiver.changed().await.ok()?;
        Some(self.snapshot())
    }
}
#[cfg(test)]
#[path = "session_catalog.tests.rs"]
mod tests;
