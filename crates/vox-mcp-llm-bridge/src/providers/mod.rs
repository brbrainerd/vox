//! HTTP clients for OpenRouter-compatible, Gemini, and Ollama chat APIs.

mod anthropic;
mod gemini;
mod metadata;
mod ollama_chat;
mod openai;
mod probe;
mod types;

pub(crate) use anthropic::http_anthropic_direct;
pub(crate) use gemini::http_gemini_with_metadata;
pub(crate) use metadata::HttpCallMetadata;
pub(crate) use ollama_chat::http_ollama_with_metadata;
pub(crate) use openai::http_openai_compatible_with_headers;
pub(crate) use probe::probe_ollama_tags;
pub(crate) use probe::probe_vox_local_health;
