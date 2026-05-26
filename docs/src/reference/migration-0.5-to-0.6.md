---
title: "Migration: Vox 0.5 → 0.6"
description: "Language-surface and stdlib changes between Vox 0.5 and 0.6, with mechanical migration recipes."
category: "Language Reference"
status: "current"
last_updated: "2026-05-26"
training_eligible: true
schema_type: "TechArticle"
---

# Migration: Vox 0.5 → 0.6

This guide lists every language-surface change between Vox `0.5.x` and `0.6.0` that requires action in user code, in dependency order. For per-PR detail see [`CHANGELOG.md`](../../../CHANGELOG.md) `[0.6.0]`; for the architectural backdrop see [the v0.6 acceptance suite](../architecture/post-sprint-forward-plan-2026-05-25.md) and the [stdlib gap audit](../architecture/vox-stdlib-gap-audit-2026-05-23.md).

## Removed surface

### `@endpoint(kind: …)` decorator → bare-form decorators

**Status:** removed in v0.6.0 (`vox-stdlib-gap-audit-2026-05-23.md §Phase H step 18`).

The `@endpoint(kind: server|query|mutation)` decorator no longer lexes. The
canonical bare-form decorators `@server`, `@query`, `@mutation` — introduced
in Phase B (audit doc §11.2, 2026-05-23) — are now the only endpoint
declaration surface. All three produce the same `Decl::Endpoint` AST node;
nothing changed in HIR, codegen, or runtime semantics.

| Retired (≤ v0.5)                       | Canonical (v0.6+) |
|----------------------------------------|-------------------|
| `@endpoint(kind: server) fn h(...) {}` | `@server fn h(...) {}`   |
| `@endpoint(kind: query) fn h(...) {}`  | `@query fn h(...) {}`    |
| `@endpoint(kind: mutation) fn h(...) {}` | `@mutation fn h(...) {}` |

**Migration recipe** (mechanical):

```diff
- @endpoint(kind: server)
+ @server
  fn list_items() to List[Item] {
      return db.Item.all()
  }
```

The `retired/decorator-usage` lint runs before the parser and surfaces a `Severity::Error` finding with the suggested replacement, so any remaining `@endpoint` text in your code is flagged at audit time with the friendly migration message rather than at parse time as an unknown token.

### `Json::get_str` / `Json::get_int` → strict-Option `Json` API

**Status:** retired in v0.6.0 (Phase 1 — P1-T2 in the mesh SSOT).

The legacy fallible-`Option` `Json` accessors returned `Option[Str]` / `Option[Int]` but discarded the path being looked up. The new strict-Option API has two layers:

- **Path navigation**: `get(field: Str)`, `at(index: Int)`, `pointer(rfc6901: Str)` — all return `Option[Json]`.
- **Type coercion**: `as_str()`, `as_int()`, `as_float()`, `as_bool()`, `has(field: Str)`.

**Migration recipe:**

```diff
- let name: Option[Str] = response.get_str("name")
+ let name: Option[Str] = response.get("name")?.as_str()

- let count: Option[Int] = response.get_int("count")
+ let count: Option[Int] = response.get("count")?.as_int()
```

For nested paths use `pointer` (RFC 6901):

```vox
// vox:skip
let username = response.pointer("/user/profile/username")?.as_str()
```

## New surface

### `@json_as(MyType)` typed JSON deserialization (Phase M)

**Status:** added in v0.6.0.

`@json_as(MyType)` annotates a `type` declaration; the compiler synthesises
`from_json` and `to_json` HirFns on the type so callers can deserialise
incoming JSON straight into the user-defined shape instead of threading
`Json` through inference.

```vox
@json_as(User)
type User {
    id: Int
    name: Str
}
```

Additional field-level attributes for use inside the annotated type:

- `@field_name("snake_case_name")` — JSON name override
- `@default(value)` — default if the field is missing
- `@skip_if_none` — omit the field from serialization when `None`

See [RFC `json-as-rfc-2026-05-24`](../architecture/rfc-json-as-2026-05-24.md) for the design rationale.

### Raw strings `r"…"` and hash-padded `r#"…"#`

