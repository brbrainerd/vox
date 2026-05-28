//! Agent-Computer Interface (ACI) envelopes for MCP tool JSON payloads.

mod contracts;
mod envelope;
mod normalization;

pub use contracts::ACI_TOOL_RESPONSE_SCHEMA_RELPATH;
pub use envelope::attach_aci_envelope;
pub use normalization::tool_name_for_aci;
