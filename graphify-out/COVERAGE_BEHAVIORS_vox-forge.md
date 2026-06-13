A semantic behavior map for the `vox-forge` crate, synthesized from 6 extracted Behavior claims. All claims originate from `crates/vox-forge/src/provider.rs` and cover a single struct, `ForgeRegistry` (4 distinct symbols). The registry's lookup surface is well-proven (empty invariant, case-insensitive match, and a miss path), but its mutator (`register`) is happy-path only, and the crate's central abstraction — the `GitForgeProvider` async trait — has no behavioral proof at all despite every method declaring a `ForgeError` failure mode.

## ForgeRegistry (construction)
- **Invariant:** A newly created `ForgeRegistry` is empty (`new()`/`Default` start with zero providers).
- Error path: n/a (infallible constructor). Edge/invariant: yes (emptiness invariant proven).

## ForgeRegistry::register
- **Happy:** Registering a forge increments registry length by 1.
- Error path: none. Edge/invariant: none.
- Contract note: `get()` resolves the *first* provider matching a name, so registering two providers with the same name silently shadows the second — an untested conflict mode.

## ForgeRegistry::get
- **Happy:** Returns `Some` for a registered forge name.
- **Happy/miss:** Returns `None` for an unregistered name.
- **Edge:** Lookup is case-insensitive (`"nullforge"` matches `"NullForge"` via `eq_ignore_ascii_case`).
- Error path: n/a (returns `Option`, not `Result`). Edge: yes. This is the best-covered symbol.

## ForgeRegistry::provider_names
- **Happy:** Returns registered provider names in insertion order (proven with a single provider).
- Error path: none. Edge/invariant: weak — ordering claimed but only exercised with one element; multi-provider ordering and same-name behavior untested.

## Semantic gaps

Symbols proven only on the happy path whose contract clearly has a failure, empty, or conflict mode:

1. **`ForgeRegistry::register` — duplicate-name conflict (most actionable).** Proven only as "len +1". Because `get()` returns the first name match, a same-named second provider is silently unreachable. No test asserts what happens on duplicate registration. Add a test registering two same-named providers and pin the resolution + `len` semantics.

2. **`GitForgeProvider` async mutators have zero proof.** `create_change_request`, `update_change_request`, `merge_change_request`, `add_labels`, `create_release`, and `create_discussion_or_issue` all return `Result<_, ForgeError>` and are only ever defined as `NullForge` stubs (returning `Unsupported`) — never asserted by any test. `merge_change_request` is integrity-sensitive (returns a merge SHA) and entirely unproven.

3. **`parse_webhook` is an untested integrity/security surface.** It parses untrusted raw `&[u8]` payloads into a `WebhookEvent`. There is no malformed-payload, unknown-event, or rejection test — exactly the kind of input-parsing boundary that warrants an error-path proof.

4. **`provider_names` ordering under-proven.** The insertion-order guarantee is asserted with only one element; the multi-provider ordering invariant it claims is effectively untested.

The actionable shortlist: a duplicate-registration test for `register`, and at minimum one rejection/error-path test each for `parse_webhook` and a representative mutator (e.g. `merge_change_request`).