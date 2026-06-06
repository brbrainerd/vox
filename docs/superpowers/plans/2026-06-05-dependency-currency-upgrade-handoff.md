# Dependency Currency — Upgrade Handoff (2026-06-05)

Handoff for the dependency bumps that were **deferred** during the 2026-06-05 Dependabot
sweep, plus a broader "get everything to latest" backlog. Goal stated by maintainer:
*eventually migrate to all latest versions unless there is a real disadvantage beyond the
work itself.* This doc estimates, for each, **what breaks, what needs fixing/removing, and
how much agony** — weighing implementation difficulty, not just blast radius.

Difficulty was assessed by reading the actual call sites (4 parallel read-only research
agents); builds were **not** run, so effort numbers are informed estimates, not verified.

## TL;DR — recommended order

| Item | Versions | Difficulty | Agony | Effort | Verdict |
|---|---|---|---|---|---|
| **wasmtime stack** | 42 → 45 | trivial–low | very low | <1 h + build | **Do it first.** Contained bump. |
| **TypeScript 6 (vox-vscode)** | 5.5 → 6.0 | low | low | 1–3 h | **Do it.** CI-gated, code already TS6-idiomatic, `docs-astro` already runs TS6. |
| **gix** | 0.70 → 0.83 | moderate (easy lean) | low–med | 2–4 h | **Do it.** Shallow read-only usage; free win = delete one dead dep. |
| **RustCrypto stack** | sha2/sha3/hmac → `digest 0.11` | moderate (low) | medium | 2–4 h | **Do it, atomically.** Main risk is upstream availability + transitive `digest 0.10` holdouts, not our code. |
| **rand 0.8 → 0.9 (finish straddle)** | 0.8 → 0.9 | low–moderate | low | 1–3 h | Cleanup: a `rand09` alias already exists mid-migration. |
| **TS version unification** | 6 files, 5.0–6.0 | low (mechanical) | low | 1–2 h | Align all `typescript` pins behind a TS6 floor. |
| **jj-lib unpin** | `=0.27.0` → latest | unknown (pinned for a reason) | likely high | TBD | Investigate why it's hard-pinned with `=`. |

---

## Part 1 — The deferred majors (closed Dependabot PRs #129/#131/#128/#111)

These were closed with `@dependabot ignore … major version` (reversible via `@dependabot
unignore` or by bumping the manifest directly).

### 1. wasmtime 42 → 45 (was #129 `wasmtime-environ`)
- **Why it was blocked:** `wasmtime-environ` is version-locked to `wasmtime`/`wasmtime-wasi`; bumping it alone won't resolve. The whole stack must move together.
- **Blast radius:** 4 crates declare it, but **all real usage is one file** — `crates/vox-wasm-engine/src/engine.rs` (~80 lines). `vox-plugin-runtime-wasm` and `vox-cli` only touch the `WasmHost` facade / `Engine::default()`.
- **What it uses:** core-wasm API only (`Config`/`Engine`/`Module`/`Linker`/`Store`/`instantiate_pre`/`get_typed_func`) + **WASI Preview-1 sync** via the modern `wasmtime_wasi::p1`/`p2::pipe` (already migrated off `wasi-common`), fuel via the current `set_fuel`. **No component model, no async, no host funcs, no ResourceLimiter, no Cranelift API** — i.e. none of the surfaces that churn.
- **What breaks/needs fixing:** likely nothing in code; worst case a renamed `p1`/`p2` helper or a `WasiCtxBuilder` signature tweak — single-file. Bump `Cargo.toml:274-275`, regenerate `workspace-hack` via `cargo hakari generate` (lines 147-148/291-292 — do **not** hand-edit). Verify workspace MSRV meets wasmtime 45 (~1.83+).
- **Difficulty: trivial → low.** Effort **<1 h** + build/test.

