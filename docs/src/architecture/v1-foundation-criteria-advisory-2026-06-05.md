---
title: "v1.0 Foundation-Criteria Advisory (2026-06-05)"
description: "Why the current v1.0 criteria cannot certify a shippable language, and a proposed FOUNDATION tier + LLM-actionable criteria format that drive the Vox compiler, crates, and GUI to production completeness."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
---

# v1.0 Foundation-Criteria Advisory

> **Scope.** This is an *advisory* on how to revise
> [`v1-release-criteria.md`](./v1-release-criteria.md). It does **not** edit
> the live criteria. It is backed by a 6-dimension forensic audit
> (commit history, compiler-arm parity, golden reality, crate-install
> story, GUI shippability, criteria adequacy) run 2026-06-05 against
> `main @ 01366cd38f`.

## 0. Bottom line

The current 22 criteria (CR-P/A/E/D/L) are **structurally incapable of
certifying that Vox is a finished language.** Every one of them gates a
**product, deployment, performance, or agentic-DX outcome that
*presupposes* a working compiler.** Not one gates the compiler's own
completeness or correctness. The proxy that comes closest — the golden
corpus guard and the CR-L1 HumanEval gate — verifies that programs
**typecheck** (or pass an interpreter-only `@test`), never that the
language **executes every construct correctly across the arms it ships.**

Consequently, an LLM told "drive these criteria to green" would optimize
deploy loops and HumanEval prompts **while the compiler stays partial** —
which is exactly what the commit history shows is happening: table-stakes
semantics (exact `Decimal`, compiled `Regex`, blocking `std.http`, `std.time`,
`Option`/`Result` match exhaustiveness) landed **at HEAD in commit #137**,
and core-fix activity is **accelerating, not converging.**

The fix is a new **FOUNDATION tier (CR-F)** that must go green **before any
product tier is even evaluated**, written in an **LLM-actionable format**
(`statement` + one exact `verify_cmd` + `artifact_path` + `if_failing`
pointer) so the criteria document itself becomes the driver that an agent
reads to know what to build next.

## 1. The structural diagnosis

Classifying all 22 existing criteria by *what they actually gate*:

| Tier present today | Criteria | Gates… | Presupposes a finished compiler? |
|---|---|---|---|
| Production | CR-P1/P2/P3 | apps live on OCI, uptime, deploy-loop time | **Yes** |
| Architecture | CR-A1/A2/A3/A4 | complexity, schema, cycles, lifecycle metadata | **Yes** |
| Performance | CR-E1/E2/E3 | cold start, bundle size, training loss | **Yes** |
| Agentic DX | CR-D1/D2/D3 | plan fidelity, repair rate, CLI docs | **Yes** |
| LLM-target | CR-L0..L8 | agent authorship, HumanEval, on-distribution, repair, plan, ACI, retirement, deploy-CLI, feedback loop | **Yes** |
| **Foundation** | **— none —** | **the compiler is complete + correct** | **n/a** |

`grep -ci 'stdout\|behavioral\|golden output\|expected output'
v1-release-criteria.md` = **0**. The only `execute` references are CR-E1
(a *timing* metric) and CR-D1 (agent *orchestration*). Neither asserts a
program produces the **correct result**.

**This is the entire problem in one sentence: the criteria gate the things
built *on top of* Vox, and never Vox itself.**

## 2. Evidence (what the audit found, quantified)

### 2.1 The language is not converging (commit forensics)

- Over 120 days: **1,905 commits**; **686 (36%)** of subjects match a
  compiler-core keyword.
- Core-fix **share of all commits is accelerating** month-over-month:
  **5.2% (Mar) → 9.3% (Apr) → 10.3% (May) → 19.8% (Jun)**. A converging
  language bends this curve toward zero; Vox bends it **upward**.
- Core-fix commits per 2-week window: **3, 2, 14, 87, 32, 68** — a renewed
  rise right at HEAD.
- **HEAD commit #137 (`01366cd38f`, 675 files, +6906/-6237)** lands, *for
  the first time*: `VoxValue::Decimal`, compiled `Regex`, real blocking
  `std.http`, `std.time.now_ms` interpreter arm, and `Option`/`Result` match
  exhaustiveness (E0301). Test counts thrash **inside one squashed PR**
  (615→622→624→627→628). This is **table-stakes semantics arriving at the
  release frontier**, not v1.0 polish.
