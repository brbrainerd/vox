# Semantic Behavior Map — `vox-mesh-types`

Deterministically synthesized from 51 distinct proven-behavior claims (of 51 extracted) across 26 symbols. 4 symbols have an explicit error-path proof; **16 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `decide_replicas`  (edge, happy; EXTRACTED)
- [happy] decide_replicas returns min_replicas (3) when trust tier is below skip_above threshold (Attested < Vetted)  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] decide_replicas returns 1 when trust tier equals skip_above threshold (Vetted)  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] decide_replicas returns 1 when trust tier is above skip_above threshold (Internal > Vetted)  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [edge] decide_replicas clamps min_replicas=0 to 1 minimum  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] Returns min_replicas when RedundancyMode is None  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] Returns min_replicas when RedundancyMode is Majority  (crates/vox-mesh-types/tests/redundancy_voting.rs)

### `Attestation`  (happy; EXTRACTED)
- [happy] Attestation with None tee_quote round-trips to preserve None after JSON serialization/deserialization  (crates/vox-mesh-types/tests/tee_attestation.rs)
- [happy] Attestation with TeeQuote serializes to JSON and deserializes with tee_quote field preserved  (crates/vox-mesh-types/tests/tee_attestation.rs)
- [happy] Attestation without TeeQuote skips tee_quote field in serialized JSON when None  (crates/vox-mesh-types/tests/tee_attestation.rs)
- [happy] Attestation deserializes with tee_quote field as None when absent from JSON  (crates/vox-mesh-types/tests/tee_attestation.rs)

### `MeshTraceContext::from_traceparent`  (error, happy; EXTRACTED)
- [error] Rejects traceparent with unsupported version field (not '00')  (crates/vox-mesh-types/src/trace.rs)
- [error] Rejects traceparent with missing or wrong number of dash-separated fields  (crates/vox-mesh-types/src/trace.rs)
- [error] Rejects trace_id with non-hexadecimal characters  (crates/vox-mesh-types/src/trace.rs)
- [happy] Parses W3C traceparent produced by new_root and returns equivalent context  (crates/vox-mesh-types/src/trace.rs)

### `vote_majority`  (edge, happy; EXTRACTED)
- [edge] vote_majority returns VoteOutcome::NoVotes for empty input  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] vote_majority returns VoteOutcome::Consensus with single output hash for one replica  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] vote_majority returns VoteOutcome::Consensus when all replicas agree on output  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] vote_majority returns VoteOutcome::Majority with majority hash and minority_count when 2 of 3 replicas match  (crates/vox-mesh-types/tests/redundancy_voting.rs)

### `RedundancyPolicy`  (happy; EXTRACTED)
- [happy] RedundancyPolicy serializes to JSON and deserializes with mode field preserved  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] RedundancyPolicy deserializes with min_replicas field correctly set  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] RedundancyPolicy deserializes with skip_above Option field correctly set  (crates/vox-mesh-types/tests/redundancy_voting.rs)

### `WorkerDonationPolicy`  (happy; EXTRACTED)
- [happy] WorkerDonationPolicy deserializes JSON containing redundancy field  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] WorkerDonationPolicy stores redundancy field with min_replicas value after deserialization  (crates/vox-mesh-types/tests/redundancy_voting.rs)
- [happy] WorkerDonationPolicy deserializes with redundancy field absent defaulting to None  (crates/vox-mesh-types/tests/redundancy_voting.rs)

### `MeshTraceContext::is_sampled`  (happy; EXTRACTED)
- [happy] Returns true when trace_flags has bit 0 set  (crates/vox-mesh-types/src/trace.rs)
- [happy] Returns false when trace_flags=0x00 (unsampled bit not set)  (crates/vox-mesh-types/src/trace.rs)

### `OpFragmentEnvelope`  (happy; EXTRACTED)
- [happy] JSON serialization and deserialization preserves id field  (crates/vox-mesh-types/tests/federation_envelope.rs)
- [happy] JSON serialization and deserialization preserves actor field  (crates/vox-mesh-types/tests/federation_envelope.rs)

### `OpFragmentKind`  (edge, happy; EXTRACTED)
- [happy] JSON serialization and deserialization preserves TaskDispatched kind variant  (crates/vox-mesh-types/tests/federation_envelope.rs)
- [edge] Deserializes unrecognized 'type' field as OpFragmentKind::Unknown  (crates/vox-mesh-types/tests/federation_envelope.rs)

### `PublicAttestationManifest`  (happy; EXTRACTED)
- [happy] JSON serialization and deserialization preserves node_id field  (crates/vox-mesh-types/tests/attestation_manifest.rs)
- [happy] JSON serialization and deserialization preserves supported_tasks list length and content  (crates/vox-mesh-types/tests/attestation_manifest.rs)

### `PublicAttestationManifest::canonical_signing_bytes`  (happy, invariant; EXTRACTED)
- [happy] Returns bytes with signature_b64 field set to empty string  (crates/vox-mesh-types/tests/attestation_manifest.rs)
- [invariant] Produces identical bytes on consecutive calls (deterministic)  (crates/vox-mesh-types/tests/attestation_manifest.rs)

