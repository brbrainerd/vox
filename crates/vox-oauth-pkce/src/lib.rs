//! Minimal RFC 8252 (OAuth for Native Apps) loopback-server PKCE flow,
//! provider-agnostic core + an OpenRouter-specific driver.

pub mod pkce;
pub mod openrouter;

pub use pkce::{PkcePair, generate as generate_pkce, generate_state};
