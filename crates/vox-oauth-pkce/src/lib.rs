//! Minimal RFC 8252 (OAuth for Native Apps) loopback-server PKCE flow,
//! provider-agnostic core + an OpenRouter-specific driver.

pub mod openrouter;
pub mod pkce;

pub use pkce::{PkcePair, generate as generate_pkce, generate_state};
