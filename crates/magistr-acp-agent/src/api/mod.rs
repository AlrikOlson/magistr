//! The API providers: OpenAI, Gemini and Ollama, driven over their HTTP
//! chat APIs — chat turns with history, streamed text, usage priced by the
//! tables the plugin bridge carried, and a cancel that drops the HTTP stream.
//! No tool harness: these providers never ask for a permission.
//!
//! One generic [`ApiProvider`] owns everything the three share — the
//! per-session history, the streaming loop, the usage arithmetic and the
//! cancel — and a [`ChatApi`] holds only what differs: how a request is
//! built and how one line of the response body decodes.
//!
//! | provider | request | body | usage |
//! |---|---|---|---|
//! | `openai` | `POST {OPENAI_BASE_URL}/chat/completions`, `stream` + `stream_options.include_usage` | SSE `data:` lines, `[DONE]` last | on the final chunk, whose `choices` is empty |
//! | `gemini` | `POST {GEMINI_BASE_URL}/models/{model}:streamGenerateContent?alt=sse`, `x-goog-api-key` | SSE `data:` lines | `usageMetadata` on every chunk; the last is the turn's |
//! | `ollama` | `POST {OLLAMA_HOST}/api/chat`, `stream: true` | NDJSON, one object per line | `prompt_eval_count` / `eval_count` on the `done` line |
//!
//! The fixtures under `tests/fixtures/{openai,gemini,ollama}` are shaped
//! after each API's documented streaming wire (checked 2026-09-02), not
//! recorded from a paid call.

mod api_session;
mod chat_api;
mod chat_message;
mod gemini;
mod line_buffer;
mod model_spec;
mod ollama;
mod openai;
mod pricing;
mod provider;
mod role;
mod stream_event;

pub use chat_api::ChatApi;
pub use chat_message::ChatMessage;
pub use gemini::Gemini;
pub use model_spec::ModelSpec;
pub use ollama::Ollama;
pub use openai::OpenAi;
pub use pricing::Pricing;
pub use provider::ApiProvider;
pub use role::Role;
pub use stream_event::StreamEvent;
