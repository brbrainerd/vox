# Language features blocking vox-mental-tracker — Index

This index ties together the language/compiler plans surfaced during vox-mental-tracker development. Each linked plan is independently executable and has its own task list. They are in the order I'd recommend tackling them; later plans assume earlier ones have landed.

| # | Plan | Unblocks | Why first |
|---|---|---|---|
| 1 | [struct types](./2026-05-08-language-struct-types.md) — [landed in #73](https://github.com/vox-foundation/vox/pull/73) | App Phase 2 voice flow (single `parse_voice` endpoint returning `ParsedVoice`); any future endpoint with a structured return | Foundational — `JSON parse` returns into either a generic value or a typed struct, and `ts-source-ffi` benefits from struct-shaped param types. |
| 2 | [JSON parse + access stdlib](./2026-05-08-language-json-stdlib.md) — [landed in #76](https://github.com/vox-foundation/vox/pull/76) | App Phase 2 (consume parser output); App Phase 4 (export bundle assembly) | Needed by app Phase 2; depends on nothing else. Pairs naturally with structs (D1 of struct plan does the typed wrapping). |
| 3 | [match-arm statement bodies](./2026-05-08-language-match-arm-statements.md) — [landed in #74](https://github.com/vox-foundation/vox/pull/74) | Ergonomic everywhere; called out in the tracker code today | Tiny, ergonomic, removes repeated workarounds. Land any time. |
| 4 | [string utilities (split / slice / char_at / index_of / starts_with / ends_with)](./2026-05-08-language-string-utils.md) — [landed in #72](https://github.com/vox-foundation/vox/pull/72) | Ad-hoc parsing, content sniffing, future Phase 4 export glue | Tiny pure additions; land any time. |
| 5 | [regex stdlib](./2026-05-08-language-regex-stdlib.md) — [landed in #75](https://github.com/vox-foundation/vox/pull/75) | App Phase 2 parser parity (Vox `preview_voice_parse` matches the TS `intent_parser`'s richer extraction) | After string utils so the test fixtures share infrastructure. |
| 6 | [TS-source FFI from Vox components](./2026-05-08-language-ts-source-ffi.md) — [foundation in #77](https://github.com/vox-foundation/vox/pull/77); codegen TBD | UI consumption of `src/ts/materializer.ts` (WeeklyPage / TimelinePage rendering true materialized rollups) | After structs (5) so extern signatures can use rich types. |

## App phases and which plans they need

| App phase (per [PR #70 plan](https://github.com/vox-foundation/vox/pull/70)) | Required language plans |
|---|---|
| Phase 1 — Domain hardening + materialization | None (TS materializer landed; Vox per-kind aggregator landed) |
| Phase 2 — Voice E2E (parser/confirm/edit/save loop) | (1) structs, (2) JSON, (3) match-arm stmts, (5) regex (for parser parity) |
| Phase 3 — Native STT parity | None language-side; depends on platform [PR #68](https://github.com/vox-foundation/vox/pull/68) Phase 2 |
| Phase 4 — Clinician-grade export completion | (2) JSON, (4) string utils, (6) TS-source FFI (for materializer in HTML render) |
| Phase 5 — Hourglass verification + CI lanes | None |
| Phase 6 — Release-readiness gate | None |

## Out of scope here, but tracked

- **Pattern matching on structs** — once (1) lands, useful follow-up.
- **Generic functions over structs** — same.
- **npm-package imports for TS FFI** — once (6) lands, scope-creeps to `@scope/pkg` imports.
- **Rust-source FFI** — mirror of (6) for `@server fn` contexts. Not needed by the tracker but symmetrical.
- **JSON Pointer / JSONPath traversal** — convenience layer on top of (2).
- **`std.json.parse_typed[T](s) to Result[T]`** — depends on (1); auto-validates parsed JSON against a struct shape.

## How to execute

Each plan has its own task list with `- [ ]` checkboxes. Spawn an executing-plans subagent per plan (or use subagent-driven-development for parallel execution where possible). Plans (3) and (4) and (5) are independent of each other and of (1)/(2); plan (6) depends on (1).

Land each plan as its own PR off `main`. The tracker app PR ([#70](https://github.com/vox-foundation/vox/pull/70)) consumes them as they merge — once (1) and (2) are in, the app's Phase 2 voice flow can drop the extractor-stub pattern entirely and ship the clean version.
