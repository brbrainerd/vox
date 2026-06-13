# Semantic Behavior Map — `vox-research-events`

Deterministically synthesized from 20 distinct proven-behavior claims (of 20 extracted) across 18 symbols. 1 symbols have an explicit error-path proof; **17 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `KeywordObservationClassifier::classify()`  (happy; EXTRACTED)
- [happy] classify() identifies 'p95 latency increased by 15ms' as ObservationClass::ProviderObservation  (crates/vox-research-events/src/observation.rs)
- [happy] classify() identifies 'MMLU accuracy improved to 92%' as ObservationClass::ModelCapabilityEvidence  (crates/vox-research-events/src/observation.rs)

### `validate_short_form()`  (error, happy; EXTRACTED)
- [happy] validate_short_form() returns Ok for ShortFormVariant with valid non-empty nanopub_uri  (crates/vox-research-events/src/publication_format.rs)
- [error] validate_short_form() returns Err for ShortFormVariant with empty nanopub_uri  (crates/vox-research-events/src/publication_format.rs)

### `DiscoverySignal`  (happy; EXTRACTED)
- [happy] DiscoverySignal round-trips through JSON serialization preserving code, strength, family, and provenance.origin fields  (crates/vox-research-events/src/schema_types.rs)

### `EvidencePackV1`  (happy; EXTRACTED)
- [happy] EvidencePackV1 serializes to JSON and deserializes back with version and publication_id fields preserved  (crates/vox-research-events/src/schema_types.rs)

### `EvidencePackV1::baseline`  (happy; EXTRACTED)
- [happy] baseline (RunRef) field survives serialization with nested run_id preserved  (crates/vox-research-events/src/schema_types.rs)

### `EvidencePackV1::pair_integrity_passed`  (happy; EXTRACTED)
- [happy] pair_integrity_passed Option field is preserved through JSON round-trip  (crates/vox-research-events/src/schema_types.rs)

### `FindingCandidateV1`  (happy; EXTRACTED)
- [happy] FindingCandidateV1 round-trips through JSON serialization preserving candidate_id, candidate_class, schema_version, and internal_signals  (crates/vox-research-events/src/schema_types.rs)

### `LearnedProfileRow`  (happy; EXTRACTED)
- [happy] LearnedProfileRow round-trips through JSON serialization with provider and sample_count preserved  (crates/vox-research-events/src/observation.rs)

### `NoveltyEvidenceBundle`  (happy; EXTRACTED)
- [happy] NoveltyEvidenceBundle serializes to JSON and deserializes back with bundle_id preserved  (crates/vox-research-events/src/schema_types.rs)

### `NoveltyEvidenceBundle::normalized_hits`  (happy; EXTRACTED)
- [happy] normalized_hits collection survives JSON round-trip with element count and content preserved  (crates/vox-research-events/src/schema_types.rs)

### `PreregistrationV1`  (happy; EXTRACTED)
- [happy] PreregistrationV1 round-trips through JSON serialization preserving id, cost_cap_usd, and statistical_test.kind fields  (crates/vox-research-events/src/preregistration.rs)

### `PublicationPlatform::Bluesky::max_chars()`  (happy; EXTRACTED)
- [happy] max_chars() on PublicationPlatform::Bluesky returns 300  (crates/vox-research-events/src/publication_format.rs)

### `ResearchEvent`  (happy; EXTRACTED)
- [happy] ResearchEvent::ClaimExtracted round-trips through JSON serialization and deserializes with matching claim_id variant  (crates/vox-research-events/src/events.rs)

### `ResearchEvent::PreregistrationSubmitted`  (happy; EXTRACTED)
- [happy] PreregistrationSubmitted variant serializes to JSON containing the literal string 'PreregistrationSubmitted'  (crates/vox-research-events/src/events.rs)

### `ResearchEvent::kind()`  (happy; EXTRACTED)
- [happy] kind() method on ResearchEvent::ClaimExtracted returns ResearchEventKind::ClaimExtracted  (crates/vox-research-events/src/events.rs)

### `WorthinessSignalsV2`  (happy; EXTRACTED)
- [happy] WorthinessSignalsV2 serializes to JSON and deserializes back with version and profile fields preserved  (crates/vox-research-events/src/schema_types.rs)

### `WorthinessSignalsV2::hard_gate`  (happy; EXTRACTED)
- [happy] hard_gate collection is preserved through JSON serialization with element count and nested id field intact  (crates/vox-research-events/src/schema_types.rs)

### `WorthinessSignalsV2::next_actions`  (happy; EXTRACTED)
- [happy] next_actions collection survives JSON round-trip with nested WorthinessActionItem.priority field preserved  (crates/vox-research-events/src/schema_types.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`DiscoverySignal`** — only: _DiscoverySignal round-trips through JSON serialization preserving code, strength, family, and provenance.origin fields_
- **`EvidencePackV1`** — only: _EvidencePackV1 serializes to JSON and deserializes back with version and publication_id fields preserved_
- **`EvidencePackV1::baseline`** — only: _baseline (RunRef) field survives serialization with nested run_id preserved_
- **`EvidencePackV1::pair_integrity_passed`** — only: _pair_integrity_passed Option field is preserved through JSON round-trip_
- **`FindingCandidateV1`** — only: _FindingCandidateV1 round-trips through JSON serialization preserving candidate_id, candidate_class, schema_version, and internal_signals_
- **`KeywordObservationClassifier::classify()`** — only: _classify() identifies 'p95 latency increased by 15ms' as ObservationClass::ProviderObservation_
- **`LearnedProfileRow`** — only: _LearnedProfileRow round-trips through JSON serialization with provider and sample_count preserved_
- **`NoveltyEvidenceBundle`** — only: _NoveltyEvidenceBundle serializes to JSON and deserializes back with bundle_id preserved_
- **`NoveltyEvidenceBundle::normalized_hits`** — only: _normalized_hits collection survives JSON round-trip with element count and content preserved_
- **`PreregistrationV1`** — only: _PreregistrationV1 round-trips through JSON serialization preserving id, cost_cap_usd, and statistical_test.kind fields_
- **`PublicationPlatform::Bluesky::max_chars()`** — only: _max_chars() on PublicationPlatform::Bluesky returns 300_
- **`ResearchEvent`** — only: _ResearchEvent::ClaimExtracted round-trips through JSON serialization and deserializes with matching claim_id variant_
- **`ResearchEvent::PreregistrationSubmitted`** — only: _PreregistrationSubmitted variant serializes to JSON containing the literal string 'PreregistrationSubmitted'_
- **`ResearchEvent::kind()`** — only: _kind() method on ResearchEvent::ClaimExtracted returns ResearchEventKind::ClaimExtracted_
- **`WorthinessSignalsV2`** — only: _WorthinessSignalsV2 serializes to JSON and deserializes back with version and profile fields preserved_
- **`WorthinessSignalsV2::hard_gate`** — only: _hard_gate collection is preserved through JSON serialization with element count and nested id field intact_
- **`WorthinessSignalsV2::next_actions`** — only: _next_actions collection survives JSON round-trip with nested WorthinessActionItem.priority field preserved_
