//! Window subscriptions detach independently of resident sessions; one in-flight batch bounds IPC.
use crate::{
    credit::Credit,
    wire::{CatalogFrame, Frame},
};
use magistr::application::{SessionClient, SessionSubscription};
use magistr_domain::identity::SessionAddress;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tauri::ipc::Channel;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

struct Active {
    id: u64,
    selection: u64,
    stop: CancellationToken,
    credit: Arc<Credit>,
}
#[derive(Default)]
pub(crate) struct Streams {
    active: Mutex<Option<Active>>,
    catalog: Mutex<Option<Active>>,
    next: AtomicU64,
}
impl Streams {
    pub(crate) async fn subscribe(
        &self,
        client: &SessionClient,
        address: &SessionAddress,
        channel: Channel<Frame>,
        selection: u64,
    ) -> Result<u64, String> {
        let mut active = self.active.lock().await;
        if active
            .as_ref()
            .is_some_and(|current| selection <= current.selection)
        {
            return Err("A newer session was selected.".into());
        }
        let subscription = client.subscribe(address).await?;
        if let Some(previous) = active.take() {
            previous.stop.cancel();
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let stop = CancellationToken::new();
        let credit = Arc::new(Credit::default());
        tokio::spawn(forward(
            id,
            subscription,
            channel,
            stop.clone(),
            Arc::clone(&credit),
        ));
        *active = Some(Active {
            id,
            selection,
            stop,
            credit,
        });
        Ok(id)
    }
    pub(crate) async fn subscribe_catalog(
        &self,
        client: &SessionClient,
        channel: Channel<CatalogFrame>,
    ) -> Result<u64, String> {
        let mut active = self.catalog.lock().await;
        if let Some(previous) = active.take() {
            previous.stop.cancel();
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let stop = CancellationToken::new();
        let credit = Arc::new(Credit::default());
        tokio::spawn(forward_catalog(
            id,
            client.catalog(),
            channel,
            stop.clone(),
            Arc::clone(&credit),
        ));
        *active = Some(Active {
            id,
            selection: 0,
            stop,
            credit,
        });
        Ok(id)
    }
    pub(crate) async fn acknowledge(&self, id: u64, sequence: u64) -> Result<(), String> {
        let active = self.active.lock().await;
        if let Some(active) = active.as_ref().filter(|active| active.id == id) {
            return active.credit.acknowledge(sequence);
        }
        let catalog = self.catalog.lock().await;
        catalog
            .as_ref()
            .filter(|active| active.id == id)
            .ok_or("This view is no longer open.")?
            .credit
            .acknowledge(sequence)
    }
}
impl Drop for Streams {
    fn drop(&mut self) {
        for active in [self.active.get_mut().take(), self.catalog.get_mut().take()]
            .into_iter()
            .flatten()
        {
            active.stop.cancel();
        }
    }
}
async fn forward(
    id: u64,
    mut subscription: SessionSubscription,
    channel: Channel<Frame>,
    stop: CancellationToken,
    credit: Arc<Credit>,
) {
    let mut snapshot = subscription.snapshot();
    let mut seen = None;
    loop {
        let frame = Frame::from_snapshot(id, &snapshot, seen);
        let sequence = frame.sequence;
        if !credit.offer(sequence) || channel.send(frame).is_err() {
            return;
        }
        // A stopped or stalled WebView never queues another payload.
        tokio::select! {
            biased;
            ()=stop.cancelled()=>return,
            _=tokio::time::sleep(Duration::from_secs(30))=>return,
            ()=credit.ready.notified()=>{}
        }
        seen = Some(sequence);
        if snapshot.closed {
            return;
        }
        tokio::select! {
            biased;
            ()=stop.cancelled()=>return,
            next=subscription.changed()=>if next.is_none() {return;}
        }
        tokio::select! {()=stop.cancelled()=>return,_=tokio::time::sleep(Duration::from_millis(16))=>{}}
        snapshot = subscription.snapshot();
        if snapshot.since(sequence).is_none() {
            match subscription.refresh().await {
                Ok(next) => snapshot = next,
                Err(_) => return,
            }
        }
    }
}
async fn forward_catalog(
    id: u64,
    mut subscription: magistr::application::CatalogSubscription,
    channel: Channel<CatalogFrame>,
    stop: CancellationToken,
    credit: Arc<Credit>,
) {
    loop {
        let catalog = subscription.snapshot();
        let frame = CatalogFrame::from_catalog(id, &catalog);
        if !credit.offer(frame.sequence) || channel.send(frame).is_err() {
            return;
        }
        tokio::select! {
            biased;
            () = stop.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(30)) => return,
            () = credit.ready.notified() => {}
        }
        tokio::select! {
            biased;
            () = stop.cancelled() => return,
            next = subscription.changed() => if next.is_none() { return; }
        }
        tokio::select! {
            () = stop.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
}