- Dominant recurring arm: **interp↔codegen parity** (16 parity commits,
  clustered at HEAD) — two execution tiers with divergent semantics being
  reconciled builtin-by-builtin.

### 2.2 There are four hand-synchronized language implementations (arm parity)

- Vox is **not one engine with backends.** It is four implementations of
  the same language: a Rust tree-walking interpreter (`--interp`, in
  `crates/vox-compiler/src/eval` — **not** crate `vox-eval`), `--mode script`
  (codegen-rust → `cargo run`), and the codegen-ts emitter (web + RN).
- Parity is a **manual obligation** ("update both crates in one PR",
  `vox-shell-stdlib-ssot-2026.md`), with **no differential test** that runs
  one `.vox` through interp **and** script **and** ts and compares output.
- Incompleteness clusters in the **unverified** arms: codegen-ts has the
  most markers (21) and **throws at runtime** for typecheck-passing
  constructs (`UnsupportedOnPlatform` for on-device db
  `Get`/`Delete`/`FilterRecord`/raw-clause; AI fixtures silently not
  lowered), whereas codegen-rust mostly fails **loudly at codegen time**
  (`compile_error!` WASI guards) and the interpreter is marker-clean.
- **Biggest silent-wrong-thing risk:** behavioral verification exists for
  exactly **one** arm (interp). A golden can emit subtly-wrong or
  runtime-throwing Rust/TS **forever** and every green gate still passes.

### 2.3 The corpus proves parsing, not running (golden reality)

- `examples/golden`: **67 top-level `.vox`**; harness is parse + lower +
  WebIR-validate, plus a full-typecheck gate. Behavioral execution only for
  the **19/67** files with an `@test` fn — the runner `continue`s past the
  other ~48 (`golden_vox_test_runner.rs:128`). **0/67** carry a `// EXPECT`
  stdout fixture. **No stdout/behavioral-output harness exists anywhere in
  the Rust tree.**
- `PARSE_STATUS.md` is literally a **parse** matrix (all rows "✅ PASS"),
  tagged `Syntax Version 0.5.0` while the workspace is `0.6.0`, and lists
  **56 rows for a 67-file corpus** — parse-only *and* stale.
- **Correction to the original premise:** the HumanEval gate is **no longer
  typecheck-only** as of HEAD — it runs each `tests.vox` under
  `vox run --mode interp` (`humaneval.rs:129/238`), so false oracles now fail
  behaviorally. But this is the **only** behavioral surface, it covers
  **only the interpreter**, and CR-L1's *actual* claim (pass-rate "when
  prompted to MENS or a reference LLM") is still measured against
  **hand-authored** reference solutions, not LLM-generated ones
  (`per_llm: Vec::new()`).

### 2.4 There is effectively no crate-install story (distribution)

- Of **103 crates, exactly ONE (`vox-crypto`) passes
  `cargo publish --dry-run`.**
- Universal blocker: **75/103** manifests carry a path dep on the
  `publish=false` `workspace-hack` (hakari) crate. Internal `vox-*` path
  deps also lack `version` requirements → hard-fail before metadata is even
  checked.
- **~34 publishable-flagged crates omit `license`** (Cargo does **not**
  auto-inherit the workspace license); `vox-cli` sets `license = false`.
- All crates **version-locked to 0.6.0** (`version.workspace = true`) — no
  independent library cadence.
- **`voxup` does not install a real toolchain**: it provisions
  `~/.vox/{bin,toolchains}`, parses a toolchain manifest, and installs a
  **proxy wrapper** — **no artifact download, no checksum verification.**
- `vox-checksum-manifest` and `vox-release-artifacts` (signed-artifact
  provenance) are **PLANNED-not-landed**. **No `cargo publish` workflow, no
  `cargo-dist`, no `cargo-semver-checks`, no public-API stability policy.**
- README admits: "Official installation packages … have not yet been
  formally released." The only real path today is
  `cargo install --path crates/vox-cli` from a git clone.

### 2.5 VOX GUI is built but unshippable and its gates are theater (GUI)

