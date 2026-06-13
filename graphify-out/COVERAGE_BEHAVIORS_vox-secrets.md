## vox-secrets — Semantic Behavior Map

Synthesized from 56 extracted behavior claims spanning 14 distinct symbols across `src/tests.rs`, `tests/vox_vault_tests.rs`, `src/sources/auth_json.rs`, `src/spec/mod.rs`, and `src/backend/vox_vault.rs`. The secret-resolution policy layer is the most thoroughly proven surface (rich error + edge + invariant coverage on `resolve_env_only`, `SecretResolver`, `ResolutionStatus`, and `CutoverPhase`). The cryptographic vault backend and the local token store (`auth_json`) are largely happy-path: their mutators and integrity surfaces lack rejection/failure proofs, which is where the actionable semantic gaps concentrate.

### resolve_env_only
Coverage: happy + edge + invariant.
- Returns `Present` when the canonical env var is set; canonical value wins when both canonical and alias are set.
- Returns `DeprecatedAliasUsed` when only the deprecated alias is set.
- With `include_env=false`, `expose()` on the resolved secret returns `None` (plaintext fallback disabled).
- Fuzz invariant: never panics on random 32-char ASCII payloads; always resolves to `Present` or `DeprecatedAliasUsed`.

### SecretResolver
Coverage: happy + error + invariant.
- Returns `BackendUnavailable` with the unavailability reason in the `detail` field.
- With `include_env=true`, returns `MissingRequired` when the env var is unset, regardless of backend.
- With `include_env=false`, returns `MissingRequired` even when the env var is set.
- `HardCutStrict` profile → `RejectedLegacyAlias` for deprecated aliases.
- `ProdStrict` profile → `RejectedSourcePolicy` for transport secrets resolved from env.

### ResolutionStatus
Coverage: happy + error + invariant.
- Status discriminants proven: `BackendUnavailable`, `DeprecatedAliasUsed`, `RejectedLegacyAlias`, `RejectedSourcePolicy`, `MissingRequired`, `Present`.
- Decommission phase with env_only backend + plaintext key set does NOT return `Present` (forced to vox_cloud requirement).
- Fuzz invariant: only `Present`/`DeprecatedAliasUsed` for arbitrary env values.

### CutoverPhase
Coverage: happy + edge + invariant.
- `legacy_sources_allowed`: true for Shadow/Canary under `DevLenient`; false for Canary under `HardCutStrict`; always false for Enforce and Decommission.
- `force_vox_cloud_backend`: true only for Decommission (Shadow does not force it).
- `from_env()` honors legacy `VOX_SECRETS_MIGRATION_PHASE`, mapping `enforce` → `Enforce`.

### required_for_profile
Coverage: invariant only.
- Requirements differ between Dev and Ci profiles. No per-profile content assertion or unknown-profile behavior.

### requirements_for_profile_mode
Coverage: happy + invariant.
- Dev/Local: `OpenRouterApiKey` in optional set; blocking set is empty.
- Chat/Cloud: at least one AllOf requirement group present.
- Only Dev-Local and Chat-Cloud combinations exercised.

### VoxCloudBackend
Coverage: happy + error (single) + invariant.
- `write_secret` + `resolve` round-trip preserves decrypted plaintext.
- `rewrap_secret` mutates the existing DB row in place; preserves plaintext across API key, bearer token, and password kinds (invariant).
- `export_account_backup` includes seeded rows.
- `import_account_backup` detects corrupted ciphertext via checksum-mismatch error (the one error-path proof).

### write_registry_token (sources/auth_json.rs)
Coverage: happy only.
- Returns the auth-file path after writing; makes the token readable via `read_registry_token`.

### read_registry_token (sources/auth_json.rs)
Coverage: happy only.
- Retrieves the exact token previously written; returns `None` after removal.

### remove_registry_token (sources/auth_json.rs)
Coverage: happy + invariant.
- Reports success removing an existing entry; subsequent read returns `None`; idempotent across repeated calls.

### SecretId / ALL_REGISTRIES (spec/mod.rs)
Coverage: invariant only.
- Every `SecretId` appears in `ALL_REGISTRIES` at most once (no runtime lookup ambiguity).

### is_windows_absolute_path() (backend/vox_vault.rs)
Coverage: happy + edge (well-proven validator).
- Accepts drive-letter + slash (`C:/foo`, `d:/foo`, `Z:/x`).
- Rejects: drive letter without slash (`C:foo`), POSIX absolute (`/foo`), relative (`foo/bar`), non-alpha first char (`1:/foo`), empty string.

## Semantic gaps

Symbols proven only on the happy path (or only invariant) whose contract clearly has a failure/empty/conflict mode:

- **`write_registry_token` (mutator, no failure path).** Writes to `auth.json` but no test for an unwritable file, malformed pre-existing JSON, or disk error. Highest-value gap — a security-relevant write surface with zero rejection coverage.
- **`read_registry_token` (no parse-failure path).** Only happy + post-delete-`None`. No proof of behavior on a corrupted/malformed `auth.json` (does it error or silently return `None`?).
- **`remove_registry_token` (no failure path).** Idempotence proven, but no unwritable-file / partial-write failure proof.
- **`VoxCloudBackend::rewrap_secret` (mutator, no failure path).** Key-rotation surface proven only happy/invariant — no rewrap of a missing row, wrong/old key, or double-rewrap.
- **`VoxCloudBackend::write_secret` / `export_account_backup` (no failure/empty path).** No duplicate-id write, locked/corrupt DB, read-only target, or empty-account export.
- **`requirements_for_profile_mode` (partial profile matrix).** Ci and Prod profiles and the rejecting/empty-requirement combinations are unproven — this is the policy validator that gates blocking secrets.
- **`required_for_profile` (invariant-only).** Only "Dev ≠ Ci" is proven; no per-profile content or unknown-profile handling.
- **`SecretId` / `ALL_REGISTRIES` (lookup-miss unproven).** Uniqueness is proven, but resolution of an unregistered/missing `SecretId` has no test.

Most actionable: the `auth_json` token mutators (`write`/`read`/`remove`) — a local credential store with no rejection or corruption-handling proof — and `VoxCloudBackend`'s mutators (`write_secret`, `rewrap_secret`, `export_account_backup`), where only `import_account_backup` has an integrity/error proof.