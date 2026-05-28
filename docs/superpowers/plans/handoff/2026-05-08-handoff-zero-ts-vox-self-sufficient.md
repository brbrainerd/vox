# Handoff — "Zero hand-written TS" — make Vox self-sufficient for full app emission

> **For agentic workers:** REQUIRED SUB-SKILLS: superpowers:writing-plans (review only — this is a meta-plan over multiple smaller plans), superpowers:test-driven-development (per-bug TDD; every gap below should land with a golden snapshot), superpowers:verification-before-completion, superpowers:requesting-code-review.

This document is a forward-looking handoff. Read it end-to-end before deciding scope. The bug-by-bug fixes here are the next-shoe-to-drop after [PR #78's plan](../language/2026-05-08-codegen-ts-bugs-blocking-tracker.md) (4 bugs landed) and [PR #79's plan](2026-05-08-handoff-path-c-vox-owns-toolchain.md) (toolchain ownership, separate track).

---

## 0. The vision

A complete Vox app should require **zero hand-written TypeScript**. Today, vox-mental-tracker ([PR #70](https://github.com/vox-foundation/vox/pull/70)) ships ~600 lines of hand-written `.ts/.tsx` to compensate for codegen gaps:

| File | Why it exists today | Where it should live |
|---|---|---|
| `src/main.tsx` | Mount React + custom router consuming `routes.manifest.ts`. | Vox-emitted entry. |
| `src/runtime.ts` | Globals (`str`, `len`, `Speech`, `std.*`, `mobile`) the codegen references but doesn't import. | A `@vox/runtime` package the codegen imports automatically — or inline-replaced at emit time. |
| `src/ts/materializer.ts` | Pure-function correction-collapse + grouping + weekly aggregate. | `apps/vox-mental-tracker/src/lib/materializer.vox` — pure Vox functions, with TS emitted by the compiler. |
| `src/ts/export_pipeline.ts` | Composes materializer + CSV + WebCrypto + HTML render. | Same — Vox functions, async/await modeled by the compiler. |
| `src/ts/export_contract.ts` | Row projection + CSV building + SHA-256. | Same. |
| `src/ts/intent_parser.ts` | Regex-based intent classifier. | Already Vox-side now (`parse_voice` uses `std.regex`); the TS file is a fixture-shared parity check that can be emitted from the same Vox source. |
| `index.html` | Vite entry shell. | Vox-emitted (the compiler already generates SSG shells under `target/generated/public/ssg-shells/`; merge that with the bundled-app entry). |
| `vite.config.ts`, `tsconfig.json`, `package.json` Vite/React/zod deps | Build tooling glue. | Goes away once [Path C](2026-05-08-handoff-path-c-vox-owns-toolchain.md) lands — `vox build` emits the deployable bundle. |
| `scripts/postbuild-fixup.mjs` | sed-style patches for codegen-emitted code. Bandaids per Section 2 below. | Disappears once those codegen gaps close. |

When this handoff is complete, the tracker app's source tree contains: `src/main.vox`, `src/lib/*.vox`, `tests/**/*.vox` (or vitest if test infra stays), `Vox.toml`, `capacitor.config.ts`, and that's it. No TS at all.

---

## 1. Codegen capabilities — bugs & gaps that block "zero TS"

These are concrete defects discovered while bringing PR #70 from "Vox checks clean" to "actually renders in a browser." They divide into hard bugs (broken output) and missing capabilities (correct emit but doesn't reach the surface a Vox author wants).

### 1.A — Hard bugs (output is structurally wrong)

#### 1.A.1 — Handler bodies wrap an expression in a never-invoked outer arrow

**Symptom:** `<button onClick={() => { (() => (X)); }}>` — the outer `(() => (X))` is a function expression, not invoked; the side effects inside X never run. Workaround in `apps/vox-mental-tracker/scripts/postbuild-fixup.mjs` regex-inserts the missing `()`.

**Where:** `crates/vox-compiler/src/codegen_ts/component.rs` (or wherever component event-handler bodies lower). The codegen emits `(() => (X));` when the handler body is an expression (e.g. a `match`); should emit `(X)` directly, or `(() => (X))()` if a function wrap is genuinely needed.

**Test:** golden snapshot covering `onClick={fn() { match foo() { Ok(x) => ..., Error(e) => ... } }}` — the emitted handler must call `match` semantics with bound vars and observable side effects.

#### 1.A.2 — Async `@endpoint` calls in handlers don't `await`; result is `Promise`, fields read `undefined`

**Symptom:** In a handler body, `let p = parse_voice(t); set_parsed_kind(p.kind)`. `parse_voice` is an `@endpoint(kind: query)` lowered to a fetch in `vox-client.ts` — returns `Promise<ParsedVoice>`. The emit doesn't `await`, so `p` is a Promise and `p.kind` is `undefined`. State updates with `undefined`.

**Where:** Same area as 1.A.1. The compiler must:
- Mark click handlers `async` when their body calls any async function.
- Insert `await` before every async-typed expression.
- The HIR already knows which functions are async (the `@endpoint(kind: query|mutation)` annotation flags them); thread that through to the lowering.

**Test:** golden — handler that `let r = some_endpoint(); use(r.field)` — emit must `await some_endpoint()` and the handler arrow must be `async`.

#### 1.A.3 — `.length()` method form vs `.length` property

**Symptom:** Vox `s.length()` (or `list.length()`) lowers to `s.length()` in TS — but JS strings/arrays expose `length` as a property. Calling it like a function throws `TypeError: s.length is not a function`. Workaround: postbuild-fixup regex-replaces `.length()` with `.length`.

**Where:** `codegen_ts` method-call lowering for `Str` / `List`. A small mapping table: `length` → emit as property access, not method call.

**Test:** golden covering `let n = s.length()` and `let m = items.length()`.

#### 1.A.4 — Vite production minifier dead-code-eliminates `(() => (X));` patterns

**Symptom:** Even with 1.A.1 worked around in source, esbuild's production minifier sees `(() => (X));` (unused expression statement at top of arrow) and elides it. We disabled minify entirely in `vite.config.ts` as a stopgap (`minify: false`).

**Where:** Same as 1.A.1 — the underlying fix is to emit invoked handlers, not wrapped-but-uninvoked arrows. Once 1.A.1 lands, minify can be re-enabled.

**Test:** add `minify: 'esbuild'` to the golden snapshot's vite-build harness; assert handler side effects still execute under production minification.

### 1.B — Missing imports / symbol resolution

#### 1.B.1 — Bare references to runtime helpers

**Symptom:** `dist/Home.tsx` and others reference `str(...)`, `len(...)`, `Speech.transcribe_microphone()`, `std.time.now_ms()`, `std.crypto.uuid()`, `std.regex.compile(...)`, `std.json.parse(...)` without an `import` statement. Authored `src/runtime.ts` shim installs them on `globalThis` — a hack that only works because TS's nominal types don't object to global lookups.

**Where:** Per-emitted-file import collector, parallel to Bug D (which fixed `@endpoint` imports). Walk the file, list every reference to a stdlib namespace or builtin, emit the import:
```ts
import { str, len } from "@vox/runtime/builtins";
import { Speech } from "@vox/runtime/speech";
import { time as voxTime, crypto as voxCrypto, regex as voxRegex, json as voxJson } from "@vox/runtime/std";
```

The `@vox/runtime` package then ships these implementations once, in TS — published from the workspace, not hand-written per app.

**Companion work:** publish `@vox/runtime` as part of the compiler's release artifacts. Until [Path C](2026-05-08-handoff-path-c-vox-owns-toolchain.md) lands, this is an npm-installable package; after, it becomes part of the bundled output.

**Test:** golden — minimal Vox file that uses `str`, `len`, `std.time.now_ms()`. Emit must contain the right imports; `tsc --noEmit` over a stub `@vox/runtime` package must succeed.

#### 1.B.2 — `mobile.*` alias for Speech

The current codegen lowers some `Speech.method()` calls to `mobile.method()` (probably a vestige of an earlier rename). Hand-written runtime aliases `mobile = Speech`. Once 1.B.1 lands properly, this aliasing should be unnecessary — pick one canonical name (`Speech`) and emit consistently.

### 1.C — Async handler dispatch & state mutation semantics

#### 1.C.1 — `state foo: T = ...; foo = expr` in handlers

Vox handlers can mutate state directly: `state count: int = 0; ... button(on_click={fn() count = count + 1})`. The codegen emits `set_count(count + 1)` — correct, BUT in handler bodies that include async `await`s, the closure captures the OLD value of `count`. After `await`, `count` is stale; React's batching may also coalesce updates oddly.

**Where:** When emitting an `await` in a handler body, refresh captured state via `useRef` + a getter, or restructure the closure to read state freshly. This is a non-trivial codegen change.

**Test:** golden — handler that does `let r = await endpoint(); count = count + r`; assert post-update count is the freshly-read value.

#### 1.C.2 — Multi-statement `match` arm bodies in handlers

**Status:** Match-arm statement bodies plan ([PR #74](https://github.com/vox-foundation/vox/pull/74)) covers parser + lowering. The codegen needs the matching emit for handler context: each arm's statements run sequentially, with `await` propagation. Verify the existing emit is correct under handlers; add goldens if not.

### 1.D — JSX / event types

#### 1.D.1 — `e: any` typing on synthetic events

The codegen emits handlers with no parameter (`fn()`) — fine for buttons, less fine for inputs (`<input on_change={fn(e) ...}>`). The Vox AST has no notion of synthetic events, so handler params can't be typed. Either:
- Inject `e: React.ChangeEvent<HTMLInputElement>` (etc.) when the surrounding element + event combo is known.
- Or: add `event` shape to Vox stdlib.

**Test:** golden covering `<input on_change={fn(e) some_state = e.target.value}>`.

### 1.E — Tree-shaking robustness

The minifier elides patterns it considers pure expressions. The codegen should emit handler bodies in shapes that minifiers always preserve: a function call statement at top, no arrow-wrapping. Land 1.A.1 + add a CI snapshot that runs each golden through `vite build --minify=esbuild` and verifies handlers still fire.

---

## 2. Runtime / `@endpoint` execution model on mobile

Today's `@endpoint` calls go via `vox-client.ts` to a fetch URL backed by an Axum Rust server (also generated by `vox build` under `target/generated/`). For a Capacitor mobile app, there's no network server — the WebView runs offline.

Three options to close this gap; the plan picks one:

**Option A — Local SQLite via Capacitor plugin.** Replace `vox-client.ts`'s fetch calls with calls to a Capacitor plugin that runs the @endpoint logic locally (against `@capacitor-community/sqlite` or similar). The codegen emits both the server (Rust/Axum) and the client (TS via plugin) from the same `@endpoint` function. The client variant inlines the SQL the server would have produced.

**Option B — Embedded Rust server via FFI.** Bundle the generated Rust server as a static library, call it via Capacitor's native bridge (JNI on Android, Swift bridging on iOS). Keeps the `@endpoint` semantics identical between dev + mobile.

**Option C — Hybrid: web-app calls the server in dev, the bundled FFI lib in Capacitor.** Codegen emits a transport layer that picks based on `Capacitor.isNativePlatform()`.

**Recommendation:** **A** for v1 (pure-JS path, no FFI complexity). B / C are follow-ups.

Whichever option ships, the codegen must:
- Emit a local-execution branch in `vox-client.ts` (or a sibling) for each `@endpoint`.
- Include a typed schema-validation layer (the existing `zod` integration is healthy — keep it).
- Support `kind: server` (server-only) endpoints with a clear-error fallback when called from a Capacitor build.

**Test:** golden — minimal `@endpoint(kind: query) fn ping() to str { return "pong" }`. Emit a TS variant that runs locally; Capacitor build smoke test verifies it returns "pong" without a network round-trip.

---

## 3. Code authored in TS today that should be Vox

Per the table in §0, three TS files (`materializer.ts`, `export_pipeline.ts`, `export_contract.ts`) implement pure-function logic that Vox is fully capable of expressing today. The barrier is just author convenience: writing it in Vox would have meant relying on capabilities that hadn't landed yet (struct types, std.json, std.regex). Those landed in [PRs #71-77](https://github.com/vox-foundation/vox/pulls?q=is%3Apr+is%3Aclosed+72..77).

The migration work:
- **3.A.** Rewrite `src/ts/materializer.ts` as `src/lib/materializer.vox` exposing the same three functions (`resolveCorrections`, `groupByDay`, `weeklyAggregate`) as `@endpoint(kind: query)` or pure top-level `fn`s.
- **3.B.** Rewrite `src/ts/export_contract.ts` as `src/lib/export_contract.vox` (CSV row projection, SHA-256 hash, `is_backdated` calc).
- **3.C.** Rewrite `src/ts/export_pipeline.ts` as `src/lib/export_pipeline.vox` composing the above.
- **3.D.** Update vitest fixtures to drive the **emitted TS** from Vox (not the hand-written one). The 34 existing test cases stay intact; they just point at `dist/lib/materializer.js` etc.
- **3.E.** Delete the TS files when the migration is complete and tests pass.

This is straightforward porting work — no compiler change needed once §1 + §2 land. Track as a Phase 7 of the app's plan series.

---

## 4. Toolchain ownership (cross-reference)

[PR #79's Path C plan](2026-05-08-handoff-path-c-vox-owns-toolchain.md) covers the build-tooling layer separately:
- A: `--target=browser-esm` JS emission
- B: `vox serve` static dev server
- C: vendored ESM cache
- D: `--target=browser-bundle` tree-shake/mangle/hash
- E: source maps
- F: tracker migration (drops Vite)
- G: docs

The §1 work in *this* plan is **prerequisite** to Path C: a bundle that doesn't run in the browser today won't run in `vox serve` either. The two plans interleave:

```
§1.A (bug fixes)  ──┐
                    ├─→ Path C Phase A (browser-esm emission)
§1.B (imports)    ──┤
                    │
§1.C (async)      ──┘
                                                ↓
§2 (endpoint exec model)  ──→  Path C Phase F (tracker migration)
                                                ↓
§3 (TS-to-Vox migration)  ──────────────────────┘
```

---

## 5. Out of scope here

- **Hot module replacement (HMR).** Plain reload is enough for v1. Path C Phase B explicitly defers HMR.
- **CSS preprocessing pipeline.** Vox emits a static `vox-tokens.css`; that's the contract. Apps wanting more bring a static pipeline (Tailwind precomputed, etc.) and import the resulting CSS.
- **React-specific concerns** (Suspense, server components, streaming). Vox's surface model is React-flavored but doesn't expose these; once it does, codegen extends.
- **Type-checking emitted TS.** Vox already type-checks its source; once Path C ships a bundle target, `tsc` is not on the runtime path.
- **The 4 already-fixed codegen-TS bugs** ([commits in main 2026-05-08](https://github.com/vox-foundation/vox/commits/main)). Don't re-fix them.

---

## 6. Open questions

These are decisions the implementing session should resolve explicitly (one-line each in the relevant PR):

1. **Where does `@vox/runtime` live?** Recommendation: a published npm package `@vox-lang/runtime` on the workspace's published-packages track; sourced from a new crate `crates/vox-runtime-ts` or similar. Path C reduces this to "compiler-vendored runtime files."
2. **Async handler shape.** When emitting an async handler (1.C.1), wrap in `void (async () => { ... })()` (fire-and-forget) or `async () => { ... }` (caller-awaited)? React event handlers are fire-and-forget; pick that.
3. **Missing-import resolution: per-file or whole-module?** Per-file scan keeps imports tight and tree-shake-friendly; recommend it.
4. **Endpoint local-execution dispatch.** A runtime feature flag `Capacitor.isNativePlatform()` vs separate emit? Recommend a single emit per endpoint that branches at the call site (smaller bundle, easier to test).
5. **TS fixtures during the §3 migration.** Keep them as parity checks (proves the Vox version produces identical output to the original TS), or replace entirely? Recommend keep-as-parity for one PR cycle, then delete once trust is established.

---

## 7. Sequencing & parallelism

Each `§1.X` is its own PR off main. §1.A.1, §1.A.3, §1.B.1 are independent — three subagents in parallel. §1.A.2 + §1.C.1 share the async-emit machinery; do them sequentially in one PR. §1.A.4 vanishes once §1.A.1 lands (no separate PR).

§2 is its own track (one PR per option-A increment).

§3 lands after §1 + §2 — porting needs the runtime imports to be auto-emitted, the TS to actually run end-to-end, and the local-endpoint execution working.

§4 (Path C) interleaves per the diagram in §4 of this doc.

Total: ~10 PRs to fully eliminate hand-written TS from a Vox app.

---

## 8. Per-PR verification gates

Each codegen PR must:
- `cargo nextest run -p vox-compiler` green (existing + new goldens).
- A new golden snapshot under `examples/golden/` exercising the specific bug.
- Run the snapshot through `tsc --noEmit` (proves the emit parses).
- Run the snapshot through `vite build --minify=esbuild` AND `vite build --minify=false` (proves the emit survives both, catches dead-code-elimination regressions).
- For PRs in §1.C / §2: a Playwright golden against a bundled output proving the behavior in a real browser.
- No regression on the existing `--target=ts-react` byte-for-byte output for files unrelated to the fix.

---

## 9. What lands when

Once all of §1 + §2 + §3 + §4 ship, the tracker app's source tree drops to:

```
apps/vox-mental-tracker/
├─ Vox.toml
├─ capacitor.config.ts        # 12 lines, Capacitor metadata only
├─ src/
│  ├─ main.vox                # everything: components, endpoints, db schema
│  └─ lib/
│     ├─ materializer.vox
│     ├─ export_contract.vox
│     └─ export_pipeline.vox
├─ tests/
│  ├─ fixtures/
│  └─ *.vox                   # vitest replaced by `vox test`
├─ contracts/
└─ docs/
```

No `package.json`, no `tsconfig.json`, no `vite.config.ts`, no `index.html`, no `src/main.tsx`, no `src/runtime.ts`, no `scripts/postbuild-fixup.mjs`, no `src/ts/*.ts`. The full app, in Vox.

---

## 10. The paste prompt for the new tab

```
You are picking up the "zero hand-written TS" track — making Vox emit
everything a real app needs so authors don't write any TypeScript.

Working directory: C:\Users\Owner\vox

Read this single document end-to-end before any action:

  docs/superpowers/plans/handoff/2026-05-08-handoff-zero-ts-vox-self-sufficient.md

Sequence:
- §1.A.1, §1.A.3, §1.B.1 are independent — spawn parallel subagents.
- §1.A.2 + §1.C.1 share async-emit machinery — one PR sequentially.
- §1.A.4 is no-op once §1.A.1 lands; don't open a separate PR.
- §2 (endpoint local-execution): pick option A first; B/C are follow-ups.
- §3 (TS→Vox migration of materializer / export_pipeline / export_contract):
  AFTER §1 + §2 land. Lands as a follow-up PR off the tracker app branch
  (claude/vox-mental-tracker-baseline, PR #70).
- Cross-check Path C plan #79 — its phases interleave per §4 of this doc.

Each codegen PR off main; tracker-app migration PR rebases on top.
```
