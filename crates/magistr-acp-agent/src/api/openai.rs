//! OpenAI over Chat Completions, streamed as server-sent events.

use agent_client_protocol::schema::v1::StopReason;
use serde::{Deserialize, Serialize};

use crate::{AgentError, ChatApi, ChatMessage, ModelSpec, Pricing, Role, StreamEvent};

/// The OpenAI Chat Completions API: `POST {base}/chat/completions` with
/// `stream: true` and `stream_options.include_usage`, so the last SSE chunk
/// (its `choices` empty) carries the turn's usage before `data: [DONE]`.
///
/// The key is `OPENAI_API_KEY`; `OPENAI_BASE_URL` points the provider at a
/// compatible server (the SDKs' own variable), and is how the tests reach a
/// fake one.
pub struct OpenAi {
    api_key: String,
    base_url: String,
    models: Vec<ModelSpec>,
}

impl OpenAi {
    /// The provider's name on the command line.
    pub const NAME: &'static str = "openai";
    /// The environment variable the key is read from.
    pub const KEY_ENV: &'static str = "OPENAI_API_KEY";
    /// The environment variable that overrides the base URL.
    pub const BASE_URL_ENV: &'static str = "OPENAI_BASE_URL";
    /// Where the API lives unless overridden.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.openai.com/v1";

    /// From the environment: refused, naming the variable, without a key.
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env_non_empty(Self::KEY_ENV).ok_or_else(|| {
            AgentError::provider(format!("{} environment variable not set", Self::KEY_ENV))
        })?;
        let base_url =
            env_non_empty(Self::BASE_URL_ENV).unwrap_or_else(|| Self::DEFAULT_BASE_URL.to_owned());
        Ok(Self::new(api_key, base_url))
    }

    /// With an explicit key and base URL.
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            models: registry(),
        }
    }
}

/// The key never prints.
impl std::fmt::Debug for OpenAi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAi")
            .field("base_url", &self.base_url)
            .field("models", &self.models.len())
            .finish_non_exhaustive()
    }
}

fn env_non_empty(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|v| !v.is_empty())
}

/// The models offered and what they charge — the table the OpenAI plugin
/// carried, per million tokens, with each model's context window.
fn registry() -> Vec<ModelSpec> {
    vec![
        ModelSpec::new(
            "gpt-4o",
            "GPT-4o",
            128_000,
            Pricing::per_million(2.50, 10.00),
        ),
        ModelSpec::new(
            "gpt-4o-mini",
            "GPT-4o mini",
            128_000,
            Pricing::per_million(0.15, 0.60),
        ),
        ModelSpec::new("o1", "o1", 200_000, Pricing::per_million(15.00, 60.00)),
        ModelSpec::new(
            "o1-mini",
            "o1-mini",
            128_000,
            Pricing::per_million(3.00, 12.00),
        ),
        ModelSpec::new(
            "o3-mini",
            "o3-mini",
            200_000,
            Pricing::per_million(1.10, 4.40),
        ),
        ModelSpec::new(
            "gpt-4-turbo",
            "GPT-4 Turbo",
            128_000,
            Pricing::per_million(10.00, 30.00),
        ),
        ModelSpec::new("gpt-4", "GPT-4", 8_192, Pricing::per_million(30.00, 60.00)),
        ModelSpec::new(
            "gpt-3.5-turbo",
            "GPT-3.5 Turbo",
            16_385,
            Pricing::per_million(0.50, 1.50),
        ),
    ]
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    stream: bool,
    stream_options: StreamOptions,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Deserialize)]
struct Chunk {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
    finish_reason: Option<String>,
    #[serde(default)]
    index: u64,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    refusal: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

impl ChatApi for OpenAi {
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
        http.post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&Request {
                model,
                messages,
                stream: true,
                stream_options: StreamOptions {
                    include_usage: true,
                },
            })
    }

    fn requires_done(&self) -> bool {
        true
    }

    fn decode(&self, line: &str) -> Result<Vec<StreamEvent>, AgentError> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(Vec::new());
        };
        let data = data.trim();
        if data == "[DONE]" {
            return Ok(vec![StreamEvent::Done]);
        }
        let chunk: Chunk = serde_json::from_str(data).map_err(|e| {
            AgentError::provider(format!(
                "openai: cannot parse stream chunk: {e} (data: {data})"
            ))
        })?;
        if let Some(error) = chunk.error {
            return Err(AgentError::provider(format!(
                "openai: stream error: {error}"
            )));
        }
        let mut events = Vec::new();
        for choice in chunk.choices {
            if choice.index != 0 {
                return Err(AgentError::provider("openai: unexpected choice index"));
            }
            if let Some(text) = choice.delta.content.filter(|text| !text.is_empty()) {
                events.push(StreamEvent::Text(text));
            }
            if let Some(text) = choice.delta.refusal.filter(|text| !text.is_empty()) {
                events.push(StreamEvent::Refusal(text));
            }
            if let Some(reason) = choice.finish_reason {
                let stop = match reason.as_str() {
                    "stop" => StopReason::EndTurn,
                    "length" => StopReason::MaxTokens,
                    "content_filter" => StopReason::Refusal,
                    _ => {
                        return Err(AgentError::provider(format!(
                            "openai: unsupported finish reason: {reason}"
                        )));
                    }
                };
                events.push(StreamEvent::Finish(stop));
            }
        }
        if let Some(usage) = chunk.usage {
            events.push(StreamEvent::Usage {
                input: usage.prompt_tokens,
                output: usage.completion_tokens,
            });
        }
        Ok(events)
    }
}

#[cfg(test)]
#[path = "openai.tests.rs"]
mod tests;
