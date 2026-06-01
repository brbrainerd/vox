//! Sender-side helper for dispatching executable `.vox` source over the mesh.
//!
//! Builds the integrity-paired `exec_source_b64` + `exec_source_blake3_hex`
//! fields a [`RemoteTaskEnvelope`](super::RemoteTaskEnvelope) carries; the
//! remote worker (`remote_worker::run_dispatched_source`) re-verifies the BLAKE3
//! hash before executing. Pure (base64 + BLAKE3) and not feature-gated — only
//! the worker-side execution lives behind `populi-transport`.

use base64::engine::Engine as _;

/// Build the `(exec_source_b64, exec_source_blake3_hex)` pair for a `.vox`
/// source string: standard base64 of the UTF-8 bytes plus their BLAKE3 hex
/// digest. A script-dispatch sender sets these on the envelope's
/// `exec_source_b64` / `exec_source_blake3_hex` fields.
#[must_use]
pub fn build_exec_source_fields(source: &str) -> (String, String) {
    let bytes = source.as_bytes();
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let hex = blake3::hash(bytes).to_hex().to_string();
    (b64, hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_exec_source_fields_round_trips_bytes_and_hash() {
        let src = "pub fn main() {\n    print(\"hi\")\n}\n";
        let (b64, hex) = build_exec_source_fields(src);

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .expect("valid base64");
        assert_eq!(decoded, src.as_bytes(), "base64 must round-trip the source");
        assert_eq!(
            blake3::hash(&decoded).to_hex().to_string(),
            hex,
            "the hash must match the encoded bytes (worker re-verifies this)"
        );
    }

    #[test]
    fn distinct_sources_produce_distinct_hashes() {
        let (_, h1) = build_exec_source_fields("pub fn main() { print(\"a\") }");
        let (_, h2) = build_exec_source_fields("pub fn main() { print(\"b\") }");
        assert_ne!(h1, h2);
    }
}
