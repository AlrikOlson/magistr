//! Client lifetime and reconnection shared by both MCP surfaces.

#[cfg(test)]
#[path = "bridge.tests.rs"]
mod tests;

use std::time::Duration;

use color_eyre::eyre;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// How long the owner gets to answer what was in flight when the client
/// leaves; after that the link is closed regardless.
const DRAIN_DEADLINE: Duration = Duration::from_secs(5);

use super::{BridgeLink, ClientSession, Inbound, SessionPolicy};

pub(crate) struct Bridge<L: BridgeLink, P: SessionPolicy> {
    options: L::Options,
    session: ClientSession<P>,
    link: Option<L>,
    /// Why the last reconnect did not reach an owner.
    unreachable: String,
}

impl<L: BridgeLink, P: SessionPolicy> Bridge<L, P> {
    /// A bridge over the owner startup reached, or — when startup was
    /// refused for a reason a later request can outlive — over none yet,
    /// with that refusal as the reason every local answer names.
    pub(crate) fn new(
        options: L::Options,
        identity: String,
        policy: P,
        link: Result<L, String>,
    ) -> Self {
        let (link, unreachable) = match link {
            Ok(link) => {
                tracing::info!(
                    generation = link.generation(),
                    embedded = link.is_embedded(),
                    identity = identity.as_str(),
                    "MCP bridge linked"
                );
                (Some(link), String::new())
            }
            Err(reason) => {
                tracing::warn!(
                    reason = reason.as_str(),
                    identity = identity.as_str(),
                    "MCP bridge started without an owner; discovery is served locally and the next request reconnects"
                );
                (None, reason)
            }
        };
        Self {
            options,
            session: ClientSession::new(identity, policy),
            link,
            unreachable,
        }
    }

    /// Relay until the client closes its side. An embedded owner is then shut
    /// down in order; an attached socket is dropped.
    /// Serve an internal client with the same admission, session and recovery rules.
    pub(crate) async fn run_transport(
        mut self,
        stdin: impl tokio::io::AsyncRead + Unpin,
        mut stdout: impl tokio::io::AsyncWrite + Unpin,
    ) -> eyre::Result<()> {
        let mut client = BufReader::new(stdin).lines();
        loop {
            tokio::select! {
                biased;
                line = client.next_line() => match line? {
                    Some(line) => self.on_client(&line, &mut stdout).await?,
                    None => {
                        self.client_closed(&mut stdout).await?;
                        break;
                    }
                },
                line = owner_line(self.link.as_mut()), if self.link.is_some() => match line {
                    Some(line) => self.on_owner(&line, &mut stdout).await?,
                    None => self.owner_lost(&mut stdout).await?,
                },
            }
        }
        match self.link.take() {
            Some(link) => link.close().await,
            None => Ok(()),
        }
    }

    async fn on_client(
        &mut self,
        line: &str,
        stdout: &mut (impl AsyncWriteExt + Unpin),
    ) -> eyre::Result<()> {
        if self.link.is_none() && wants_answer(line) {
            self.relink(stdout).await?;
        }
        match self.session.inbound(line, self.link.is_some()) {
            Inbound::Forward(frame) => {
                let Some(link) = self.link.as_mut() else {
                    return Ok(());
                };
                if link.send(&frame).await.is_err() {
                    self.owner_lost(stdout).await?;
                }
            }
            Inbound::Unanswered(request) => {
                let answer = self.session.answer_unavailable(&request, &self.unreachable);
                write_line(stdout, &answer.to_string()).await?;
            }
            Inbound::Dropped => {}
        }
        Ok(())
    }

    async fn on_owner(
        &mut self,
        line: &str,
        stdout: &mut (impl AsyncWriteExt + Unpin),
    ) -> eyre::Result<()> {
        self.session.outbound(line);
        write_line(stdout, line).await
    }

