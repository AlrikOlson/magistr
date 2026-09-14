//! Ollama over `/api/chat`, streamed as newline-delimited JSON.

use agent_client_protocol::schema::v1::StopReason;
use serde::{Deserialize, Serialize};

use crate::{AgentError, ChatApi, ChatMessage, ModelSpec, Pricing, Role, StreamEvent};

/// A local Ollama daemon: `POST {host}/api/chat` with `stream: true`, one
/// JSON object per line, `message.content` on each and the token counts
/// (`prompt_eval_count`, `eval_count`) on the `done` line. Free, so the
/// usage update's cost is zero.
///
/// The registry is the daemon's own: [`discover`](Self::discover) reads
/// `GET /api/tags` once, so the picker lists what is pulled and a daemon
/// that is not running is refused before the agent serves. `OLLAMA_HOST`
/// names the daemon (the variable Ollama itself uses), `localhost:11434`
/// unless set.
#[derive(Debug)]
pub struct Ollama {
    base_url: String,
    models: Vec<ModelSpec>,
}

impl Ollama {
    /// The provider's name on the command line.
    pub const NAME: &'static str = "ollama";
    /// The environment variable naming the daemon.
    pub const HOST_ENV: &'static str = "OLLAMA_HOST";
    /// Where the daemon listens unless `OLLAMA_HOST` says otherwise.
    pub const DEFAULT_HOST: &'static str = "http://localhost:11434";
    /// The context window reported for every model. `/api/tags` does not
    /// carry it, and the daemon's default `num_ctx` is what a chat runs in.
    pub const WINDOW: u64 = 4_096;

    /// Read the daemon's model list at `OLLAMA_HOST` (or the default).
    pub async fn discover() -> Result<Self, AgentError> {
        let host = std::env::var(Self::HOST_ENV)
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::DEFAULT_HOST.to_owned());
        Self::discover_at(host).await
    }

    /// Read the model list of the daemon at `host` (`host:port` or a URL).
    pub async fn discover_at(host: impl Into<String>) -> Result<Self, AgentError> {
        let base_url = normalize_host(&host.into());
        let tags: Tags = reqwest::Client::new()
            .get(format!("{base_url}/api/tags"))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| {
                AgentError::provider(format!(
                    "cannot reach Ollama at {base_url}: {e}; start it with `ollama serve`"
                ))
            })?
            .json()
            .await
            .map_err(|e| AgentError::provider(format!("ollama: cannot parse /api/tags: {e}")))?;
        if tags.models.is_empty() {
            return Err(AgentError::provider(format!(
                "Ollama at {base_url} has no models; pull one with `ollama pull llama3.1`"
            )));
        }
        let models = tags
            .models
            .into_iter()
            .map(|tag| ModelSpec::new(tag.name.clone(), tag.name, Self::WINDOW, Pricing::FREE))
            .collect();
        Ok(Self { base_url, models })
    }
}

/// `host:port` becomes `http://host:port`; a trailing slash goes.
fn normalize_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('/');
    if host.contains("://") {
        host.to_owned()
    } else {
        format!("http://{host}")
    }
}

#[derive(Deserialize)]
struct Tags {
    #[serde(default)]
    models: Vec<Tag>,
}

#[derive(Deserialize)]
struct Tag {
    name: String,
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Deserialize)]
struct Chunk {
    #[serde(default)]
    message: Option<ChunkMessage>,
    #[serde(default)]
    done: bool,
    done_reason: Option<String>,
    error: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

#[derive(Deserialize)]
struct ChunkMessage {
    #[serde(default)]
    content: String,
}

impl ChatApi for Ollama {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn models(&self) -> &[ModelSpec] {
        &self.models
    }

    fn request(
        &self,
        http: &reqwest::Client,
        model: &str,
        history: &[ChatMessage],
        system_prompt: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let mut messages = Vec::with_capacity(history.len() + usize::from(system_prompt.is_some()));
        if let Some(system_prompt) = system_prompt {
            messages.push(Message {
                role: "system",
                content: system_prompt,
            });
        }
        messages.extend(history.iter().map(|m| Message {
            role: match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            content: &m.content,
        }));
        http.post(format!("{}/api/chat", self.base_url))
            .json(&Request {
                model,
                messages,
                stream: true,
            })
    }

    fn decode(&self, line: &str) -> Result<Vec<StreamEvent>, AgentError> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Vec::new());
        }
        let chunk: Chunk = serde_json::from_str(line).map_err(|e| {
            AgentError::provider(format!(
                "ollama: cannot parse stream line: {e} (line: {line})"
            ))
        })?;
        if let Some(error) = chunk.error {
            return Err(AgentError::provider(format!(
                "ollama: stream error: {error}"
            )));
        }
        let mut events = Vec::new();
        if let Some(text) = chunk.message.map(|m| m.content).filter(|c| !c.is_empty()) {
            events.push(StreamEvent::Text(text));
        }
        if chunk.done {
            if let (Some(input), Some(output)) = (chunk.prompt_eval_count, chunk.eval_count) {
                events.push(StreamEvent::Usage { input, output });
            }
            let stop = match chunk.done_reason.as_deref() {
                // Older Ollama responses omit the reason but still carry done.
                None | Some("stop") => StopReason::EndTurn,
                Some("length") => StopReason::MaxTokens,
                Some(reason) => {
                    return Err(AgentError::provider(format!(
                        "ollama: unsupported done reason: {reason}"
                    )));
                }
            };
            events.push(StreamEvent::Finish(stop));
        }
        Ok(events)
    }
}

#[cfg(test)]
#[path = "ollama.tests.rs"]
mod tests;
