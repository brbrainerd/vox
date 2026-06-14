//! W3C `traceparent`-compatible mesh trace context.
//!
//! Provides a minimal, zero-external-dependency (uses `rand`) propagation
//! primitive that is forward-compatible with OpenTelemetry without pulling in
//! the OTel crate.  S2 will extend this to cross-node propagation; S1 wires
//! the local path only.
//!
//! # Wire format
//!
//! `00-{32 lowercase hex trace_id}-{16 lowercase hex span_id}-{2 hex flags}`
//!
//! Example: `00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01`

use std::fmt;

use rand::RngCore;
use serde::{Deserialize, Serialize};

/// 16-byte trace identifier (128-bit, W3C `traceparent` field 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId([u8; 16]);

/// 8-byte span identifier (64-bit, W3C `traceparent` field 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId([u8; 8]);

impl TraceId {
    pub fn random() -> Self {
        let mut buf = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut buf);
        Self(buf)
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.0)
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        let bytes = hex_decode_fixed::<16>(s)?;
        if bytes == [0u8; 16] {
            return None; // W3C: all-zeros is invalid
        }
        Some(Self(bytes))
    }
}

impl SpanId {
    pub fn random() -> Self {
        let mut buf = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut buf);
        Self(buf)
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.0)
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        let bytes = hex_decode_fixed::<8>(s)?;
        if bytes == [0u8; 8] {
            return None; // W3C: all-zeros is invalid
        }
        Some(Self(bytes))
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Error returned by [`MeshTraceContext::from_traceparent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTraceparentError(String);

impl fmt::Display for ParseTraceparentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid traceparent: {}", self.0)
    }
}

impl std::error::Error for ParseTraceparentError {}

/// Minimal W3C-traceparent-compatible trace context.
///
/// Carries a `trace_id` (stable across the whole task), a `parent_span_id`
/// (identifies the producing span), and W3C `trace_flags` (bit 0 = sampled).
///
/// In S1 this context flows through the **local** path only:
/// orchestrator → populi A2A envelope → handler span.  Cross-node propagation
/// is wired in S2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshTraceContext {
    pub trace_id: TraceId,
    pub parent_span_id: SpanId,
    /// W3C trace flags byte (bit 0 = sampled).  Default `0x01` (always sample in S1).
    pub trace_flags: u8,
}

impl MeshTraceContext {
    /// Create a brand-new root context (fresh trace_id and span_id).
    pub fn new_root() -> Self {
        Self {
            trace_id: TraceId::random(),
            parent_span_id: SpanId::random(),
            trace_flags: 0x01,
        }
    }

    /// Parse a W3C traceparent string.
    ///
    /// Accepts version `00` only (the only currently defined version).
    pub fn from_traceparent(s: &str) -> Result<Self, ParseTraceparentError> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 4 {
            return Err(ParseTraceparentError(format!(
                "expected 4 dash-separated fields, got {}",
                parts.len()
            )));
        }
        if parts[0] != "00" {
            return Err(ParseTraceparentError(format!(
                "unsupported version {:?} (only '00' supported)",
                parts[0]
            )));
        }
        let trace_id = TraceId::from_hex(parts[1])
            .ok_or_else(|| ParseTraceparentError(format!("invalid trace_id {:?}", parts[1])))?;
        let parent_span_id = SpanId::from_hex(parts[2]).ok_or_else(|| {
            ParseTraceparentError(format!("invalid parent_span_id {:?}", parts[2]))
        })?;
        let flags_bytes = hex_decode_fixed::<1>(parts[3])
            .ok_or_else(|| ParseTraceparentError(format!("invalid trace_flags {:?}", parts[3])))?;
        Ok(Self {
            trace_id,
            parent_span_id,
            trace_flags: flags_bytes[0],
        })
    }

    /// Serialize to W3C traceparent string.
    pub fn to_traceparent(&self) -> String {
        format!(
            "00-{}-{}-{:02x}",
            self.trace_id.to_hex(),
            self.parent_span_id.to_hex(),
            self.trace_flags,
        )
    }

    /// Produce a child context: same `trace_id`, new random `parent_span_id`.
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id,
            parent_span_id: SpanId::random(),
            trace_flags: self.trace_flags,
        }
    }

    /// `trace_id` as a 32-char lowercase hex string (for span attributes).
    pub fn trace_id_hex(&self) -> String {
        self.trace_id.to_hex()
    }

    /// Whether the sampled bit (bit 0) is set.
    pub fn is_sampled(&self) -> bool {
        self.trace_flags & 0x01 != 0
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode_fixed<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    #[test]
    fn traceparent_round_trip() {
        let ctx = MeshTraceContext::from_traceparent(KNOWN).unwrap();
        assert_eq!(ctx.to_traceparent(), KNOWN);
        assert_eq!(ctx.trace_id_hex(), "4bf92f3577b34da6a3ce929d0e0e4736");
        assert!(ctx.is_sampled());
    }

    #[test]
    fn traceparent_rejects_malformed() {
        let cases = [
            "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01", // bad version
            "4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",    // missing version
            "00-ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ-00f067aa0ba902b7-01", // non-hex trace_id
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01", // all-zero trace_id
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01", // all-zero span_id
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7",    // missing flags
            "",
        ];
        for bad in &cases {
            assert!(
                MeshTraceContext::from_traceparent(bad).is_err(),
                "should reject: {bad:?}"
            );
        }
    }

    #[test]
    fn child_preserves_trace_id_and_changes_span_id() {
        let parent = MeshTraceContext::new_root();
        let child = parent.child();
        assert_eq!(
            parent.trace_id, child.trace_id,
            "trace_id must be preserved"
        );
        assert_ne!(
            parent.parent_span_id, child.parent_span_id,
            "span_id must change"
        );
        assert_eq!(parent.trace_flags, child.trace_flags);
    }

    #[test]
    fn new_root_produces_valid_traceparent() {
        let ctx = MeshTraceContext::new_root();
        let s = ctx.to_traceparent();
        let parsed = MeshTraceContext::from_traceparent(&s).unwrap();
        assert_eq!(ctx, parsed);
    }

    #[test]
    fn unsample_flag_preserved() {
        let s = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
        let ctx = MeshTraceContext::from_traceparent(s).unwrap();
        assert!(!ctx.is_sampled());
        assert_eq!(ctx.to_traceparent(), s);
    }
}

#[cfg(test)]
mod semcov_wave25_tests {
    use super::*;

    // Catches: TraceId::from_hex accepting uppercase hex and returning wrong bytes
    // (hex_nibble handles A-F but a mismatch could silently produce wrong IDs).
    #[test]
    fn trace_id_from_hex_uppercase_accepted() {
        let upper = "4BF92F3577B34DA6A3CE929D0E0E4736";
        let lower = "4bf92f3577b34da6a3ce929d0e0e4736";
        let from_upper = TraceId::from_hex(upper).expect("uppercase must parse");
        let from_lower = TraceId::from_hex(lower).expect("lowercase must parse");
        assert_eq!(
            from_upper.to_hex(),
            from_lower.to_hex(),
            "upper- and lower-case hex must decode to identical bytes"
        );
    }

    // Catches: TraceId::from_hex accepting a string that is one character too short
    // (length guard off by one — e.g., checking `< N*2` instead of `!= N*2`).
    #[test]
    fn trace_id_rejects_wrong_length_hex() {
        // 31 hex chars (should be 32)
        assert!(
            TraceId::from_hex("4bf92f3577b34da6a3ce929d0e0e473").is_none(),
            "31-char hex must be rejected"
        );
        // 33 hex chars
        assert!(
            TraceId::from_hex("4bf92f3577b34da6a3ce929d0e0e47366").is_none(),
            "33-char hex must be rejected"
        );
        // empty
        assert!(
            TraceId::from_hex("").is_none(),
            "empty string must be rejected"
        );
    }

    // Catches: SpanId::from_hex accepting a string of the wrong length
    // (copy-paste of TraceId length constant in hex_decode_fixed could produce silent truncation).
    #[test]
    fn span_id_rejects_wrong_length_hex() {
        // 15 hex chars (should be 16)
        assert!(
            SpanId::from_hex("00f067aa0ba902b").is_none(),
            "15-char span hex must be rejected"
        );
        // 17 hex chars
        assert!(
            SpanId::from_hex("00f067aa0ba902b7ff").is_none(),
            "17-char span hex must be rejected"
        );
    }

    // Catches: to_traceparent padding trace_flags with the wrong format — e.g.,
    // `{:x}` instead of `{:02x}` causing single-digit flags like `1` instead of `01`.
    #[test]
    fn traceparent_flags_always_two_hex_digits() {
        let s = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let ctx = MeshTraceContext::from_traceparent(s).unwrap();
        let out = ctx.to_traceparent();
        let parts: Vec<&str> = out.split('-').collect();
        assert_eq!(
            parts[3].len(),
            2,
            "trace_flags field must always be exactly 2 hex chars, got {:?}",
            parts[3]
        );
    }

    // Catches: child() inheriting the wrong trace_flags (e.g., hard-coding 0x01 instead
    // of copying from parent), which would silently flip sampling on child spans.
    #[test]
    fn child_inherits_trace_flags_from_parent() {
        // Build a context with flags = 0x00 (not sampled).
        let s = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00";
        let parent = MeshTraceContext::from_traceparent(s).unwrap();
        assert!(!parent.is_sampled());
        let child = parent.child();
        assert_eq!(
            child.trace_flags, parent.trace_flags,
            "child must inherit trace_flags from parent"
        );
        assert!(!child.is_sampled());
    }

    // Catches: TraceId::to_hex producing uppercase or mixed-case output, which
    // would break the round-trip through from_traceparent (which lowercases the format string).
    #[test]
    fn trace_id_to_hex_is_lowercase() {
        let s = "4bf92f3577b34da6a3ce929d0e0e4736";
        let id = TraceId::from_hex(s).unwrap();
        assert_eq!(id.to_hex(), s, "to_hex must return lowercase");
    }

    // Catches: SpanId::to_hex producing uppercase output.
    #[test]
    fn span_id_to_hex_is_lowercase() {
        let s = "00f067aa0ba902b7";
        let id = SpanId::from_hex(s).unwrap();
        assert_eq!(id.to_hex(), s, "span to_hex must return lowercase");
    }

    // Catches: MeshTraceContext serde round-trip silently dropping trace_flags
    // or mangling the nested structs (TraceId/SpanId as byte arrays).
    #[test]
    fn mesh_trace_context_serde_round_trip() {
        let s = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let ctx = MeshTraceContext::from_traceparent(s).unwrap();
        let json = serde_json::to_string(&ctx).unwrap();
        let back: MeshTraceContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, back);
        assert_eq!(back.to_traceparent(), s);
    }

    // Catches: from_traceparent accepting a string with 5 dash-separated segments
    // (e.g., a future-version string with an extra field) instead of rejecting it.
    #[test]
    fn traceparent_rejects_too_many_fields() {
        let s = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-extra";
        assert!(
            MeshTraceContext::from_traceparent(s).is_err(),
            "5-field traceparent must be rejected"
        );
    }

    // Catches: ParseTraceparentError's Display impl omitting the inner message,
    // making error reporting useless in logs.
    #[test]
    fn parse_error_display_contains_reason() {
        let err = MeshTraceContext::from_traceparent("").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("traceparent"),
            "error display must mention 'traceparent', got: {msg:?}"
        );
    }

    // Catches: new_root() sometimes producing all-zero TraceId or SpanId due to
    // a bug in the random fill (e.g., wrong buffer size, fill_bytes no-op).
    #[test]
    fn new_root_ids_are_not_all_zeros() {
        // Run several times to reduce flakiness on the astronomically unlikely all-zero case.
        for _ in 0..5 {
            let ctx = MeshTraceContext::new_root();
            assert_ne!(
                ctx.trace_id.to_hex(),
                "00000000000000000000000000000000",
                "new_root trace_id must not be all zeros"
            );
            assert_ne!(
                ctx.parent_span_id.to_hex(),
                "0000000000000000",
                "new_root span_id must not be all zeros"
            );
        }
    }

    // Catches: trace_id_hex() returning something different from trace_id.to_hex(),
    // e.g., if it formats the inner bytes differently.
    #[test]
    fn trace_id_hex_matches_to_hex() {
        let ctx = MeshTraceContext::new_root();
        assert_eq!(
            ctx.trace_id_hex(),
            ctx.trace_id.to_hex(),
            "trace_id_hex() and trace_id.to_hex() must agree"
        );
    }
}
