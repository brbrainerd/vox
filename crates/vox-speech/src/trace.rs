//! Correlation IDs for Oratio ↔ MCP ↔ compiler telemetry.

use uuid::Uuid;

/// Generate a new unique correlation id (UUID v4) for a speech/MCP request chain.
#[must_use]
pub fn new_correlation_id() -> String {
    Uuid::new_v4().hyphenated().to_string()
}