- `vox-gui` is a substantive Tauri 2 shell (20 backend command modules,
  ~21 real surfaces, active in PR #136) — **not** a stub.
- But the self-surfacing coverage gate (`vox ci gui-surface-registry`)
  **runs in zero CI workflows** (only a stale comment in `ssot-drift.yml`
  claims otherwise), and its wiring check is a weak App.tsx substring match.
- The one **required** GUI merge gate (`gui-playwright-smoke`) tests the
  **emitted codegen web app**, not vox-gui's surfaces. vox-gui's own e2e +
  vitest suites **never run in CI.**
- Packaging is broken **three independent release-blocking ways**:
  `tauri.conf.json` emits **no installers** (no `targets`/`active`, only an
  `icon.png` — no `.ico`/`.icns`); `release-gui.yml` **never builds the
  `externalBin` sidecars**; the Windows signing step points at a
  **nonexistent `src-tauri/` path** → ships unsigned.
- **A normal user cannot download and run a signed VOX GUI build today.**

## 3. What to PRUNE from the prior framing (stale assumptions)

The audit invalidated two assumptions worth retiring so effort isn't wasted:

1. **"The measurement fixtures don't exist."** *Stale.* The Marquee manifest
   (`contracts/marquee/manifest.v1.yaml`) and the CR-L corpora now exist at
   full size on disk: spec-to-app = 10 specs, repair-corpus = 50 projects,
   plan-fidelity = 50 plans, humaneval-vox = 164 problems, with live
   artifact dirs under `contracts/reports/`. **Do not re-plan "build the
   corpora."** The gap is foundational verification, not corpus inventory.
2. **"Honest completion finished the language."** *Stale.* The
   [2026-05-21 honest-completion plan](../../superpowers/specs/2026-05-21-v1-honest-completion-plan.md)
   delivered genuine **anti-overclaim machinery** (`vox audit --gate all`,
   the evidence ledger, "no path, no claim"). But it was about **measurement
   honesty for the existing outcome gates**, and explicitly pushed
   compiler-arm fixes into a *separate* plan with **no CR criterion behind
   them.** Reuse its machinery; do not assume it covers the core.

## 4. The proposed fix: a FOUNDATION tier + an LLM-actionable format

### 4.1 The meta-format (CR-META) — make the doc itself the driver

The user's hard requirement is criteria "read accurately by an LLM that
automatically result in completion." Satisfy it **structurally**: every
criterion (foundation **and** product) must carry four machine-readable
fields, and the doc fails its own lint if any are missing.

```
[CR-Fn] <one-sentence falsifiable statement>
  verify_cmd:    <exact command that exits 0 on pass, non-0 on fail>
  artifact_path: contracts/reports/<gate>/<UTC>.json   # the breakdown it writes
  if_failing:    <pointer to the plan section / fixture dir to build next>
```

Enforced by extending the existing evidence-ledger arch-check
(`vox-arch-check`, honest-plan §1.2): parse `v1-release-criteria.md`, assert
each `[CR-*]` block has a fenced `verify_cmd`, a resolvable `artifact_path`
matching a registered gate, and a non-empty `if_failing`. **For any red
gate, the agent reads `verify_cmd` to confirm failure, `artifact_path` to
inspect the per-item breakdown, and `if_failing` to know exactly what to
build — no human in the loop.**

### 4.2 CR-F0 — foundation-first ordering (the keystone)

> **`vox audit --gate all --strict-block-ga` must evaluate all CR-F gates
> BEFORE any CR-P/E/A/D/L gate, and must report product gates as
> `blocked_by_foundation` (never `met:true`) while any CR-F is red.**

- `verify_cmd`: `cargo run -p vox-cli -- audit --gate all --strict-block-ga`
- `artifact_path`: `contracts/reports/_snapshot/<UTC>.json` — `tier:"foundation"`
  rows first; product rows `blocked_by_foundation` when any `CR-F.met==false`.
- `if_failing`: implement gate ordering in `crates/vox-audit/src/registry.rs`;
  add a `vox-arch-check` rule asserting foundation gates sort first.

This single change makes it **impossible** for an agent (or a human) to
declare "v1.0" while the language is partial — it inverts the current
incentive that lets product work paper over compiler gaps.

### 4.3 The Foundation tier (CR-F1…F6)

Consolidated and de-duplicated from the audit's CR-X/CR-C/CR-G proposals.
Each is one falsifiable command.

| ID | Gate | `verify_cmd` (essence) | `if_failing` |
|---|---|---|---|
| **CR-F1** Behavioral goldens | Every golden that produces output executes and matches a committed `// EXPECT` block (or `@test`); **zero** parse/typecheck-only goldens. | new `cargo test -p vox-integration-tests --test golden_behavioral_gate`; coverage = `(#EXPECT∪@test)/(#top-level) ≥ 1.0` | build the `// EXPECT` subprocess harness (Track A1 of [golden-corpus plan](../../superpowers/plans/2026-06-02-vox-golden-corpus-and-compiler-reality.md)); add EXPECT lines |
| **CR-F2** Cross-arm parity | Every executable golden produces **byte-identical** stdout under `--mode interp` **and** `--mode script`; zero divergences, empty allowlist. | new `golden_arm_parity_test.rs` runs both modes, asserts equality | reconcile the diverging arm builtin-by-builtin; freeze a **non-growing** divergence allowlist |
| **CR-F3** Spec coverage | A machine-readable checklist (`contracts/spec/language-surface-coverage.v1.yaml`) maps **every** grammar production / decorator / builtin to ≥1 passing behavioral fixture; zero uncovered or `incomplete-arm` rows. | `vox audit --gate spec-coverage` cross-refs CR-F1/F2 results | add the missing fixture or finish the named arm; arch-check fails if a new production lands without a checklist row |
| **CR-F4** No incomplete arms | Zero `todo!()`/`unimplemented!()`/`not yet`/runtime-`UnsupportedOnPlatform` for any construct marked *supported* in CR-F3. Unlowerable constructs must fail at **codegen time**, never runtime. | `vox audit --gate no-incomplete-arms` (extend stub detectors over `vox-compiler` typeck+eval + `vox-codegen`) | implement the lowering arm, or convert the runtime throw into a codegen-time diagnostic (the codegen-rust WASI pattern) |
| **CR-F5** Convergence | Core-fix commits per rolling 2-week window decline for 3 consecutive windows AND last ≤ 25% of peak; the **release-tagged commit body contains zero first-time-semantics entries**. | `git log` + regex + arithmetic; `git show <tag>` body grep == 0 | keep fixing the core until the curve bends; do **not** tag a release whose own commit lands new semantics |
| **CR-F6** Regression budget | Zero `// vox:skip`, zero de-stub-pending mocks, zero stub/placeholder returns in `vox-compiler`, `vox-codegen`, and the golden corpus; count **non-increasing** between releases. | `rg` count over fixed paths vs the prior tag's stored integer | replace the stub with a real impl or scope the construct out of the supported set |

### 4.4 The Distribution tier (CR-K1…K7) — "what Rust users install"

This tier answers the user's question directly. **Declare a public crate
set first**, then gate it.

- **CR-K1** Every crate in the declared public set passes
  `cargo publish --dry-run` cleanly (today: 1/103).
- **CR-K2** A canonical in-repo manifest (`crates/_public.toml`) lists the
  external-publication set; each has non-empty `description`, `license`,
  `repository`, `readme`.
- **CR-K3** No public crate depends on a `publish=false` crate
  (incl. `workspace-hack`) at publish time; intra-workspace deps carry a
  `version` requirement.
- **CR-K4** `voxup install default` produces a **working** `vox` binary
  (real toolchain, not the proxy stub); installed `vox --version` matches
  the release.
- **CR-K5** A documented SemVer/public-API policy exists and is enforced by
  `cargo-semver-checks` in CI for the public set.
- **CR-K6** A `publish-crates.yml` workflow publishes the public set in
  reverse-topological order on `v*` tags, gated on CR-K1.
- **CR-K7** Release binaries ship a verifiable SHA-256 checksum/provenance
  manifest (`vox-checksum-manifest` / `vox-release-artifacts` landed).

**Recommended public set (individually useful to external Rust devs),
in dependency order:** `vox-crypto` (already clean), `vox-jsonschema-util`,
`vox-telemetry`, `vox-journal`, `vox-git` (pure-Rust gix bridge),
`vox-grammar-export`, `vox-db-types` → `vox-db`. The compiler/orchestrator
crates stay internal until their API surface is deliberately frozen.

### 4.5 The GUI tier (CR-U1…U6) — "VOX GUI installs and every surface works"

- **CR-U1** `vox ci gui-surface-registry` runs as a **required** CI gate and
  fails on drift (today: runs nowhere).
- **CR-U2** Every `live_backend`/`curated_decorator` surface renders a
  non-empty, non-error panel in a headless run of the **actual vox-gui
  frontend** (not the emitted codegen app); test count == registry count.
- **CR-U3** vox-gui's own Playwright e2e + vitest suites run as required CI
  gates.
- **CR-U4** `tauri.conf.json` produces installers for all three platforms
  with a complete icon set; a CI dry-run yields a non-empty bundle.
- **CR-U5** `release-gui.yml` builds the sidecar binaries before bundling and
  **signs + verifies** the installer using existing paths (no `src-tauri/`).
- **CR-U6** A launch+IPC smoke test proves the packaged app starts and the
  **real** invoke handlers answer (not the mocked e2e bridge).

### 4.6 Re-tier the existing criteria as the Product tier

Keep CR-P/A/E/D/L verbatim, relabel them **Product tier**, and place them
**after** CR-F/CR-K/CR-U under CR-F0 ordering. Flag the four that an LLM
**cannot self-complete from a checkout** — **CR-P1** (3 apps live on OCI),
**CR-P2** (7-day external uptime), **CR-P3** (live deploy loop), **CR-E3**
(training-loss parity vs PyTorch) — as `external_infra: true` so the agent
doesn't burn cycles trying to satisfy them by writing code.

## 5. Open decisions (Adapt — what I need from you before implementing)

These are genuine forks where your steer changes the work:

1. **Supported-arm matrix.** Is `--mode script` (codegen-rust execution) a
   **shipped, must-be-at-parity** arm for v1.0, or interpreter-primary with
   codegen as best-effort? CR-F2's scope depends entirely on this. (Same
   question for codegen-ts: parity-gated, or "emits, you verify downstream"?)
