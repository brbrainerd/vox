# vox-nanopub — Semantic Behavior Map

One-paragraph summary: Across 3 extracted Behavior claims, vox-nanopub has 2 symbols under test. The cryptographic verifier `verify_nanopub` is the strongest surface, proven on both the happy path (valid signature accepted) and an error/tamper path (mutated TriG rejected) — exactly the dual-sided contract an integrity check needs. The constructor `build_nanopub` is proven only that it emits the expected four-graph TriG structure with correct prefixes; it has no failure, empty-input, or malformed-output proof. The single most actionable gap is `build_nanopub`, since it produces the document whose integrity `verify_nanopub` later attests.

## verify_nanopub
File: `crates/vox-nanopub/src/signing.rs`

Distinct proven behaviors:
- Returns `true` for a correctly signed nanopub document (`sign_and_verify_round_trip`, happy).
- Returns `false` when the TriG document is tampered with after signing (`tampered_trig_fails_verify`, error).

Coverage flags:
- Happy path: yes
- Error/rejection path: yes (tamper rejection)
- Edge/invariant: not separately proven (no empty-doc, missing-signature, or malformed-key cases), but the security-critical accept/reject pair is present.

## build_nanopub
File: `crates/vox-nanopub/src/trig.rs`

Distinct proven behaviors:
- Produces a TriG document containing assertion, provenance, and pubinfo graph sections with proper prefixes (`trig_document_contains_four_graphs`, happy).

Coverage flags:
- Happy path: yes
- Error/rejection path: none
- Edge/invariant: none (no empty-graph, missing-section, or duplicate-prefix proof)

## Semantic gaps

Symbols proven only on the happy path whose contract has an obvious failure/empty/conflict mode:

- **`build_nanopub` (`crates/vox-nanopub/src/trig.rs`)** — Highest priority. A TriG constructor for nanopublications has clear unhappy modes that go untested: empty or missing assertion/provenance/pubinfo content, malformed/invalid input that should be rejected or escaped, and prefix or graph-name conflicts in serialization. Because the document it builds is the exact artifact `verify_nanopub` signs and integrity-checks, any silent structural defect here undermines the whole integrity surface while still passing the lone happy-path test. Add at minimum: an empty/missing-graph rejection or well-defined-output test, and a malformed-input behavior test.

Note: `verify_nanopub` is not a gap — its tamper-rejection test already exercises the critical failure side of its contract.