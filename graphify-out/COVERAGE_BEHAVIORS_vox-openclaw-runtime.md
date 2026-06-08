# Semantic Behavior Map — `vox-openclaw-runtime`

## Summary

This map is synthesized from 15 EXTRACTED `Behavior` claims. After deduplication they describe **5 symbols** across two files: `openclaw_discovery.rs` (`clamp_ttl`, `derive_default_well_known_url`, `fallback_endpoints`) and `openclaw_protocol.rs` (`InboundFrame`, `GatewayRequest`). Every claim is `kind: happy` — **no symbol has any error-path proof, and only `clamp_ttl` exercises edge/boundary behavior.** The proven surface is the cheap pure helpers and the optimistic parse of well-formed fixtures; the load-bearing discovery resolver (cache, fallback, precedence) and the rejection behavior of the two protocol deserializers are entirely unproven.

## Per-symbol behaviors

### `clamp_ttl` (`openclaw_discovery.rs`)
Proven behaviors:
- Clamps values below `MIN_CACHE_TTL_SECONDS` (30) up to the minimum.
- Clamps values above `MAX_CACHE_TTL_SECONDS` (86,400) down to the maximum.
- Returns `DEFAULT_CACHE_TTL_SECONDS` (300) when passed `None`.

Error-path proof: n/a (infallible). Edge/invariant proof: **yes** — both clamp boundaries plus the `None` default. Gap: the in-range identity case (value already within bounds) is not asserted.

### `derive_default_well_known_url` (`openclaw_discovery.rs`)
Proven behaviors:
- Appends `/.well-known/openclaw.json` to the base URL (trailing slash collapsed).

Error-path proof: n/a. Edge proof: **no** — only the trailing-slash input is covered; the implementation does `trim_end_matches('/')`, so the no-slash variant is a distinct untested branch.

### `fallback_endpoints` (`openclaw_discovery.rs`)
Proven behaviors:
- Returns `http_gateway_url == DEFAULT_HTTP_GATEWAY_URL`.
- Returns `ws_gateway_url == DEFAULT_WS_GATEWAY_URL`.
- `catalog_list_url` contains `http://127.0.0.1:3000/v1/skills`.
- `catalog_search_url` contains `http://127.0.0.1:3000/v1/skills/search`.

Error-path proof: n/a. Edge/invariant proof: **no**. This is the last-resort safety bundle, but no claim proves it is *selected* on the fetch-failure path.

### `InboundFrame` (`openclaw_protocol.rs`)
Proven behaviors:
- Deserializes JSON with `type=event`, `event=connect.challenge`.
- The `Event` variant extracts `event == connect.challenge`.
- The `Event` variant extracts `nonce == "abc"` from the payload.

Error-path proof: **no**. Edge/invariant proof: **no**. Proven only on one well-formed event.

### `GatewayRequest` (`openclaw_protocol.rs`)
Proven behaviors:
- Deserializes the operator-protocol `connect` request fixture.
- `frame_type == "req"`.
- `method == "connect"`.
- `params` contains the `client` key.

Error-path proof: **no**. Edge/invariant proof: **no**. Proven only on one well-formed request fixture.

## Semantic gaps

These are symbols whose contract has a clear failure / empty / conflict mode that is **not** proven, ordered by actionability.

1. **`resolve_openclaw_endpoints` — no claims at all (highest priority).** This is the orchestrator and the real integrity surface: it owns cache-hit-vs-expiry, last-known-good fallback on fetch error, and env/override precedence via `apply_precedence`. None of these branches has a single behavior claim. The whole point of the module (resilient resolution) is unverified.

2. **`fetch_well_known` — no claims at all.** Has an explicit non-2xx HTTP rejection (`OpenClaw discovery HTTP {status}`), a JSON parse-failure path, and partial-document field-merge logic. All untested.

3. **`InboundFrame` — validator/deserializer with no rejection test.** No proof it rejects (or how it handles) an unknown `type`, a missing `event` field, or a malformed payload. A protocol boundary that only ever sees valid input in tests.

4. **`GatewayRequest` — validator/deserializer with no rejection test.** No proof of behavior on wrong `frame_type`, missing `method`, or absent `params`. Same boundary risk as `InboundFrame`.

5. **`fallback_endpoints` selection unproven.** Field population is proven, but its role as the failure-mode bundle is only meaningful if `resolve_openclaw_endpoints` actually returns it on error — which gap #1 leaves unverified.

6. **Pure-helper edge holes (low severity):** `clamp_ttl` in-range identity case and `derive_default_well_known_url` no-trailing-slash case are distinct untested branches.