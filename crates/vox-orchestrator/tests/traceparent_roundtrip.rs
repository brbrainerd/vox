//! P0-T8 acceptance: W3C traceparent encode / parse / from-current-span.

use vox_orchestrator::a2a::traceparent::{TraceContext, encode, parse};

#[test]
fn encode_decode_roundtrip() {
    let ctx = TraceContext::new();
    let header = encode(&ctx);
    let parts: Vec<&str> = header.split('-').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "00");
    assert_eq!(parts[1].len(), 32);
    assert_eq!(parts[2].len(), 16);
    assert_eq!(parts[3].len(), 2);

    let parsed = parse(&header).expect("parse");
    assert_eq!(parsed.trace_id, ctx.trace_id);
    assert_eq!(parsed.parent_id, ctx.parent_id);
}

#[test]
fn parse_rejects_malformed() {
    assert!(parse("").is_none());
    assert!(parse("not-a-traceparent").is_none());
    assert!(parse("00-tooshort-1234567812345678-01").is_none());
}

#[test]
fn from_current_span_uses_active_trace() {
    let _guard = tracing_subscriber::fmt().with_test_writer().try_init().ok();
    let ctx = TraceContext::from_current_span();
    assert_ne!(ctx.trace_id, "00000000000000000000000000000000");
}