### 2. TypeScript 6 on vox-vscode (was #111)
- **Correction to the original deferral rationale:** CI **does** gate this — job `vox-vscode-extension` (`.github/workflows/ci.yml:849-864`) runs `npm run compile` (`tsc -p ./`) on Node 24. And `docs-astro` already runs TS 6.0.3 via `astro@^6`, proving TS6 is viable in-repo. So this is a *good* near-term migration, not a risky one.
- **Blast radius:** ~28 extension-host `.ts` files (no `.tsx`; the React/Radix deps are unused-by-tsc cruft). `tsconfig` is `strict`, `skipLibCheck: true`, `target ES2020`.
- **Why it's easy:** code is already TS6-idiomatic — `catch (e: unknown)` + `instanceof` narrowing everywhere; **no removed-in-TS6 flags** in tsconfig; `skipLibCheck` insulates from lib.d.ts drift; `noUncheckedIndexedAccess` is off (TS6 doesn't flip it).
- **What to watch (each trivial):** `globalThis.crypto` access in `VoxMcpClient.ts:199-201,500-502` (one-line cast or `@types/node` bump); bump `@types/node@^20 → ^24` to match the Node-24 runner.
- **Difficulty: low.** Expect **0–2** one-line fixes. Effort **1–3 h**.

### 3. gix 0.70 → 0.83 (was #128)
- **Why it looked scary:** 13 minor versions; gix is pre-1.0 and breaks APIs often.
- **Why it's actually fine:** usage is **shallow, read-only, and confined to gix's stable object-read core**. 3 files: `vox-git/src/bridge.rs` (ahead/behind via `merge_base` + parent walk), `vox-effort-audit/src/walk.rs`, `vox-scientia/src/producers/commit_graph.rs`. ~30 call sites over ~8–10 symbols: `gix::open`, `ObjectId::from_hex`, `find_commit`, `merge_base`, `head_id`, `rev_parse_single`, `Id::detach`, `Commit::{parent_ids,decode}`, decoded-commit fields. **None of the high-churn subsystems** (status/diff/worktree/remote/fetch/revision-walk) are used — HEAD/ref reads are hand-parsed from `.git/`, diffs shell out to `git`.
- **What breaks/needs fixing:** prime suspect is `walk.rs:143` `decoded.time().seconds` (gix-date `Time` shifted field↔method across versions); verify `merge_base`/`rev_parse_single`/`head_id` signatures and that features (`blocking-network-client`, `blocking-http-transport-reqwest`, `progress-tree`, `revision`) weren't renamed. Bump root `Cargo.toml:272`, regenerate `workspace-hack` (6 `gix-*` pins).
- **Free win:** `crates/vox-effort-route/Cargo.toml:15` declares `gix` but **no source uses it** — delete that line.
- **No simpler alternative by policy:** `vox-git` charter is "pure-Rust, never libgit2/git2" (`vox-git/src/lib.rs:18-21`), so git2 is off the table; dropping gix for shell-out would contradict the crate's design intent. **Just do the bump.**
- **Difficulty: moderate (easy lean).** Effort **2–4 h** (mostly build + workspace-hack regen).

### 4. RustCrypto stack — sha2/sha3/hmac → `digest 0.11` (was #131 `sha3`)
- **Why sha3 can't move alone:** sha2/sha3/hmac share the `digest`/`crypto-common` traits; mixing 0.10 and 0.11 across them fails trait resolution. **One atomic, whole-workspace PR.**
- **Blast radius:** 16 first-party crates declare sha2/sha3/hmac; ~30 hashing call sites — but **all use the highest-level idioms** (`Sha3_256::new().update().finalize()` → `{:x}` / `hex::encode` / `data_encoding.encode(&…)` / `.into() → [u8;32]`). Only **2 real HMAC sites** (`vox-codegen/web_ir/paginated_emit.rs`, `vox-orchestrator/context_envelope.rs`); `vox-plugin-webhook/signing.rs` hand-rolls HMAC over raw `Digest`. **No `GenericArray`/`typenum`/`finalize_fixed`/`Output` usage anywhere** — exactly the surfaces that break are absent.
- **What changes:** 3 version strings in root `Cargo.toml` (`sha2 0.11`, `sha3 0.12`, `hmac 0.13`) + regenerate `workspace-hack`. Expect **0–4 source fixups**, all one-liners, most likely in `vox-crypto/facades.rs` and `vox-distributed-training/{checkpoint,strategy/data_parallel}.rs` (the `.finalize().into()`→`[u8;32]` sites) and the 2 HMAC sites (`into_bytes()`/`verify_slice`).
- **The real risk is ecosystem, not code:** (a) confirm sha2 0.11 / sha3 0.12 / hmac 0.13 are actually published; (b) transitive dependents (`ed25519-dalek`, `chacha20poly1305`, `blake3`, `gix-features`) may still pull `digest 0.10`, so the lockfile will carry **both** generations — allowed (different majors coexist) but bloats the build. If the 0.11 line isn't fully released, difficulty jumps to **hard (blocked on upstream)**.
- **Nothing to remove:** `blake2`/`sha1`/`crypto-common`/`digest` are not real first-party deps (only `workspace-hack`/test fixtures).
- **Difficulty: moderate (low end).** Effort **2–4 h** + the upstream-availability check.