### `TeeQuote`  (happy, invariant; EXTRACTED)
- [happy] TeeQuote.kind field deserializes with value TeeQuoteKind::Stub  (crates/vox-mesh-types/tests/tee_attestation.rs)
- [invariant] TeeQuote.measurement_blake3_hex field has 64-character length after deserialization  (crates/vox-mesh-types/tests/tee_attestation.rs)

### `TeeQuoteKind`  (happy; EXTRACTED)
- [happy] TeeQuoteKind::Stub field round-trips correctly through JSON serialization/deserialization  (crates/vox-mesh-types/tests/tee_attestation.rs)
- [happy] TeeQuoteKind::IntelTdx round-trips correctly when nested in TaskResult.attestation.tee_quote  (crates/vox-mesh-types/tests/tee_attestation.rs)

### `Attestation.tee_quote`  (happy; EXTRACTED)
- [happy] tee_quote field is omitted from JSON serialization when None (skip_serializing_if attribute works)  (crates/vox-mesh-types/tests/tee_attestation.rs)

### `MeshTraceContext`  (happy; EXTRACTED)
- [happy] Parses W3C traceparent format and serializes back to identical string  (crates/vox-mesh-types/src/trace.rs)

### `MeshTraceContext::new_root`  (happy; EXTRACTED)
- [happy] Generates context that serializes to valid W3C traceparent format  (crates/vox-mesh-types/src/trace.rs)

### `MeshTraceContext::to_traceparent`  (happy; EXTRACTED)
- [happy] Preserves unsampled flag (0x00) in serialized traceparent  (crates/vox-mesh-types/src/trace.rs)

### `MeshTraceContext::trace_id_hex`  (happy; EXTRACTED)
- [happy] Returns 32-character lowercase hex representation of trace_id  (crates/vox-mesh-types/src/trace.rs)

### `OpFragmentEnvelope::canonical_signing_bytes`  (happy; EXTRACTED)
- [happy] Produces JSON bytes with signature_b64 field set to empty string  (crates/vox-mesh-types/tests/federation_envelope.rs)

### `SpanId::from_hex`  (error; EXTRACTED)
- [error] Rejects all-zero span_id (per W3C spec)  (crates/vox-mesh-types/src/trace.rs)

### `StubTeeVerifier.verify`  (happy; EXTRACTED)
- [happy] verify() returns Err for any TeeQuote regardless of kind  (crates/vox-mesh-types/tests/tee_attestation.rs)

### `TaskResult`  (happy; EXTRACTED)
- [happy] TaskResult with nested Attestation containing TeeQuote round-trips correctly through JSON  (crates/vox-mesh-types/tests/tee_attestation.rs)

### `TeeQuote.measurement_blake3_hex`  (happy; EXTRACTED)
- [happy] measurement_blake3_hex field is present and contains 64 hex characters when round-tripped  (crates/vox-mesh-types/tests/tee_attestation.rs)

### `TeeVerifyError::Unsupported`  (error; EXTRACTED)
- [error] TeeVerifyError::Unsupported variant contains the input TeeQuoteKind when verify() fails  (crates/vox-mesh-types/tests/tee_attestation.rs)

### `TraceId::from_hex`  (error; EXTRACTED)
- [error] Rejects all-zero trace_id (per W3C spec)  (crates/vox-mesh-types/src/trace.rs)

### `decide_replicas_with_seed`  (invariant; EXTRACTED)
- [invariant] decide_replicas_with_seed with a seed produces same replica count as decide_replicas without seed  (crates/vox-mesh-types/tests/redundancy_voting.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`Attestation`** — only: _Attestation with None tee_quote round-trips to preserve None after JSON serialization/deserialization_
- **`Attestation.tee_quote`** — only: _tee_quote field is omitted from JSON serialization when None (skip_serializing_if attribute works)_
- **`MeshTraceContext`** — only: _Parses W3C traceparent format and serializes back to identical string_
- **`MeshTraceContext::is_sampled`** — only: _Returns true when trace_flags has bit 0 set_
- **`MeshTraceContext::new_root`** — only: _Generates context that serializes to valid W3C traceparent format_
- **`MeshTraceContext::to_traceparent`** — only: _Preserves unsampled flag (0x00) in serialized traceparent_
- **`MeshTraceContext::trace_id_hex`** — only: _Returns 32-character lowercase hex representation of trace_id_
- **`OpFragmentEnvelope`** — only: _JSON serialization and deserialization preserves id field_
- **`OpFragmentEnvelope::canonical_signing_bytes`** — only: _Produces JSON bytes with signature_b64 field set to empty string_
- **`PublicAttestationManifest`** — only: _JSON serialization and deserialization preserves node_id field_
- **`RedundancyPolicy`** — only: _RedundancyPolicy serializes to JSON and deserializes with mode field preserved_
- **`StubTeeVerifier.verify`** — only: _verify() returns Err for any TeeQuote regardless of kind_
- **`TaskResult`** — only: _TaskResult with nested Attestation containing TeeQuote round-trips correctly through JSON_
- **`TeeQuote.measurement_blake3_hex`** — only: _measurement_blake3_hex field is present and contains 64 hex characters when round-tripped_
- **`TeeQuoteKind`** — only: _TeeQuoteKind::Stub field round-trips correctly through JSON serialization/deserialization_
- **`WorkerDonationPolicy`** — only: _WorkerDonationPolicy deserializes JSON containing redundancy field_