**Status:** added in v0.6.0 (`docs/src/architecture/v0.6-core-ssot.md` raw-string subsection).

Raw string literals do not process escape sequences. Hash-padded form `r#"…"#` lets the literal contain `"` characters.

```vox
// vox:skip
let path = r"C:\Users\me\Documents"
let regex = r#"^([a-z]+):"([^"]+)"$"#
```

### Intra-project imports (`import "./relative.vox"`)

**Status:** added in v0.6.0 (Phase 1 — P1-T1).

`import "./foo.vox"` resolves a sibling `.vox` file at runtime under
`vox run --mode interp`. Combined with `pub fn`, this gives Vox a real
module system without the build-system overhead.

```vox
// vox:skip
// src/util.vox
pub fn slugify(s: Str) to Str {
    return s.to_lower().replace(" ", "-")
}

// src/main.vox
import "./util.vox"
fn main() {
    print(slugify("Hello World"))   // -> "hello-world"
}
```

**Caveat:** `--mode script` parity and full typecheck of imported symbols are tracked as v0.7 work; the runtime resolver in `--mode interp` is the v0.6 deliverable. (The example above is `// vox:skip`'d because cross-file typecheck of imported `Str` differs from in-file lookup as of v0.6.)

## Changed semantics

### Typed list subscript: `list[i]` returns `Option[T]`

**Status:** changed in v0.6.0 (was: bounds-panic in v0.5).

Indexing a typed list with `[i]` previously evaluated to `T` and panicked at runtime when `i` was out of range. v0.6 makes the operation total: `list[i]` returns `Option[T]`, forcing callers to handle the missing case.

```diff
- let first: Item = items[0]               // panics if items is empty
+ let first: Option[Item] = items[0]       // forces explicit handling
+ if let Some(item) = first {
+     // ...
+ }
```

The `// vox:skip` comments in the diff snippets above mark them as illustrative migrations rather than parseable modules; the surrounding text describes the semantics.

This change is consistent with the strict-Option `Json` API and with `dict[k]` semantics elsewhere in the stdlib.

## Workspace / runtime changes

These don't affect Vox-language source but matter for tool/IDE/integrator code:

| Area | Change |
|------|--------|
| Workspace version | `0.5.0` → `0.6.0`. All crates inherit via `[workspace.package]`. |
| `vox-db` schema baseline | `BASELINE_VERSION = 67` (all P3-T1 schema migrations applied). |
| Dashboard route convention | `/api/v2/<surface>` REST + `/v1/ws` topic-multiplex WS (mesh SSOT §5.6). |
| Unified Task Hopper | New `InMemoryHopper` in `vox-orchestrator` (Hp-T1..T4); HTTP intake at `/api/v2/hopper/*`. |
| `PrioritySource` enum | `Developer(2) > Orchestrator(1) > LearningPolicy(0)`; dominance via `Ord` (Hp-T3). |
| `DeveloperOverride` token | `reprioritize` now requires a capability token minted by `DeveloperOverrideMint` (Hp-T4). |
| ACI envelope default | `OrchestratorConfig::agentos_aci_envelope_enabled` defaults to `true` (CR-L5). |
| Effect-row enum | Adds `GpuCompute` and `Mutate` variants (P1-T6; prerequisite for MENS distributed training). |
| Telemetry unification (A–D) | New `vox-telemetry` L1 facade with `TelemetryEvent` / `TelemetryRecorder`; `VOX_TELEMETRY` master switch; `vox telemetry doctor`. |

## Related

- [`CHANGELOG.md`](../../../CHANGELOG.md) — release-level diff.
- [`ref-decorators.md`](./ref-decorators.md) — current decorator registry.
- [`vox-stdlib-gap-audit-2026-05-23.md §Phase H`](../architecture/vox-stdlib-gap-audit-2026-05-23.md) — the soak-gated retirement procedure for `@endpoint`.
- [`session-handoff-2026-05-25-finalization-pass.md`](../architecture/session-handoff-2026-05-25-finalization-pass.md) — v0.6 acceptance criteria, fully green at tag time.
