# Semantic Behavior Map — `vox-http-client`

9 extracted `Behavior` claims collapse to **3 distinct symbols**. Every claim is `kind: happy` / `confidence: EXTRACTED`; there is no error-path, edge, or invariant proof anywhere in this set. The map below groups deduplicated behaviors per symbol and flags each symbol's missing failure mode. Coverage is shallow-but-broad: serialization shape, builder chaining, and middleware construction are all confirmed only under nominal inputs, leaving every rejection/None/failure branch unproven.

## `error_json`
File: `crates/vox-http-client/src/envelope.rs`
Tests: `envelope_serializes_ok_false_and_code` (4 claims, all happy)

Distinct proven behaviors:
- Serializes to a JSON value with `ok = false`.
- Includes the `code` parameter in the output.
- Includes the `message` parameter in the output.
- Includes `request_id` in the output **when `Some` is provided**.

Error path: none. Edge/invariant: none.

The four claims are all facets of one round-trip test over a fully-populated error envelope. The `request_id` claim explicitly covers only the `Some` branch — the `None` branch (omit field, or emit null) is never asserted.

## `client_builder`
File: `crates/vox-http-client/tests/client_builder_chaining.rs`, `client_builder_smoke.rs`
Tests: `client_builder_composes_with_gzip_toggle`, `client_builder_preset_builds_offline` (3 claims, all happy)

Distinct proven behaviors:
- Returns a builder exposing a `gzip` method.
- `gzip(true)` chained then built succeeds without error.
- Default `client_builder()` builds successfully offline.

Error path: none. Edge/invariant: none.

Only success construction is proven. No claim exercises a configuration that should make `build()` fail, nor any non-default toggle interaction beyond `gzip(true)`.

## `populi_control_plane_client`
File: `crates/vox-http-client/tests/middleware_stack_smoke.rs`
Tests: `populi_middleware_client_builds_with_and_without_retry` (2 claims, all happy)

Distinct proven behaviors:
- Accepts a `Client` + `false` (retry off) without panic.
- Accepts a `Client` + `true` (retry on) without panic.

Error path: none. Edge/invariant: none.

Both claims are "does not panic" smoke assertions over the retry boolean. There is no proof that the retry flag changes observable behavior, and no construction-failure path.

## Semantic gaps

Symbols proven **only** on the happy path whose contracts clearly have an unproven failure/empty/conflict mode:

1. **`error_json` — `request_id = None` branch (highest value, integrity surface).** This is an error-envelope constructor whose entire purpose is failure reporting, yet the optional `request_id` is only ever tested as `Some`. The `None` serialization shape (field omitted vs. `null`) is a contract decision with zero coverage. Add a test asserting the `None` output shape.

2. **`client_builder` — no `build()` rejection test (mutator/constructor with no failure path).** A builder that can `build()` can presumably fail to build under bad config; only the offline-success and `gzip(true)` happy paths exist. Add a test that drives a configuration expected to produce an `Err`.

3. **`populi_control_plane_client` — retry semantics unproven (behavioral no-op risk).** The two claims confirm only that passing `true`/`false` doesn't panic; nothing proves the `true` path actually installs retry middleware or that the `false` path omits it. A "does not panic" assertion cannot distinguish a wired flag from a dead one. Add a behavioral test (e.g., a transient-failure mock that succeeds only with retry enabled).