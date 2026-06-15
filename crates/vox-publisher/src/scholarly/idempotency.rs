//! Deterministic idempotency keys aligned with `external_submission_jobs.idempotency_key`.

/// Stable idempotency material for outbound scholarly operations.
///
/// `content_sha3_256` should be the Codex manifest digest (not necessarily
/// [`crate::publication::PublicationManifest::content_sha3_256`] if the row was ingested
/// differently—callers should pass the stored digest from `publication_manifests`).
#[must_use]
pub fn scholarly_idempotency_key(
    adapter: &str,
    publication_id: &str,
    content_sha3_256: &str,
    operation: &str,
) -> String {
    let a = adapter.trim();
    let op = operation.trim();
    format!("{a}:{publication_id}:{content_sha3_256}:{op}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_stable() {
        let k = scholarly_idempotency_key("zenodo", "pub-1", "digest_a", "create_deposition");
        assert_eq!(k, "zenodo:pub-1:digest_a:create_deposition");
    }
}