2. **db.* and intra-project imports.** These execute **only** in the
   interpreter today. For v1.0: implement them in the script/ts arms (big
   lift), or **explicitly reject** them there with a stable diagnostic
   (CR-C6 / fast)? "Silently accepted-and-wrong" is the one outcome CR-F
   forbids.
3. **Public crate set boundary.** Confirm the §4.4 list, or name a different
   set. This decides how much `license`/metadata/`workspace-hack`-stripping
   work CR-K entails.
4. **Convergence bar hardness.** Is CR-F5 ("no first-time semantics in the
   release commit; bug-rate must bend down") a **hard GA blocker**, or an
   advisory dashboard? It is the single most honest "is the language done"
   signal but it gates on **trend**, not a point-in-time command.

## 6. Highest-value next steps (Prioritize)

Ordered by leverage. The first three are the keystone; nothing else is
trustworthy until they exist.

1. **Build the behavioral substrate (CR-F1).** Generalize the HEAD HumanEval
   `--mode interp` runner into a `// EXPECT`/stdout golden harness over
   `examples/golden`. This is Track A of the already-written
   [2026-06-02 golden-corpus plan](../../superpowers/plans/2026-06-02-vox-golden-corpus-and-compiler-reality.md);
   it is designed but **unbuilt**. Without it, "green" never means "runs."
2. **Add the cross-arm differential gate (CR-F2).** One test that runs each
   executable golden through interp **and** script and asserts byte-equal
   output. This is what catches the four-implementations divergence the
   commit history is fighting builtin-by-builtin.
3. **Wire CR-F0 ordering into `vox audit --gate all`.** Foundation gates
   evaluate first; product gates report `blocked_by_foundation` while any
   CR-F is red. Reuses existing registry + roll-up machinery.
4. **Stand up the language-surface coverage checklist (CR-F3).** Turn "is the
   language done?" into a machine-answerable list of constructs → fixtures.
5. **Unblock one publishable crate path (CR-K1–K3).** Pick `vox-crypto` +
   `vox-jsonschema-util` + `vox-journal`, declare `crates/_public.toml`, add
   `license.workspace = true`, and prove `cargo publish --dry-run` clean for
   the set — establishes the pattern for the rest.
6. **Make the GUI's existing gate bite (CR-U1) and fix the three packaging
   blockers (CR-U4/U5).** The hardest GUI work is already done; it is
   unshippable for config/CI reasons, not missing features.
7. **Adopt the CR-META format and the lint** so every future criterion is
   born LLM-drivable, then re-tier the existing CR-P/A/E/D/L behind CR-F0.

> The unifying principle: **today's criteria measure what is built *on* Vox;
> the missing tier measures whether *Vox itself* runs. Add the foundation
> tier, order it first, and make every criterion a command an LLM can run —
> and the criteria document becomes the engine that finishes the language.**
