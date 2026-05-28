# Handoff — Path C: Vox compiler owns the full web toolchain

> **For agentic workers:** REQUIRED SUB-SKILLS: superpowers:using-git-worktrees, superpowers:writing-plans (review only — plan is pre-written), superpowers:subagent-driven-development (parallel where independent), superpowers:executing-plans (sequential fallback), superpowers:test-driven-development (per task), superpowers:verification-before-completion, superpowers:requesting-code-review.

This document is a complete, stand-alone handoff. The receiving session has no prior context. Read it end-to-end before any action.

---

## 0. The goal

**Eliminate the JS toolchain dependency for Vox apps.** No npm, no pnpm, no Vite, no esbuild — just `vox build` and `vox serve` produce a deployable, browser-runnable, Capacitor-wrappable app. Cargo stays (that's how the compiler ships); JS-side runtime libraries (React, Capacitor) stay; what disappears is the *build* toolchain layer between Vox source and a static `dist/`.

When this work lands, the canonical "build a Vox app" flow becomes:

```
vox build apps/my-app/src/main.vox -o apps/my-app/dist          # bundled prod
vox serve apps/my-app/src/main.vox                              # dev server + watch
```

…and `apps/my-app/package.json` keeps only Capacitor + React as **runtime** dependencies. No `devDependencies`, no `vite.config.ts`, no `index.html`, no `src/main.tsx` shim.

## 1. Where you are

- **Repo:** `C:\Users\Owner\vox\` (vox-foundation/vox).
- **Main branch for this work:** `main`.
- **Build target during dev:** `cargo build --release -p vox-cli` then `target/release/vox.exe`. The globally-installed `~/.cargo/bin/vox` is stale relative to source until reinstalled, and reinstall requires stopping any running vox.exe processes (don't assume that's safe).
- **Open PR using the current TS codegen path (do not break it):** [#70](https://github.com/vox-foundation/vox/pull/70) — vox-mental-tracker app. Its Phase 5 Playwright lane caches Chromium and self-skips when `BASE_URL` is unset; once Path C lands, the tracker rebases and **Phase F of this plan** swaps Vite for `vox serve`.

## 2. Prerequisites — the 4 codegen-TS bugs in [PR #78's plan](../language/2026-05-08-codegen-ts-bugs-blocking-tracker.md)

Before Path C's emit changes are valuable, the existing TS codegen needs to produce *correct* output. Four bugs documented in [`2026-05-08-codegen-ts-bugs-blocking-tracker.md`](../language/2026-05-08-codegen-ts-bugs-blocking-tracker.md):

- **A.** match arms emit `case _:` literal patterns; `Ok(t)` / `Error(e)` bindings drop on the floor.
- **B.** `Speech.method()` lowers to `mobile.method()` — wrong runtime namespace.
- **C.** JSON-bearing string literals not `JSON.stringify`'d, so inner `"` chars break emitted TS parse.
- **D.** `@endpoint` calls and `std.*` references emit as bare identifiers with no import statements.

These four bugs share the same lowering layer Path C builds on. **Fix them first** — Phases A–G of this plan assume the underlying lowering is correct. Recommended order from PR #78 stands: **C → B → A → D**, each as its own PR off main.

## 3. The Path C plan

Seven phases, A through G. Phases A–E are compiler-side; Phase F migrates the tracker; Phase G is docs. **Each phase is one PR off main** unless explicitly broken further. Sequential dependencies are noted; everything else can run in parallel.

### Phase A — JS-flavor emission (no bundler, no foreign-dep handling)

**Outcome:** `vox build --target=browser-esm src/main.vox -o dist/` produces a directory the browser runs as native ES modules with an import map for foreign deps. No Vite, no tsc, no node.

**Tasks:**
- [ ] **A1.** Add `--target=browser-esm` flag to `vox build`. Default target stays unchanged (current TS codegen). Surface in `crates/vox-cli/src/commands/build.rs`.
- [ ] **A2.** JSX → `React.createElement(...)` lowering in `codegen_ts/jsx.rs`. The ts-codegen path keeps emitting JSX (it's TS that consumes it); the new `browser-esm` target lowers all the way to `createElement` calls. Reuse the existing JSX walker; only the leaf emitter changes.
- [ ] **A3.** Strip TypeScript type annotations on the `browser-esm` path. Per-let, per-param, per-return-type. The HIR has the types; codegen just doesn't emit them in this mode. Audit: implicit `Result<T>` returns, generic params, `as` casts, satisfies expressions.
- [ ] **A4.** Emit `.mjs` files instead of `.tsx`. Same module structure as today (one file per Vox component / module), just JS extension and JS contents.
- [ ] **A5.** Emit `index.html` at the dist root with `<script type="module" src="./main.mjs"></script>` and an inline import map block for foreign deps (default sources from `https://esm.sh/<pkg>@<version>`). Per-route HTML shells (already emitted to `target/generated/public/ssg-shells/` today) get pointer redirects from the root or are merged into a client-side router pulled from the existing `routes.manifest.ts`.
- [ ] **A6.** Move the React/react-dom shim into the import map (`"react": "https://esm.sh/react@19"`). Document the version pinning policy.
- [ ] **A7.** Golden example `examples/golden/browser_esm_minimal.vox` — declares one component, uses one `useState`, renders `<div>Hello {name}</div>`. `vox build --target=browser-esm` emits `dist/` containing `index.html` + `main.mjs` + supporting files. Add a Playwright golden under `crates/vox-compiler/tests/` that boots Chromium against `python3 -m http.server` serving the dist.

**Files (compiler side):**
- New: `crates/vox-compiler/src/codegen_browser_esm/{mod.rs, html.rs, jsx_lower.rs, type_strip.rs, import_map.rs}`.
- Modify: `crates/vox-compiler/src/codegen_ts/jsx.rs` (extract reusable JSX walker), `crates/vox-cli/src/commands/build.rs` (target flag + dispatch).

**Verification gate:**
- `cargo nextest run -p vox-compiler` passes (new + existing).
- The browser_esm_minimal Playwright golden passes locally.
- Existing `--target=ts-react` (the current default) output is byte-identical — no regression on the TS codegen path.

---

### Phase B — `vox serve` static server with watch + rebuild

**Outcome:** `vox serve apps/my-app/src/main.vox` watches the source, rebuilds on change, serves the resulting `dist/` on a localhost port. Capacitor `cap sync` reads the same `dist/`.

**Tasks:**
- [ ] **B1.** New CLI subcommand `vox serve <vox-file>` (alias `vox dev`). Flags: `--port` (default 5173), `--host` (default 127.0.0.1), `--target` (default `browser-esm`).
- [ ] **B2.** Build once on startup. Use the existing `tokio` runtime (already a workspace dep) for the file server.
- [ ] **B3.** Watch mode using `notify` crate (already in workspace if any other CLI uses it; add otherwise). On `.vox` file change in the dependency graph: re-run `vox build --target=...` into the same `dist/`, then post a server-sent event to connected clients.
- [ ] **B4.** Reload mechanism: SSE endpoint at `/__vox_reload`. Inject a `<script>` into the served `index.html` that listens and triggers `location.reload()` on event. **Not HMR** — full reload only; HMR is its own multi-month plan.
- [ ] **B5.** MIME types for `.mjs`, `.css`, `.html`, fonts. Audit security headers (no `X-Frame-Options` allow-iframe by default; we want Capacitor compatible).
- [ ] **B6.** `Capacitor compatibility test` — once dist is built, `cap sync android` against the tracker app must succeed without a Vite step.

**Files:**
- New: `crates/vox-cli/src/commands/serve.rs`, `crates/vox-cli/src/serve/{watcher.rs, sse.rs, mime.rs}`.

**Verification gate:**
- `vox serve` boots, serves the minimal example, full-reloads on file change.
- A Playwright lane points at `vox serve` instead of Vite — replaces the tracker's eventual `webServer` config in `playwright.config.ts`.

---

### Phase C — Foreign-dep handling: vendored ESM cache

**Outcome:** No mandatory CDN dependency. App builds offline once `target/runtime-cache/` is warm.

**Tasks:**
- [ ] **C1.** Decide on the foreign-dep manifest. Recommended: a `[browser]` table in the existing `Vox.toml`:
  ```toml
  [browser.runtime-deps]
  react = { version = "19.0.0", source = "esm.sh" }
  "react-dom/client" = { version = "19.0.0", source = "esm.sh" }
  "lucide-react" = { version = "0.468.0", source = "esm.sh" }
  ```
- [ ] **C2.** First-build behavior: for each entry, fetch the ESM build from `esm.sh` (or the configured source) and store under `target/runtime-cache/<pkg>@<version>/index.mjs`. Hash-pin via SHA-256 in `Vox.toml.lock` (new file alongside `Cargo.lock`). Subsequent builds offline.
- [ ] **C3.** `vox build --offline` errors if the cache is cold instead of fetching.
- [ ] **C4.** Import map in `index.html` (Phase A5) now points at `/runtime/<pkg>@<version>/index.mjs`, served from the runtime cache.
- [ ] **C5.** `vox cache ls / clean / verify` subcommand surfaces.

**Files:**
- New: `crates/vox-cli/src/commands/cache.rs`, `crates/vox-cli/src/runtime_cache/{fetcher.rs, lockfile.rs}`.
- Modify: Vox.toml schema in `crates/vox-config` (or wherever it lives).

**Verification gate:**
- A throwaway `vox build` against a fresh `target/` warms the cache, second build with `--offline` succeeds.
- The lockfile pins all transitive deps; `vox cache verify` flags hash drift.

---

### Phase D — Bundling for production (`vox build --target=browser-bundle`)

**Outcome:** Single (or chunk-per-route) `.mjs` output instead of N modules. Tree-shaken, minified, content-hashed.

**Tasks:**
- [ ] **D1.** Walk the import graph during codegen. The compiler already does this for type-checking; reuse the def-use graph in HIR.
- [ ] **D2.** Concatenate live modules into one `.mjs` per chunk, deduplicating module init by IIFE-wrapping each in a `__VOX_modules` table keyed by id.
- [ ] **D3.** Tree-shake unused exports. The HIR knows which functions/components are reachable from `routes.manifest.ts`; everything else is dropped.
- [ ] **D4.** Minify: identifier mangling (rename locals to short names from a global table) and whitespace stripping. Don't reach for a JS minifier — the compiler already understands the symbol table; a 1k-line Rust pass is plenty for v1.
- [ ] **D5.** Content-hash filenames: `main.<sha8>.mjs`. Update `index.html` references.
- [ ] **D6.** New flag `vox build --target=browser-bundle` selects this path. `--target=browser-esm` (Phase A) stays as the dev-friendly default.

**Files:**
- New: `crates/vox-compiler/src/codegen_browser_bundle/{mod.rs, treeshake.rs, mangle.rs, concat.rs, hash.rs}`.

**Verification gate:**
- Output of `--target=browser-bundle` on the tracker fits in <200 KB gzipped (sanity check).
- Bundle boots; tree-shake doesn't drop reachable code (covered by Playwright golden against the bundle).
- Content hash changes when source changes; identical builds produce identical hashes (determinism — critical for CDN caching).

---

### Phase E — Source maps

**Outcome:** Browser devtools show original Vox source on errors and during stepping.

**Tasks:**
- [ ] **E1.** Emit `.mjs.map` files for `--target=browser-esm` and `--target=browser-bundle`. Format: source map v3, `sources` pointing at the `.vox` files, `mappings` at line granularity (column granularity is nice-to-have, postponable).
- [ ] **E2.** Inline `//# sourceMappingURL=` comment at end of each emitted `.mjs`.
- [ ] **E3.** Document the limitations (Vox-syntax-line → JS-output-line; not full reverse evaluation).

**Files:**
- New: `crates/vox-compiler/src/codegen_browser_esm/sourcemap.rs`, reused by bundle path.

**Verification gate:**
- A deliberate `panic!` in a Vox component, opened in Chrome devtools, surfaces the right `.vox` filename and a near-correct line.

---

### Phase F — Migrate vox-mental-tracker

**Outcome:** The tracker app at [PR #70](https://github.com/vox-foundation/vox/pull/70) drops Vite-shaped infrastructure. Once this Phase F PR lands on top of A–E, the tracker is the proof point for the whole Path C work.

**Tasks:**
- [ ] **F1.** Delete (none-yet-existed) `vite.config.ts`, `index.html` at app root, `src/main.tsx`. The tracker never gained these in the original PR #70 scope; this is a "stay clean" gate.
- [ ] **F2.** Update `apps/vox-mental-tracker/package.json`: keep only Capacitor + the React runtime deps that still need to be import-mapped. Drop `@vitejs/plugin-react`, `vite`, anything pnpm-side that's only needed for a JS bundler.
- [ ] **F3.** Add `[browser.runtime-deps]` block to `apps/vox-mental-tracker/Vox.toml` listing react@19 and react-dom@19.
- [ ] **F4.** Update `capacitor.config.ts` `webDir` to remain `dist/` — `vox build --target=browser-bundle -o dist` is the new producer.
- [ ] **F5.** Update `apps/vox-mental-tracker/playwright.config.ts` with a `webServer` block that runs `vox serve` (no Vite). Point `baseURL` at the `vox serve` port.
- [ ] **F6.** Update `.github/workflows/vox-mental-tracker.yml`:
  - `vox-check` lane unchanged.
  - `vitest` lane unchanged (vitest stays for unit tests against the TS-side libs like `materializer.ts` / `export_pipeline.ts` — those are app-side TS, not codegen output).
  - `playwright` lane gains a `vox build` step before browser launch; the existing browser cache (this turn's commit `2b2ae5be1`) carries over.
  - `contracts` lane unchanged.
- [ ] **F7.** Update `apps/vox-mental-tracker/docs/contributors/ci-lanes.md` and the Phase 5 plan to reflect the new pipeline.
- [ ] **F8.** Run the Phase 6 release gates (G1-G4 programmatic). Manual gates G5 (Android Capacitor build) and G6 (iOS) confirm the dist is wrappable. **G5 is the proof — the tracker boots on a real Android device with zero JS toolchain in the build pipeline.**

---

### Phase G — Docs + ecosystem

**Outcome:** Other Vox app authors know how to use Path C.

**Tasks:**
- [ ] **G1.** `docs/src/reference/cli-build.md` — surface the new `--target=browser-esm`, `--target=browser-bundle` flags.
- [ ] **G2.** `docs/src/reference/cli-serve.md` — `vox serve` reference.
- [ ] **G3.** `docs/src/how-to/web-app-without-vite.md` — migration guide for existing Vox apps using Vite. Include the tracker's before/after as a worked example.
- [ ] **G4.** `docs/src/architecture/web-toolchain.md` — explain the design: why no bundler dependency, what we own, what we deliberately don't.
- [ ] **G5.** `docs/src/reference/runtime-deps.md` — the `[browser.runtime-deps]` Vox.toml table semantics.
- [ ] **G6.** Capabilities matrix: what Vite features Path C does NOT cover (PostCSS pipeline, asset transforms beyond MIME, fancy plugins). For each, suggest the workaround or escape hatch.

---

## 4. Sequencing & parallelism

**Wave 1 (after PR #78's bugs land — sequentially):** Phase A. Single PR, foundational.

**Wave 2 (parallel, both depend on A):**
- Phase B (vox serve)
- Phase C (foreign-dep cache)

**Wave 3 (after A + B + C):**
- Phase D (bundling) — independent build target on top of A's plumbing
- Phase E (source maps) — adds output to A's and D's emit

**Wave 4 (after A–E):**
- Phase F (tracker migration)
- Phase G (docs) — can run in parallel with F; cross-link once both ready

Estimated total: ~8 PRs, ~6 weeks of focused work for one engineer; less if subagents take Wave 2 and Wave 3 in parallel.

## 5. Out of scope

These are real concerns but **not** part of Path C:

- **Hot module reload (HMR).** Phase B does full reload only. HMR requires a runtime module-replacement protocol; its own multi-month plan.
- **CSS preprocessing** (Sass, PostCSS, Tailwind). Path C ships static `.css` files only; the existing `vox-tokens.css` model continues. Tailwind can be precomputed and imported as a static `.css` if needed.
- **Image / asset transforms.** Static pass-through only.
- **HMR-style `import.meta.hot`.** Not exposed.
- **Browser-side Capacitor JS plugin transforms.** They ship as ESM and just work via the import map.
- **Replacing tsc for type-checking.** Vox already type-checks its own source. The browser-esm path strips types instead of emitting them; tsc is not part of the runtime path.

## 6. Branch hygiene

- One phase = one branch off `main` = one PR. Don't bundle phases.
- Within a phase, multi-task PRs are fine (Phase A's seven tasks are one PR).
- Phase F (tracker migration) **does not** land in `claude/vox-mental-tracker-baseline` ([PR #70](https://github.com/vox-foundation/vox/pull/70)). It's its own follow-up PR off main; the tracker's main PR rebases after Phase F lands.

## 7. Verification gates per PR

Each phase PR must show:
- `cargo nextest run -p vox-compiler` (and `-p vox-cli` if touched) green.
- `vox check` clean across all `examples/golden/*.vox` and `apps/*/src/main.vox`.
- New golden snapshots for the introduced surface.
- `cargo build --release -p vox-cli` (so a fresh binary exists for downstream phases).
- For phases that add a `--target=`: byte-identical output on the existing `--target=ts-react` path (no regression).
- For Phase F specifically: `vox build --target=browser-bundle` produces a `dist/` that `cap sync android` accepts and an Android emulator boots and renders Home.

## 8. Open questions to resolve early

These are decisions the Phase A author should make explicit (one paragraph each in the PR):

1. **Which JSX runtime?** Classic (`React.createElement`) or new transform (`_jsx`/`_jsxs`). Recommend **classic** for v1 — simpler to emit, no runtime helper module needed.
2. **`react-dom/client` import path.** Browsers via esm.sh expose subpath exports; pin a known-working URL form.
3. **How to handle Vox endpoints on the browser side.** They were emitted as bare identifiers (Bug D in PR #78's plan). Once D lands and they're properly imported from `vox-client.ts`, the browser-esm path inherits the wiring. Don't re-solve here.
4. **`SpeechModule` runtime resolution.** Same — Bug B in PR #78. Path C uses whatever the fixed lowering produces.
5. **HTTP route shells (`target/generated/public/ssg-shells/`).** Today `vox build` emits per-route HTML shells. Phase A5's single `index.html` model uses a client-side router; either drop the shells on the `browser-esm` target OR keep them as SSG fallbacks. Decide and document.

## 9. The paste prompt for the new tab

```
You are picking up the Vox Path C track — the Vox compiler owns the full
web toolchain, so apps need no JS bundler.

Working directory: C:\Users\Owner\vox

Read this single document end-to-end before any action:

  docs/superpowers/plans/handoff/2026-05-08-handoff-path-c-vox-owns-toolchain.md

Prerequisites: the four codegen-TS bugs in
docs/superpowers/plans/language/2026-05-08-codegen-ts-bugs-blocking-tracker.md
(PR #78's plan) need to be fixed first. Land those four PRs (order
C → B → A → D), then start Path C Phase A.

Each phase is its own branch off main. Phase F (tracker migration)
is a follow-up PR off main, not on the existing app branch.
```