---

## Part 2 — Broader "migrate to latest" backlog (beyond the deferred four)

### High value
1. **Unify the `typescript` pins** — currently 6 different versions: `docs-astro` `^6.0.3`, visualizer `~5.9.3`, marquee_app `~5.8.3`, vox-mental-tracker `^5.6.0`, vox-vscode `^5.5.0`, `crates/vox-gui/ui` `^5.0.2`. Align behind the TS6 floor `docs-astro` already proves works. Mechanical, **1–2 h**.
2. **Finish the `rand` 0.8 → 0.9 straddle** — root `Cargo.toml` already wires both `rand = "0.8"` (`:230`) and `rand09 = { package = "rand", version = "0.9" }` (`:347`). Complete the move, drop the alias. `rand` 0.9 renamed several APIs → *needs code*, **1–3 h**.
3. **Verify the visualizer's outlier pins** — `apps/experimental/visualizer` is on `vite@^8.0.1` + `@vitejs/plugin-react@^6.0.2` + `@types/node@^25` while every other app is on vite `^6`. (Install resolved fine during the 2026-06-05 dev-dep bump, so it's real — but it's a maintenance outlier; decide whether to lead with it or pull it back in line.)

### Medium / cleanup
- **`schemars` dual-version** — both `schemars = "1"` (`:243`) and `schemars08 = "0.8"` (`:344`); consolidation candidate (*needs code*).
- **`thiserror = "1"` → 2** (`:198`) — near-drop-in, low risk.
- **`cargo_metadata 0.18 → 0.19/0.20`** (`:298`) — trivial.
- **`typify 0.6.2 → 1.x`** (`:178`) — codegen API churn, *needs code*.
- **`tailwind-merge` skew** — `crates/vox-gui/ui` on `^2.x` while visualizer + vox-vscode are on `^3` (v2→v3 has an API change → *needs code* for the GUI).
- **`vitest 2 → 3`** in `crates/vox-gui/ui` (+ marquee/mental-tracker) — config/API changes, *needs code*.
- **Playwright skew** — 1.49 vs 1.60 across integration-tests / vox-gui-ui / docs-astro / mental-tracker; align to one (trivial).
- **`lucide-react` anomaly** — vox-vscode pins `^1.17.0` but lucide-react's real line is `0.x`; looks bogus (and it's unused there) — investigate/remove.

### Investigate before attempting
- **`jj-lib = "=0.27.0"`** (`:273`) — hard `=` pin signals known breakage on bump; jj moves fast (0.30+). Find out *why* it's pinned before estimating.
- **`tree-sitter-cli 0.22` + `nan`** in `tree-sitter-vox` — `nan` is legacy (modern tree-sitter uses N-API); migrating off it is *needs code*, low priority (build tooling, not shipped runtime).

### Leave alone (deliberate)
- `tantivy 0.22` — feature-gated on purpose (see build/crate-org audit memory); don't bump unless asked.
- `serde_yaml 0.9` — upstream unmaintained; eventual swap to `serde_yml`/TOML is a separate decision, not a version bump.

---

## Cross-cutting notes
- **`workspace-hack` is hakari-generated** — never hand-edit it; every Rust bump above requires `cargo hakari generate` to refresh the unification pins (gix-*, wasmtime-environ, digest/crypto-common, etc.).
- **CI capacity is the real constraint, not correctness.** The 2026-06-05 sweep saturated the self-hosted runners by triggering ~20 full matrices at once. Do these as **small batches (≤3 PRs)**; the required gate is the single `Check, Build, and Test (Rust)` check (`ci-summary`), which only validates Rust — JS/app and GitHub-Actions bumps it can't validate, so route those as admin-merges, not CI rounds.
- **Reopening a deferred bump:** `@dependabot unignore <dep> major version`, or just bump the manifest directly in the migration PR.
