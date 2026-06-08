# Semantic Behavior Map — `vox-arch-check`

Deterministically synthesized from 40 distinct proven-behavior claims (of 40 extracted) across 11 symbols. 4 symbols have an explicit error-path proof; **3 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `check_evidence_ledger()`  (edge, error, happy; EXTRACTED)
- [error] When a ledger references a missing artifact, check_evidence_ledger() returns a finding with kind MissingArtifact  (crates/vox-arch-check/src/evidence_ledger.rs)
- [error] When a dated directory has no reports, check_evidence_ledger() flags it with FindingKind::DirectoryHasNoDatedReports  (crates/vox-arch-check/src/evidence_ledger.rs)
- [happy] When a dated directory contains a fresh (today's) dated report, check_evidence_ledger() returns no findings  (crates/vox-arch-check/src/evidence_ledger.rs)
- [error] When a dated report is stale (older than max_age_days), check_evidence_ledger() flags it with FindingKind::Stale  (crates/vox-arch-check/src/evidence_ledger.rs)
- [edge] When an artifact has an unknown kind, check_evidence_ledger() produces a finding with severity level WARN  (crates/vox-arch-check/src/evidence_ledger.rs)
- [error] check_evidence_ledger detects missing artifact files and reports one finding  (crates/vox-arch-check/src/evidence_ledger.rs)
- [error] check_evidence_ledger reports missing artifacts with correct claim_id and MissingArtifact kind  (crates/vox-arch-check/src/evidence_ledger.rs)
- [error] check_evidence_ledger detects empty dated report directories and flags with DirectoryHasNoDatedReports kind  (crates/vox-arch-check/src/evidence_ledger.rs)
- [error] check_evidence_ledger returns a finding with kind=MissingArtifact when an artifact_path referenced in the ledger does not exist on disk  (crates/vox-arch-check/src/evidence_ledger.rs)
- [error] check_evidence_ledger returns a finding with kind=DirectoryHasNoDatedReports when an artifact_kind=directory_with_dated_json directory contains no dated .json files  (crates/vox-arch-check/src/evidence_ledger.rs)

### `scan()`  (happy, invariant; EXTRACTED)
- [happy] scan() detects raw Command::new("git") calls outside vox-vcs-git crate  (crates/vox-arch-check/src/forbidden_patterns.rs)
- [happy] scan() does not flag forbidden patterns in exempt files like vox-vcs-git/src/git_exec.rs  (crates/vox-arch-check/src/forbidden_patterns.rs)
- [happy] When vox-arch-check: allow annotation appears on the preceding line, scan() suppresses the hit  (crates/vox-arch-check/src/forbidden_patterns.rs)
- [happy] When vox-arch-check: allow annotation appears on the same line, scan() suppresses the hit  (crates/vox-arch-check/src/forbidden_patterns.rs)
- [invariant] scan() does not scan non-Rust files (e.g., .toml) under crates directory  (crates/vox-arch-check/src/forbidden_patterns.rs)
- [invariant] scan() does not recurse into target/ directories even if files match the glob pattern  (crates/vox-arch-check/src/forbidden_patterns.rs)

### `compute_key()`  (happy, invariant; EXTRACTED)
- [invariant] compute_key returns the same hash for identical input workspace state  (crates/vox-arch-check/src/cache.rs)
- [invariant] compute_key returns a 64-character hash string (SHA-256 hex format)  (crates/vox-arch-check/src/cache.rs)
- [happy] compute_key returns different hashes when workspace files change  (crates/vox-arch-check/src/cache.rs)
- [invariant] compute_key produces the same 64-character SHA-256 hex string on repeated invocations with identical input files  (crates/vox-arch-check/src/cache.rs)
- [happy] compute_key produces different values when Cargo.lock content changes  (crates/vox-arch-check/src/cache.rs)

### `field_present()`  (edge, error, happy; EXTRACTED)
- [happy] field_present returns true when field name is followed by colon or bullet separator and a value  (crates/vox-arch-check/src/criteria_format.rs)
- [edge] field_present returns false when field name has no value or separator  (crates/vox-arch-check/src/criteria_format.rs)
- [edge] field_present returns false for prose mentions of field names without separator characters  (crates/vox-arch-check/src/criteria_format.rs)
- [happy] field_present returns true only when a backticked field name is followed by a : or · separator and a non-empty value  (crates/vox-arch-check/src/criteria_format.rs)
- [error] field_present returns false for prose mentions of a field name lacking an explicit : or · separator, rejecting such lines as field declarations  (crates/vox-arch-check/src/criteria_format.rs)

### `load()`  (edge, error, happy; EXTRACTED)
- [happy] load retrieves CachedData with matching key and git_touched_paths preserved  (crates/vox-arch-check/src/cache.rs)
- [edge] load returns None when cache file does not exist  (crates/vox-arch-check/src/cache.rs)
- [edge] load returns None when requested key does not match stored key  (crates/vox-arch-check/src/cache.rs)
- [error] load() returns None when no cache file exists at the computed key path  (crates/vox-arch-check/src/cache.rs)
- [error] load() returns None when the cached data's internal key field does not match the requested key, even if a cache file exists  (crates/vox-arch-check/src/cache.rs)

### `check_criteria_format()`  (error, happy; EXTRACTED)
- [happy] check_criteria_format succeeds for criteria doc with prose references alongside proper definitions  (crates/vox-arch-check/src/criteria_format.rs)
- [error] check_criteria_format errors when required fields are missing, even if mentioned in prose  (crates/vox-arch-check/src/criteria_format.rs)

### `split_blocks()`  (edge; EXTRACTED)
- [edge] split_blocks ignores prose references like [CR-F0] mid-sentence and only recognizes line-leading definitions  (crates/vox-arch-check/src/criteria_format.rs)
- [edge] split_blocks correctly identifies only line-leading bold [CR-*] markers as definitions, ignoring mid-sentence prose references and parenthetical mentions  (crates/vox-arch-check/src/criteria_format.rs)

### `test helper function`  (edge; EXTRACTED)
- [edge] make_workspace creates the required directory structure and files for the cache subsystem tests  (crates/vox-arch-check/src/cache.rs)
- [edge] write_ledger correctly constructs and writes a well-formed evidence-ledger.v1.json file to the expected contracts/reports directory  (crates/vox-arch-check/src/evidence_ledger.rs)

### `arch-check binary (--warn-only flag)`  (happy; EXTRACTED)
- [happy] exits with status 0 when run on a clean synthetic workspace  (crates/vox-arch-check/tests/integration.rs)

### `store()`  (happy; EXTRACTED)
- [happy] store persists CachedData with key field intact  (crates/vox-arch-check/src/cache.rs)

### `store() and load()`  (happy; EXTRACTED)
- [happy] CachedData can be stored to disk via store() and successfully retrieved via load() with key and git_touched_paths intact  (crates/vox-arch-check/src/cache.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`arch-check binary (--warn-only flag)`** — only: _exits with status 0 when run on a clean synthetic workspace_
- **`store()`** — only: _store persists CachedData with key field intact_
- **`store() and load()`** — only: _CachedData can be stored to disk via store() and successfully retrieved via load() with key and git_touched_paths intact_