    /// The client closed its side, possibly with answers still on their way:
    /// a client that sends its last request and leaves at once still gets
    /// that answer, as it would from a server on its own stdio. The owner is
    /// told EOF, its remaining lines are relayed, then the link is closed.
    async fn client_closed(
        &mut self,
        stdout: &mut (impl AsyncWriteExt + Unpin),
    ) -> eyre::Result<()> {
        let Some(mut link) = self.link.take() else {
            return Ok(());
        };
        let owner_sees_eof = link.half_close().await;
        let drained = tokio::time::timeout(DRAIN_DEADLINE, async {
            // Without EOF the owner never closes its side: stop once every
            // request the client sent has its answer.
            while owner_sees_eof || self.session.has_outstanding() {
                let Some(line) = link.next_line().await else {
                    break;
                };
                self.on_owner(&line, stdout).await?;
            }
            Ok::<(), eyre::Report>(())
        })
        .await;
        if let Ok(relayed) = drained {
            relayed?;
        }
        link.close().await
    }

    /// The owner behind an attached link is gone. An embedded owner cannot
    /// go away underneath its own bridge; that is a fault.
    async fn owner_lost(&mut self, stdout: &mut (impl AsyncWriteExt + Unpin)) -> eyre::Result<()> {
        let Some(link) = self.link.take() else {
            return Ok(());
        };
        if link.is_embedded() {
            link.close().await?;
            eyre::bail!("The Magistr connection ended unexpectedly.");
        }
        drop(link);
        self.unreachable = "the owner ended its session".to_owned();
        tracing::warn!("MCP owner lost; in-flight calls refused, next call reconnects");
        for answer in self.session.owner_lost(&self.unreachable) {
            write_line(stdout, &answer.to_string()).await?;
        }
        Ok(())
    }

    /// One bounded attempt to reach an owner again: attach to whoever admitted
    /// the workspace meanwhile, or admit it here. On success the recorded
    /// `initialize` is replayed so the new owner knows this client.
    async fn relink(&mut self, stdout: &mut (impl AsyncWriteExt + Unpin)) -> eyre::Result<()> {
        let mut link = match L::open(&self.options).await {
            Ok(link) => link,
            Err(error) => {
                self.unreachable = error.to_string();
                tracing::warn!(error = %self.unreachable, "MCP owner still unreachable");
                return Ok(());
            }
        };
        if self.session.initialized()
            && let Some((id, initialize, initialized)) = self.session.replay()
        {
            match link.replay(&id, &initialize, &initialized).await {
                Ok(passthrough) => {
                    for line in passthrough {
                        self.on_owner(&line, stdout).await?;
                    }
                }
                Err(error) => {
                    self.unreachable = error.to_string();
                    tracing::warn!(error = %self.unreachable, "MCP owner did not accept the replay");
                    link.close().await?;
                    return Ok(());
                }
            }
        }
        tracing::info!(
            generation = link.generation(),
            embedded = link.is_embedded(),
            identity = self.session.identity(),
            "MCP bridge relinked"
        );
        self.link = Some(link);
        if let Some(notification) = self.session.tools_changed() {
            write_line(stdout, &notification.to_string()).await?;
        }
        Ok(())
    }
}

async fn owner_line<L: BridgeLink>(link: Option<&mut L>) -> Option<String> {
    match link {
        Some(link) => link.next_line().await,
        None => std::future::pending().await,
    }
}

/// Whether a client line is a request (carries an id and a method): the only
/// kind worth reconnecting for.
fn wants_answer(line: &str) -> bool {
    serde_json::from_str::<Value>(line).is_ok_and(|message| {
        message.get("method").is_some() && message.get("id").is_some_and(|id| !id.is_null())
    })
}

async fn write_line(stdout: &mut (impl AsyncWriteExt + Unpin), line: &str) -> eyre::Result<()> {
    stdout.write_all(line.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

#[cfg(unix)]
pub(crate) fn stdio() -> eyre::Result<(
    crate::mcp_tools::owner_stdio::ProxyStdio,
    crate::mcp_tools::owner_stdio::ProxyStdio,
)> {
    use crate::mcp_tools::owner_stdio::ProxyStdio;
    Ok((ProxyStdio::stdin()?, ProxyStdio::stdout()?))
}

#[cfg(not(unix))]
pub(crate) fn stdio() -> eyre::Result<(tokio::io::Stdin, tokio::io::Stdout)> {
    Ok((tokio::io::stdin(), tokio::io::stdout()))
}
