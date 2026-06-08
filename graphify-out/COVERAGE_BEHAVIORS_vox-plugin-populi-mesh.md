# Semantic Behavior Map — vox-plugin-populi-mesh

Synthesized from 8 extracted Behavior claims (post-dedup) covering 3 symbols in the transport layer (`envelope.rs`, `router.rs`). Two symbols — `SignedA2AEnvelope::verify_self_signed` and the `bootstrap_exchange` handler — are robustly characterized with happy, error, edge, and invariant coverage. The third, `populi_http_app_with_auth` router registration, is proven only on route topology (presence and legacy-route absence) and lacks any authentication-rejection proof despite its `_with_auth` contract — the single actionable semantic gap.

## SignedA2AEnvelope::verify_self_signed
File: `crates/vox-plugin-populi-mesh/src/transport/envelope.rs`

Proven behaviors:
- **Error (tamper):** payload modified after signing → `SignatureMismatch` error (`tampered_payload_fails`).
- **Error (version):** envelope version ≠ 1 → `UnsupportedVersion` error carrying the actual version (`wrong_version_fails`).

Coverage: error-path ✅, edge/invariant ✅ (version-boundary check), happy-path not separately asserted but implied. Well-covered integrity surface — both the cryptographic-mismatch and the version-gate rejection modes are tested.

## bootstrap_exchange (handler + endpoint)
File: `crates/vox-plugin-populi-mesh/src/transport/router.rs`

Proven behaviors:
- **Happy:** correct token → `200 OK` with `BootstrapExchangeResponse` containing `mesh_token` (`bootstrap_exchange_round_trip`).
- **Invariant:** tokens are one-time use — replaying the same token → `410 GONE` (`bootstrap_exchange_round_trip`).
- **Error:** wrong token → `401 UNAUTHORIZED` (`bootstrap_exchange_wrong_token_is_401`).
- **Edge:** bootstrap not configured → POST to `/v1/populi/bootstrap/exchange` returns `404 NOT_FOUND` (`bootstrap_exchange_disabled_returns_404`).

Coverage: happy ✅, error ✅, edge ✅, invariant ✅. Fully characterized — success, single-use consumption, credential rejection, and disabled-feature availability are all covered.

## populi_http_app_with_auth (router registration)
File: `crates/vox-plugin-populi-mesh/src/transport/router.rs`

Proven behaviors:
- **Happy:** router includes `GET /v1/populi/nodes` returning `200 OK` (`populi_routes_exist_and_legacy_mens_routes_are_absent`).
- **Edge:** router does NOT include legacy `/v1/mens/nodes` → `404 NOT_FOUND` (same test).

Coverage: happy ✅, edge ✅ (route absence), error/auth ❌. The route-topology contract is verified, but the security contract implied by `_with_auth` is not.

## Semantic gaps

- **`populi_http_app_with_auth` — no auth-rejection proof (security surface).** The constructor name asserts an authentication layer, yet every claim about it concerns route presence/absence only. The `200 OK` on `/v1/populi/nodes` appears to be exercised on the happy path with valid (or absent) credentials; there is no test proving that a missing or invalid credential against an authenticated populi route yields `401`/`403`. This is the most actionable gap: an integrity/security surface proven only on its routing skeleton, with its core rejection mode untested. A regression that silently dropped the auth middleware would still pass the existing route-topology test.

All other symbols have their failure/empty/conflict modes covered (`verify_self_signed`: tamper + version; `bootstrap_exchange`: wrong-token, replay, disabled).