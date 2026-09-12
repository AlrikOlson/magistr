//! Gemini over `streamGenerateContent`, streamed as server-sent events.

use agent_client_protocol::schema::v1::StopReason;
use serde::{Deserialize, Serialize};

use crate::{AgentError, ChatApi, ChatMessage, ModelSpec, Pricing, Role, StreamEvent};

/// The Gemini API: `POST {base}/models/{model}:streamGenerateContent?alt=sse`
/// with the key in the `x-goog-api-key` header (never in the URL, where a
/// log would keep it). Every SSE chunk carries `candidates[].content.parts[]
/// .text` and a `usageMetadata`; the last chunk's counts are the turn's.
///
/// The key is `GEMINI_API_KEY` or `GOOGLE_API_KEY`; `GEMINI_BASE_URL`
/// overrides the endpoint, and is how the tests reach a fake one.
pub struct Gemini {
    api_key: String,
    base_url: String,
    models: Vec<ModelSpec>,
}

impl Gemini {
    /// The provider's name on the command line.
    pub const NAME: &'static str = "gemini";
    /// The environment variables the key is read from, in order.
    pub const KEY_ENVS: [&'static str; 2] = ["GEMINI_API_KEY", "GOOGLE_API_KEY"];
    /// The environment variable that overrides the base URL.
    pub const BASE_URL_ENV: &'static str = "GEMINI_BASE_URL";
    /// Where the API lives unless overridden.
    pub const DEFAULT_BASE_URL: &'static str = "https://generativelanguage.googleapis.com/v1beta";
    /// The context window every listed model has.
    const WINDOW: u64 = 1_048_576;

    /// From the environment: refused, naming both variables, without a key.
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = Self::KEY_ENVS
            .iter()
            .find_map(|var| env_non_empty(var))
            .ok_or_else(|| {
                AgentError::provider(format!(
                    "{} or {} environment variable not set",
                    Self::KEY_ENVS[0],
                    Self::KEY_ENVS[1]
                ))
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
impl std::fmt::Debug for Gemini {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gemini")
            .field("base_url", &self.base_url)
            .field("models", &self.models.len())
            .finish_non_exhaustive()
    }
}

fn env_non_empty(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|v| !v.is_empty())
}

/// The models offered, priced the way the in-tree Gemini backend priced
/// them: a Flash rate and a Pro rate, per million tokens.
fn registry() -> Vec<ModelSpec> {
    let flash = Pricing::per_million(0.075, 0.30);
    let pro = Pricing::per_million(1.25, 5.00);
    vec![
        ModelSpec::new(
            "gemini-2.5-flash",
            "Gemini 2.5 Flash",
            Gemini::WINDOW,
            flash,
        ),
        ModelSpec::new("gemini-2.5-pro", "Gemini 2.5 Pro", Gemini::WINDOW, pro),
        ModelSpec::new(
            "gemini-2.0-flash",
            "Gemini 2.0 Flash",
            Gemini::WINDOW,
            flash,
        ),
    ]
}

#[derive(Serialize)]
struct Request<'a> {
    contents: Vec<Content<'a>>,
}

#[derive(Serialize)]
struct Content<'a> {
    role: &'static str,
    parts: Vec<Part<'a>>,
}

#[derive(Serialize)]
struct Part<'a> {
    text: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Chunk {
    #[serde(default)]
    candidates: Vec<Candidate>,
    #[serde(default)]
    usage_metadata: Option<UsageMetadata>,
    prompt_feedback: Option<PromptFeedback>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptFeedback {
    block_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    #[serde(default)]
    content: Option<CandidateContent>,
    finish_reason: Option<String>,
    #[serde(default)]
    index: u64,
}

#[derive(Deserialize)]
struct CandidateContent {
    #[serde(default)]
    parts: Vec<CandidatePart>,
}

#[derive(Deserialize)]
struct CandidatePart {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    #[serde(default)]
    prompt_token_count: u64,
    #[serde(default)]
    candidates_token_count: u64,
}

impl ChatApi for Gemini {
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
    ) -> reqwest::RequestBuilder {
        let contents = history
            .iter()
            .map(|m| Content {
                role: match m.role {
                    Role::User => "user",
                    Role::Assistant => "model",
                },
                parts: vec![Part { text: &m.content }],
            })
            .collect();
        http.post(format!(
            "{}/models/{model}:streamGenerateContent?alt=sse",
            self.base_url
        ))
        .header("x-goog-api-key", &self.api_key)
        .json(&Request { contents })
    }

    fn decode(&self, line: &str) -> Result<Vec<StreamEvent>, AgentError> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(Vec::new());
        };
        let data = data.trim();
        let chunk: Chunk = serde_json::from_str(data).map_err(|e| {
            AgentError::provider(format!(
                "gemini: cannot parse stream chunk: {e} (data: {data})"
            ))
        })?;
        if let Some(error) = chunk.error {
            return Err(AgentError::provider(format!(
                "gemini: stream error: {error}"
            )));
        }
        let mut events = Vec::new();
        if let Some(reason) = chunk
            .prompt_feedback
            .and_then(|feedback| feedback.block_reason)
        {
            let stop = match reason.as_str() {
                "SAFETY" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "IMAGE_SAFETY" => {
                    StopReason::Refusal
                }
                _ => {
                    return Err(AgentError::provider(format!(
                        "gemini: prompt blocked: {reason}"
                    )));
                }
            };
            events.push(StreamEvent::Finish(stop));
        }
        for candidate in chunk.candidates {
            if candidate.index != 0 {
                return Err(AgentError::provider("gemini: unexpected candidate index"));
            }
            events.extend(
                candidate
                    .content
                    .into_iter()
                    .flat_map(|content| content.parts)
                    .filter_map(|part| part.text)
                    .filter(|text| !text.is_empty())
                    .map(StreamEvent::Text),
            );
            if let Some(reason) = candidate.finish_reason {
                let stop = match reason.as_str() {
                    "STOP" => StopReason::EndTurn,
                    "MAX_TOKENS" => StopReason::MaxTokens,
                    "SAFETY"
                    | "RECITATION"
                    | "BLOCKLIST"
                    | "PROHIBITED_CONTENT"
                    | "SPII"
                    | "IMAGE_SAFETY"
                    | "IMAGE_PROHIBITED_CONTENT"
                    | "IMAGE_RECITATION"
                    | "ESCALATION" => StopReason::Refusal,
                    _ => {
                        return Err(AgentError::provider(format!(
                            "gemini: unsupported finish reason: {reason}"
                        )));
                    }
                };
                events.push(StreamEvent::Finish(stop));
            }
        }
        if let Some(usage) = chunk.usage_metadata {
            events.push(StreamEvent::Usage {
                input: usage.prompt_token_count,
                output: usage.candidates_token_count,
            });
        }
        Ok(events)
    }
}

#[cfg(test)]
#[path = "gemini.tests.rs"]
mod tests;
