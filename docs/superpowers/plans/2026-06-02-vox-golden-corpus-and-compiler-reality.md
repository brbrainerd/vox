# Vox Golden Corpus & Compiler-Reality Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Many tracks are designed to run as parallel **Workflow** fan-outs (see §"Workflow Execution Recipes for Opus 4.8"). Each `make-compiler-real` task that touches compiler internals follows the TDD micro-loop: write the failing test → run it (confirm fail) → minimal impl → run (confirm pass) → commit.

**Goal:** Make every Vox `.vox` "doc-box"/golden file compile **and** produce verified, sensible input→output; close the interpreter/codegen/typecheck tier-divergences so the language is "real" in both run modes; and fill the language-coverage gaps that most weaken MENS training — driven by a parallel, workflow-based execution model.

**Architecture:** Nine workstreams ("tracks"). Track A builds the missing **behavioral-verification substrate** (output fixtures + dual-runtime gate) that every other track depends on for proof-of-correctness. Tracks B/C/D close **verified compiler tier-divergences** (constructs that typecheck and run under one runtime but silently misbehave under the other). Tracks E/F **repair and extend the golden corpus** (fix tautological/stub/stale goldens; add goldens for genuinely-shipped-but-untrained features). Tracks G/H/I cover **HumanEval integrity, doctest discipline, and remaining `.vox` trees**. Priority is by *MENS training importance* × *implementation reality gap*.

**Tech Stack:** Rust workspace (`crates/vox-compiler` lexer/parser/HIR/typeck/eval, `crates/vox-codegen` Rust+TS+WebIR emit, `crates/vox-actor-runtime` native builtins, `crates/vox-audit` CR-L gates, `crates/vox-integration-tests` harnesses); `.vox` golden corpus under `examples/golden/`; HumanEval suite under `contracts/eval/humaneval-vox/`; `rust_decimal`, `insta` snapshots, `nextest`. Verify locally with `vox ci pre-push --full --since <ref>` before any push.

---

## Execution log & plan deltas (updated 2026-06-03)

Disciplined execution pass: every plan premise re-verified against live source/binary before acting. Changes land in the worktree (uncommitted, ready for review).

**DONE (verified):**
- **I1 — codegen warnings → 0.** Removed the unreachable `_ => "any"` arm at `codegen_ts/rn/component.rs:1417` (Named/Generic/Function/Tuple/Unit/Decimal are exhaustive over `HirType`) and the spurious `mut` on three closure bindings (`vox_client.rs:798`, `reactive.rs:884` — the build originally reported **3** warnings, not 2). `cargo build -p vox-codegen` → **0 warnings**.
- **G1 — HumanEval CR-L1 gate is now behavioral.** `crates/vox-audit/src/subcommands/humaneval.rs` now runs `tests.vox` under `vox run --mode interp` (new `run_file` helper) in addition to `vox check`; a false oracle / stubbed reference now fails the gate. Added regression test `humaneval_executes_tests_and_fails_false_oracle` (passes: false-oracle drops pass=1/2=0.5). **Real corpus re-run: `overall_pass_rate: 1.0` (164/164, bar 0.80 met)** — no CI break, and the metric is now an honest behavioral pass rate.

**PRUNED (premise was wrong — verified against code/binary):**
- **A4 — "`vox run` exits 0 on AssertionFailed" is FALSE for `--mode interp`.** `run.rs:73-79` already propagates `EvalError` via `?`; empirically `vox run --mode interp` on a false-assert `main()` → **exit 1** (true assert → exit 0). `assert` returns `None`→`EvalError::AssertionFailed` (`builtins.rs:1946`/`expr.rs:290`). **A4 collapses to: verify `--mode script` parity only** (the always-available interp path is already correct). G1 therefore had **no A4 dependency** and the `pass_rate`→`validity_rate` rename is **dropped** (the rate is now genuinely behavioral, so the original name is honest).
- Confirmed by binary: `vox check` on a false-assert file → "Check passed", exit 0 → this is exactly why G1 (execute, don't just check) was the right fix.

**ADAPTED:**
- **A1 — implement as a subprocess harness over `vox run --mode interp`**, comparing captured stdout to `// EXPECT:` comments via `assert_cmd`, rather than threading a capture sink through the interpreter. Reason: `print` is a free fn (`builtins.rs:1937` `call_global_builtin`) with no interpreter-state access; the subprocess form tests the real CLI path and needs no interpreter refactor. (The `// EXPECT` convention and `assert_cmd` pattern are unchanged.)

**NEW findings (fold into Track I):** pre-existing warnings outside I1's scope — `vox-code-audit/src/review/client.rs:6` unused `std::time::Duration`; `humaneval.rs:284` useless `corpus_size >= 0` (u32) in the smoke test. Low priority; not yet fixed to avoid scope creep.

**Build-economics note for executors:** each `cargo build/test` on this workspace is ~2–19 min cold / ~1–3 s warm. Batch compiler edits per crate before rebuilding; prefer `--since`-scoped tests; reuse the prebuilt `target/debug/vox.exe` for `.vox` integration checks. This is why corpus tracks (E/F) — which only touch `.vox` files and reuse the prebuilt binary — are the cheapest to parallelize and should follow A2.

### Pass 2 (2026-06-03) — verified increment

- **I1 + G1 confirmed green by build:** `cargo build -p vox-codegen` → **0 warnings**; `vox-audit` builds clean (only the pre-existing `std::time::Duration` unused-import warnings remain — already logged).
- **NEW corpus gate gap + 2 flagship goldens fixed.** Ran full `vox check` (typecheck) over all 62 goldens: **2 FAILED** — `crud_api.vox` and `iot_telemetry.vox`, both `E0001: field 'id' missing` on `db.insert`. No harness runs full typecheck over the corpus (`golden_vox_examples_test` is parse+lower only), so typecheck-broken goldens ship **training-eligible**. **Fixed both**: removed the redundant explicit `id` PK to match the canonical `getting_started.vox` idiom (*not* fake `id: 0`), and **de-stubbed `iot_telemetry`'s `return 42.0` mock (V6)** into a real `match db.DeviceLog.all() { Ok(logs) => <average over entries> }`. **Sweep now 62/62 pass `vox check`.** (Verified with prebuilt `vox.exe`; no rebuild needed.)
- **PRUNED — scout claims that did NOT reproduce against the binary:** **E5** (`inventory_rosetta_platform.vox` undefined `has_capability`) and **E6** (`ref_actors.vox` `str+int`) both pass `vox check` cleanly. Removed from scope. (Reaffirms the verify-every-claim discipline.)
- **NEW verified findings:**
  - **V11 (make-real candidate):** `db.insert` unifies the record literal against the *full* table type via strict record unification (`typeck/unify.rs:267`) with **no auto-PK exemption** for `id`. (`form_check.rs:54`'s "auto-supplied defaults exempt" is `@form`-label logic, unrelated.) So any `@table` declaring an explicit `id` PK + an `id`-less `insert` fails typecheck. Decide: exempt auto-`id` on insert (lets goldens showcase explicit PKs) vs. keep the implicit-`id` idiom and forbid explicit `id`.
  - **V12 (make-real candidate):** `db.Table.all()` is typed `Result[List[Record]]`; a bare `let logs = db.T.all(); for e in logs { e.field }` fails type inference — field access requires an explicit `match Ok(logs)` unwrap. Idiomatic, but a sharp edge; worth a golden + possibly inference help.
  - **NEW TASK A5 (P0, supersedes part of A1):** add a `vox check` (full typecheck) gate over **all** goldens in `vox-integration-tests` — it would have caught both breakages and is cheaper/more universal than the `// EXPECT` harness. Promote above A1.

### Pass 3 (2026-06-03) — C1 (Result[T,E]) landed; D4/D5/G4 done

- **C1 (Result[T,E]) DONE — typed error slot.** Changed `Ty::Result(Box<Ty>)` → `Ty::Result(Box<Ty>, Box<Ty>)` and threaded `E` through all **98 sites across 11 files** (compiler-guided: enum change → fix every error). The 78 builtin-signature constructors got `, Box::new(Ty::Str)` via a balanced-paren perl pass (no Result-in-Result nesting, so one pass; verified 0 single-arg remaining); the ~20 semantic sites (lowering in `registration.rs`/`ast_decl_lints.rs` thread the real 2nd arg; `unify`/`resolve`/`occurs`/`instantiate` recurse into both ok+err; **`Err`/`Error` pattern now binds the real `E`, not hardcoded `str`** at `expr_ops.rs`) were hand-fixed. `Result[T]` still defaults to `Result[T, str]` (behavior-preserving) and displays as `Result[T]`. TDD: `result_error_arm_binds_declared_error_type` (a `Result[str, int]` whose `Error(code)` arm must type `code` as int) — green. **Full regression: vox-compiler builds; match_exhaustiveness 4/4; interpreter 8/8; golden parse/lower; typecheck gate 2/2; @test runner 62 tests/71 files; `vox check` 71/71; HumanEval 164/164.** Zero corpus breakage. (Remaining C1 follow-on: error-ADT *variant* exhaustiveness on the `Error` arm; `eval/value.rs` Err side is still `String` — both noted in the C1 reconciliation spec. `infer.rs` was kept compiling, not deleted — C5 still pending.)
- **NEW FINDING (interp bug, pre-existing, NOT C1):** `someOption is None` returns **false** in the interpreter even when the value is `None` — a `None`-literal-vs-`Option(None)` representation mismatch in `eval` equality (`match` on the Option works; `is None` equality doesn't). Reproduces in the pure `vox run --mode interp` binary (which skips typecheck). Surfaced by `regex_free_functions.vox::test_replace_and_captures` (a Track-F golden whose `@test` the workflow agents never executed — they verified `main()` via `vox run`, but `@test` fns only run in the Rust harness). Fixed the golden to use idiomatic `match` (better training anyway); the interp `is None` bug is a real make-real gap to fix later (likely `eval/expr.rs` None-literal eval + `value.rs` PartialEq Option arm).

### Specs for the spec-gated remainder (written 2026-06-03)

The no-spec-required work (B1–B4, C2, A5, I1, Track E, Track F, G1–G4, E8, hakari) is landed + committed (5 commits on `cc_bdesktop2/distracted-villani-a55822`). What remains genuinely needs design first; specs written:

- **B5** → [`specs/2026-06-03-db-query-plan-typecheck-design.md`](../specs/2026-06-03-db-query-plan-typecheck-design.md) — `.where/.filter/.order_by/.limit/.select` typed chaining. Corrects the scout's "one-line guard removal" (empirically `.where` itself fails typecheck). Step 0 = verify the codegen side.
- **C1** → [`specs/2026-06-03-result-two-param-reconciliation.md`](../specs/2026-06-03-result-two-param-reconciliation.md), an addendum to the canonical [`specs/2026-05-18-result-two-param-design.md`](../specs/2026-05-18-result-two-param-design.md). Records: C2 done, `@endpoint` retired, `infer.rs` dead (2 live lowering paths not 3).
- **Track D** → [`specs/2026-06-03-durable-codegen-de-noop-design.md`](../specs/2026-06-03-durable-codegen-de-noop-design.md) — actor dispatch no-op (confirmed `durability_lower.rs:119`), workflow `DefaultTracker`, activity journal. D3 (actor dispatch) first.

### Pass 2 (continued, 2026-06-03) — A5 + G2 + G3 landed (all verified)

- **A5 DONE.** New `crates/vox-integration-tests/tests/golden_typecheck_gate.rs`: runs `typecheck_module` over all 62 goldens (the same typecheck `vox check` uses), fails on any Error-severity diagnostic, **plus** a `gate_actually_catches_type_errors` self-test (a `let x: int = "s"` mismatch) proving it is not a no-op. Result: **`62 golden files typecheck clean ✓` + self-test pass** (`2 passed; 0 failed`). This permanently closes the "golden ships type-broken" gap.
- **`crud_api.vox` + `iot_telemetry.vox` DONE** → corpus is **62/62 on `vox check`**. `iot_telemetry`'s `42.0` mock is gone (real `match Ok(logs)` average).
- **V11 RESOLVED by codegen reality (no compiler change; strong typing stands).** Investigated `crates/vox-codegen/src/codegen_rust/emit/tables/codegen.rs`: every table's PK is an **auto `_id INTEGER PRIMARY KEY AUTOINCREMENT`** (`:342`); `insert` (`:111-144`) writes only user columns and returns `last_insert_rowid()`; `get`/`delete` query `WHERE _id = ?` (`:152,:265`). So a user-declared `id: int` is **not** an auto-PK — it is an ordinary column (disconnected from the real `_id` lookup key) that `insert` must populate, and `typeck/builtins.rs:1549` correctly requires it. An `Insert[T]` "minus id" exemption would be **wrong** (it would let a real column be silently omitted). **Decision: keep strong typing exactly as-is at the admission boundary; do NOT exempt `id`.** The canonical idiom is "don't declare a redundant `id`; the auto `_id` is the key" (as `getting_started.vox` does). **The `id`-removal in `crud_api`/`iot_telemetry` is the correct, final fix — not a stopgap.** V11 closed.
- **Track E (conservative) DONE via parallel Workflow** (`vox-corpus-metadata-delog`, 5 agents, worktree-free disjoint-file fan-out). Added `difficulty` to ~48 goldens; added full STYLE.md frontmatter blocks to the ~12 that had none (`decimal_math`, `json_as_typed`, `mesh/noop`, `mobile_camera`, `mobile_test`, `reactive_counter`, `ref_actors/agents/orchestrator/syntax/types`, `std_http_wrappers`); replaced stale `@endpoint` in `auth_patterns`/`background_jobs` `constructs:` lists with `@mutation`. **Independently verified:** 62/62 `vox check`, and a `git diff` confirms the *only* non-comment code change in the entire golden tree is the `iot_telemetry` average — every Track E edit is comment/frontmatter-only (zero logic changes). Covers tasks **E7, E9** and the metadata half of **E10**.
- **B1 (V1 closed) DONE — exact decimal in the interpreter.** Added `VoxValue::Decimal(rust_decimal::Decimal)` (`eval/value.rs`) with `DecimalLit → from_str_exact` (`eval/expr.rs`), Add/Sub/Mul/Div (zero-guarded) + Lt/Gt/Lte/Gte + unary-neg arms, PartialEq (rust_decimal Eq treats `8.2500 == 8.25`), and `vox_value_display`/`vox_value_type_name` arms. TDD: added failing test `decimal_arithmetic_is_exact_in_interp` (was `Bool(false)`), then green. **Verified end-to-end:** `vox run --mode interp examples/golden/decimal_math.vox` → "Decimal verification successful", exit 0 (its `0.1+0.2 is 0.3`, tax, and 17-digit precision asserts now hold under interp — previously silently false). Regression sweep green (interpreter_test 5/5, typecheck gate 2/2, @test runner + parse/lower). `vox.exe` rebuilt. **Dual-runtime `dec` divergence eliminated.** Next: B2 compiled-regex, then B3/B4 http/time interp dispatch.
- **B2 (V8 closed) DONE — compiled-regex object in the interpreter.** Added `VoxValue::Regex(regex::Regex)` + `VoxValue::Match(Vec<Option<String>>)` (`eval/value.rs`), changed `std.regex.compile` to return `Result[Regex]` (was a bare `Str`), and added `re.matches/find/find_all` + `m.group/groups` method dispatch in `call_builtin_method` (`eval/builtins.rs`), plus PartialEq/type_name/display arms. Typeck already supported the API (golden passes `vox check`), so this was interp-only. TDD: `compiled_regex_find_group_in_interp` (the `re.find(...).group(1)` idiom from `regex_stdlib.vox`) — red → green. Interpreter suite 6/6, typecheck gate 2/2, @test runner, `vox check` 62/62 — no regression. `vox.exe` rebuilt. **`regex_stdlib.vox`'s taught idiom now runs under `--mode interp`.**
- **E8 DONE — both stale `// vox:skip` removed; corpus now has ZERO skipped goldens (all 71 fully validated).** `db_native_ir.vox`: skip claimed it "exercises the planned chained-query surface" but the body uses only `all()`/`len()` — stale; removed, passes `vox check`. `index_showcase.vox`: skip cited raw-SQL `db.query` + `Field 'id' not found`, but the body no longer uses raw-SQL (only the comment did) and its `Id[Task]` param typechecks (`Id[T]→int`) — stale; removed, passes `vox check`.
- **B5 RE-SCOPED (scout claim corrected, verified empirically).** The scout said "remove the typeck:381 guard and order/limit reach working codegen." **False on the typeck side:** empirically `db.T.where({...})` itself fails typecheck with `E0001: Method 'where' not found on Table` (the Table method table has no where/filter/order_by/limit/select; `opt_plan` is referenced ONLY by the guard at expr.rs:381 — the predicate path `validate_db_predicate` is wired to a *different* db-scoped-handler construct, not the `.where()` method chain). So making `db.T.where().order_by().limit()` typecheck is a **real db-query-plan TYPE feature** (recognize the chainable methods on `Ty::Table`/a query type, return a chainable result type, thread predicate validation) — **spec-gated, not a one-line removal.** Codegen reportedly exists, but the typeck surface does not. Reclassified from "20-min fix" to a spec→impl task. db_native_ir.vox did NOT need the chained surface (its body is all()/len()), so E8 un-skipped it independently.
- **Track F (9 new goldens) DONE — parallel Workflow (9 agents).** All 9 pass independent `vox check` + `vox run --mode interp` verification (71/71 golden sweep clean). Files written: `while_loop_algorithms.vox` (collatz+power-of-2 accumulators, loop+break search), `closures_hof.vox` (make_adder, map/filter/fold chain, sorted_by_key — training_weight:1.5), `string_interpolation.vox` (multi-part interpolation in loops, str method chaining), `range_and_indexing.vox` (range(n)/range(s,e) in for-loops, let-mut + index-assign), `regex_free_functions.vox` (cross-tier-safe is_match/captures/replace), `tuple_destructure.vox` (tuple construction/destructuring in let+match, indexed for), `adt_multi_field.vox` (3-variant Shape ADT with multi-field constructors, exhaustive match — training_weight:1.5), `process_run.vox` (std.process.run + run_capture guard pattern), `env_and_path.vox` (std.env.get, std.path canonical file_name/parent/extension/stem). Covers tasks F1a/b/c/d, F4, F5a/b, F7a/b.
- **G4 DONE — 164 orphan HumanEval dirs removed (492 files).** Orphan dirs (inner `spec.toml`, `@test/is` convention, unreferenced by manifest — 0 manifest overlap verified both by grep and by manifest slug cross-reference) were corrupting every `find`-based scan and doubling the problems/ directory. Post-deletion: 0 orphan dirs, 164 live dirs intact, manifest-authoritative execution scan PASS=164 FAIL=0. `vox audit humaneval` overall_pass_rate=1.0.
- **B4 (V7 partial) DONE — `std.time` interp dispatch.** Added a `Some("time")` namespace arm (`now_ms`/`now` → `SystemTime` epoch ms) in `call_builtin_method`; `std.time.now_ms()` was typeck+codegen-declared but had no interp arm (errored as unreachable). TDD: `std_time_now_ms_runs_in_interp` (asserts positive Int) — green. Suite 7/7. No new dep (std only). **B3 (V7 closed) DONE — REAL HTTP in the interpreter** (maintainer direction: "Vox is a web-app language; add the HTTP dependency to the compiler"). Added `reqwest = { workspace=true, features=["blocking"] }` to `crates/vox-compiler/Cargo.toml` (inherits the workspace rustls-tls stack). Implemented `std.http.get_text(url)` / `post_json(url, body)` (both `→ Result[str]`) in the `Some("http")` arm via `reqwest::blocking` run on a **dedicated std::thread** — required because the interpreter executes inside the CLI's tokio runtime and `reqwest::blocking` panics if built in an async context (mirrors the codegen `vox_http_*` worker-thread pattern). TDD: replaced the prior graceful-error test with `std_http_get_text_performs_real_request_in_interp` (empty URL → real reqwest `Result::Err`, not a stub) — green; suite 8/8. Reverted `std_http_wrappers.vox`'s "script-only" note (it now runs in both tiers). **`std.http` runs identically under `--mode interp` and `--mode script`. Track B complete (B1–B4).** Follow-up: `workspace-hack` (cargo-hakari) may need `cargo hakari generate` for the new `reqwest/blocking` feature before push (CI hakari-check); local build is green.
- **hakari follow-up DONE.** Ran `cargo hakari generate` → `workspace-hack/Cargo.toml` updated (+20/-12) to unify the new `reqwest/blocking` feature. CI hakari-check will pass.
- **C2 (V9 partial) DONE — Option/Result match exhaustiveness, hard error, ZERO blast radius.** `typeck/checker/match_exhaust.rs` only checked `Bool`+named ADTs; `Ty::Option`/`Ty::Result` fell through unchecked. Added `check_builtin_match_exhaustiveness` (Option=`Some`/`None`; Result=`Ok`/`Err`|`Error`; binding-ident or wildcard covers the rest) emitting `E0301`. TDD: `match_exhaustiveness_test.rs` (3 tests: non-exhaustive Option **and** Result rejected, exhaustive+binding accepted) — green. **Measured blast radius before committing to error severity: 62/62 goldens pass, 656/656 HumanEval `.vox` pass, 0 newly-flagged** — the corpus was already exhaustive, so this is pure strong-typing hardening with no disruption. `vox.exe` rebuilt. (C1 `Result[T,E]` two-param representation remains the larger, spec-gated type-system item.)
- **G2 DONE (V4 fixed).** Restored `build_held_out_manifest` in `humaneval.rs` (derives held-out from manifest `training_eligible: false`; blake3 fixture hashes; manifest sha256 `corpus_hash`). `cargo test -p vox-audit --test regen_held_out -- --ignored` now compiles + runs (was E0425). Regenerated `held-out.v1.json`: **total 164, held-out 31, `corpus_hash: sha256:8a2f…`** — replacing the stale 10-orphan-slug / blake3 file.
- **G3 (partial) DONE + 1 open policy item.** The regen surfaced the true held-out count = **31** (not the declared 44). Fixed `manifest.v1.yaml` `held_out_current: 44 → 31`. **OPEN (maintainer decision):** `held_out_target: 30` vs actual **31** — drop one problem from held-out to hit 30, or accept 31 and bump the target. Not changed pending your call (it's a contamination-policy knob, not a bug).

---

## 0. How this plan was produced (provenance)

Produced from a 19-agent parallel scout (`vox-golden-doc-scout` workflow: 8 per-file corpus auditors + 4 `.vox`-tree auditors + 6 compiler-reality domain auditors + 1 synthesizer; ~2.18M subagent tokens, 720 tool calls) reconciled against two **live** Rust harness runs and **independent hand-verification** of every load-bearing bug claim. Do not weaken a task because "the scout said so" — each compiler bug below was confirmed against source at the cited `file:line`.

### Ground truth (live harness runs, 2026-06-02, v0.6.0)

- `cargo test -p vox-compiler --test golden_vox_examples_test` → **PASS**: all 62 goldens parse + lower + WebIR-validate + runtime-project.
- `cargo test -p vox-integration-tests --test golden_vox_test_runner` → **PASS**: `28 @test functions across 62 golden files: all passed`.
- **But**: only **14 of 62** goldens contain any `@test`; the runner *skips every file without `@test`*, so **48/62 goldens have zero execution coverage** (parse-only). **0/62** verify stdout output. A repo-wide grep finds **no stdout/output-golden harness** anywhere in the Rust tree. This is the central gap the plan's Track A closes.

### Hand-verified compiler bugs (each confirmed at source)

| # | Claim | Location | Verified |
|---|-------|----------|----------|
| V1 | `dec` literals lower to `f64` in interpreter (comment admits "interp approximates as Float") → `decimal_math.vox` exact-equality asserts are **false under `--mode interp`** | `crates/vox-compiler/src/eval/expr.rs:79-85` | ✅ read |
| V2 | Typecheck **hard-rejects** `.limit`/`.order_by` db chaining ("not supported yet") although HIR-lowering + Rust codegen for it are complete → working codegen unreachable; `db_native_ir.vox` is `// vox:skip` because of it | `crates/vox-compiler/src/typeck/checker/expr.rs:381` | ✅ read |
| V3 | `@inference` is parse-only: `inference_model` has **0 occurrences in `vox-codegen`**; sets `inference_model` but not `is_llm`; model silently dropped | `vox-codegen` (0 hits) + `parser/descent/decl/mid.rs:1056` | ✅ grep |
| V4 | `regen_held_out.rs` is a **compile break**: calls `build_held_out_manifest`, which has **0 occurrences in `vox-audit/src`** | `crates/vox-audit/tests/regen_held_out.rs:11` | ✅ grep |
| V5 | HumanEval CR-L1 gate runs **`vox check` (typecheck) only**, never executes `assert` bodies, yet reports `overall_pass_rate` (implies behavioral pass@k). `vox check` passes a deliberately-false assertion | `crates/vox-audit/src/subcommands/humaneval.rs:117` | ✅ read |
| V6 | `iot_telemetry.vox` ships `return 42.0 // Mock aggregate` in a **training-eligible golden** (stub-in-corpus; forbidden by AGENTS.md §Structural Limits + no-stubs policy); plus false "filter() is not yet implemented" header comment | `examples/golden/iot_telemetry.vox:51,13` | ✅ read |
| V7 | `std.http` / `std.time` typecheck + codegen but have **no interpreter dispatch arm** → unreachable under `--mode interp` (`std_http_wrappers.vox` is script-mode-only, unlabeled) | `crates/vox-compiler/src/eval/builtins.rs` (`call_builtin_method`) | ⚠️ scout-asserted; Track A2 dual-runtime gate confirms empirically |
| V8 | `std.regex` compiled-object idiom (`re.find().group(1)` — the form `regex_stdlib.vox` teaches) returns a **bare string in interp** (no `.find/.matches/.group`) but a real `VoxRegex/VoxMatch` in codegen | `crates/vox-compiler/src/eval/builtins.rs:1757-1772` | ⚠️ scout-asserted; Track A2 confirms |
| V9 | `Ty::Result` carries **only a success type**; second type arg dropped (`Result[T,E]` checks as `Result[T]`); `Err` payload hardcoded to `Ty::Str`; `Option`/`Result` matches are **not exhaustiveness-checked** | `typeck/registration.rs:256`, `checker/expr_ops.rs:233`, `checker/match_exhaust.rs:19` | ⚠️ scout-asserted; verify before C-track |
| V10 | Emitted codegen for durable workflows/activities/actors is **no-op**: workflows use `DefaultTracker` (journal nothing), activities wrap a `cfg(test)`-only `journal::execute`, actor mailbox binds the envelope to `_` (drops every message) | `crates/vox-codegen/.../durability_lower.rs:40,111` + `journal/execute.rs:15` | ⚠️ scout-asserted; verify before D-track |

> **The inversion insight that shapes priority:** the compiler is *more real than the corpus reveals*. Shipped-but-untrained features (full db predicate AST, `Async[T]` views, HMAC-cursor `Paginated[T]`, model-agnostic LLM dispatch, interpreter durable-replay) sit beside a few genuinely-fake markers (`@inference`/`@training_step`/`@distributed_train`) and the tier-divergences above. So a large fraction of value is **making typecheck/interp agree with the codegen that already exists** and **adding goldens that exercise shipped features**, not net-new engines.

---

## 1. Execution model — workflow-driven parallelism for Opus 4.8

This plan is built to be executed by **parallel Opus 4.8 subagents via the `Workflow` tool**, not one agent serially. The rules:

1. **Track A is the keystone and runs first, mostly serial.** Every other track's "definition of done" is "the dual-runtime gate (A2) and/or output fixture (A1) proves the behavior." Do A first.
2. **File-disjoint tracks fan out concurrently.** Track E (each golden file is independent) and Track F (each new golden is independent) are *embarrassingly parallel* — run them as `pipeline()`/`parallel()` fan-outs with **`isolation: 'worktree'`** so concurrent file writes never collide. Track H (doc files) likewise.
3. **Compiler-internal tracks (B/C/D) are file-overlapping** (`eval/builtins.rs`, `typeck/*` touched by many tasks) → run those tasks **sequentially within a track** but **tracks B vs C vs D in parallel** (they touch different modules: B=`eval/`, C=`typeck/`, D=`codegen/durability`). Use one worktree per track.
4. **Every task ends green or it isn't done.** Each compiler task adds a Rust `#[test]`; each golden task is proven by A2 (assertion-pass under *both* `--mode interp` and `--mode script`) and, where it prints, by an A1 `// EXPECT:` fixture.
5. **No silent caps.** If a fan-out skips a file (e.g., a worktree conflict), `log()` it; never report a track "done" with un-run items.

See §"Workflow Execution Recipes" for the concrete `Workflow` scripts.

### Priority order (by training importance × reality gap)

1. **P0 — Track A** (behavioral verification substrate). Nothing is trustable without it.
2. **P0 — Track G1/G2/G4-investigate** (HumanEval gate executes tests; fix the dead regenerator). The CR-L1 number is currently a validity rate mislabeled as pass@k.
3. **P1 — Track B** (verified tier-divergences: dec, regex, http/time interp, db `.order_by/.limit` unblock). These make the *shipped* language run identically in both modes — highest "make it real" leverage.
4. **P1 — Track E** (repair corpus: kill stubs/tautologies, add tests, fix frontmatter, de-log). Direct training-data quality.
5. **P2 — Track F** (new goldens for shipped-but-untrained: imperative core, db predicates, `Async[T]`/`Paginated[T]`, MENS decorators). Fills the worst coverage holes.
6. **P2 — Track C** (Result[T,E] + exhaustiveness) and **Track D** (durable codegen de-no-op) — spec-gated, larger design surface.
7. **P3 — Tracks H/I** (doctest discipline, remaining `.vox` trees, codegen warnings).

---

## 2. Master task index (the complete enumeration — "nothing forgotten")

Legend — **Imp**: training importance (Crit/High/Med/Low). **‖**: parallelizable. **Gate**: spec-gated (needs a design note first). Detailed task specs follow in §3; compact tasks carry file+approach+test+verify inline in §3 too.

| ID | Track | Task | Files (primary) | Imp | ‖ | Dep |
|----|-------|------|-----------------|-----|---|-----|
| A1 | A Output-infra | `// EXPECT:` golden-output fixture harness | `crates/vox-integration-tests/tests/golden_output_fixtures.rs` (new) | Crit | – | – |
| A2 | A | Dual-runtime (interp+script) assertion-pass gate over all goldens | `crates/vox-integration-tests/tests/golden_dual_runtime.rs` (new) | Crit | – | A4 |
| A3 | A | Lint: reject tautological tests (identity-return+`len>0`; assert-vs-literal) | `crates/vox-code-audit/src/detectors/tautological_test.rs` (new) | High | ✓ | – |
| A4 | A | `vox run` exits non-zero on `AssertionFailed`/`Panic` | `crates/vox-cli/src/commands/run.rs`, `crates/vox-compiler/src/eval/` | Crit | – | – |
| B1 | B make-real | Interpreter `VoxValue::Decimal` (rust_decimal); fix `dec` literal | `eval/value.rs`, `eval/expr.rs:79-85`, `eval/builtins.rs` | Crit | – | – |
| B2 | B | Interpreter compiled-`Regex`/`Match` value (`re.find/matches/find_all`, `m.group`) | `eval/builtins.rs:1757-1772`, `eval/value.rs` | High | – | – |
| B3 | B | Interpreter `std.http` dispatch arm (`get_text`/`post_json`) | `eval/builtins.rs` (`call_builtin_method`) | High | – | – |
| B4 | B | Interpreter `std.time` dispatch arm (`now_ms`) + verify `uuid`/`hash_*` | `eval/builtins.rs` | Med | – | – |
| B5 | B | Remove typeck `.limit`/`.order_by` block; reconcile Table method table; un-skip `db_native_ir.vox` | `typeck/checker/expr.rs:381`, `typeck/builtins.rs:1546-1593`, `examples/golden/db_native_ir.vox` | Crit | – | – |
| B6 | B | TS codegen real `dec` emission (string-literal → decimal lib) | `crates/vox-codegen-ts/src/...:827` | High | – | B1 |
| B7 | B | `for`-loop iterate `str`/`map`/`tuple` (not just `List`) | `eval/expr.rs:457-487` | High | – | – |
| B8 | B | Call-expr arity check + apply param defaults (or document) | `eval/expr.rs:295-322,265-275` | Med | – | – |
| B9 | B | Fix silent no-op for non-`Ident` index-assign targets | `eval/stmt.rs:219-251` | Med | – | – |
| B10 | B | Dedup `path.basename/dirname` vs `file_name/parent` (remove legacy or alias) | `builtin_registry.rs:513-515,762-769` | Med | – | – |
| B11 | B | Add `split_lines` method (or document `split("\n")` canonical) | `eval/builtins.rs`, `actor-runtime/builtins`, `str_utils.vox` | Med | – | – |
| C1 | C types | Spec+impl `Ty::Result(ok,err)`; thread `E`; bind `Err` payload to real `E` | `typeck/ty.rs`, `registration.rs:256`, `checker/expr_ops.rs:233`, unify/resolve | High | Gate | – |
| C2 | C | `Option`/`Result` match exhaustiveness | `typeck/checker/match_exhaust.rs:19-31` | High | – | – |
| C3 | C | `Id[T]` real newtype OR document as int alias | `registration.rs:275` | Med | Gate | – |
| C4 | C | Validate user-level generic fn end-to-end (+ golden) | `typeck/registration.rs:362`, `unify.rs:96` | High | – | – |
| C5 | C | Remove or quarantine dead `infer.rs` | `typeck/infer.rs`, `typeck/mod.rs:45` | Low | – | – |
| D1 | D durable | Thread real `VoxDbTracker` into emitted workflow body | `codegen .../durability_lower.rs:40-43` | Crit | Gate | – |
| D2 | D | Wire `journal::execute` to a runtime tracker (not `cfg(test)`) | `codegen .../journal/execute.rs:15-46` | Crit | Gate | – |
| D3 | D | Emit real actor `Envelope` dispatch table | `codegen .../durability_lower.rs:111-121` | High | Gate | – |
| D4 | D | Reclassify `saga_compensation.vox` as manual compensation | `examples/golden/saga_compensation.vox` | Med | ✓ | – |
| D5 | D | `scheduled_tick.vox` → real `@scheduled` syntax (drop false E028 claim) | `examples/golden/scheduled_tick.vox` | High | ✓ | – |
| D6 | D | Broaden determinism-lint blocklist + add negative golden | `typeck/.../determinism`, `examples/golden/` | High | ✓ | – |
| D7 | D | `VoxDbTracker` patch + dedup-cache hooks | `crates/vox-db/.../tracker` | Low | Gate | D1 |
| E1 | E corpus | Add `@test` to the ~22 untested goldens | `examples/golden/*.vox` | High | ✓ | A2 |
| E2 | E | Replace tautological tests (6 `ai_fixtures`, `test_suite`, `format_conversion`, `deferred_fill_hole`) | `examples/golden/...` | High | ✓ | A2 |
| E3 | E | Fix `iot_telemetry.vox` 42.0 mock → real aggregate; delete false filter comment | `examples/golden/iot_telemetry.vox` | High | ✓ | B5 |
| E4 | E | Fix `multi_tenancy.vox` cross-tenant leak (scope query or label illustrative) | `examples/golden/multi_tenancy.vox` | Med | ✓ | B5 |
| E5 | E | Fix `inventory_rosetta_platform.vox` undefined `has_capability` | `examples/golden/inventory_rosetta_platform.vox` | Med | ✓ | A2 |
| E6 | E | Fix `ref_actors.vox` `str+int` + stale `TASK-2.6` | `examples/golden/ref_actors.vox` | Med | ✓ | A2 |
| E7 | E | Frontmatter: add `difficulty` to ~38; full block to ~13 no-frontmatter goldens | `examples/golden/*.vox` | Med | ✓ | – |
| E8 | E | Remove stale `// vox:skip` (`db_native_ir` after B5, `index_showcase`) | `examples/golden/...` | Med | ✓ | B5 |
| E9 | E | Fix stale `@endpoint` in `constructs:` frontmatter (`auth_patterns`, `background_jobs`) | `examples/golden/...` | Low | ✓ | – |
| E10 | E de-log | Remove debug `scratch_extract.vox` write + audit per-file `delog_issues` | `docs/src/explanation/expl-rosetta-inventory.md`, goldens | Med | ✓ | – |
| E11 | E | Fix `agent_pipeline.vox` body/frontmatter mismatch (add actor or fix desc) | `examples/golden/agent_pipeline.vox` | Med | ✓ | A2 |
| E12 | E | Wire `not_found`/`error`/`pending` into `web_routing_fullstack.vox` `routes{}` | `examples/golden/web_routing_fullstack.vox` | Med | ✓ | A2 |
| E13 | E | Upgrade `http_error_mapping.vox` to real `@query` with status-mapped errors | `examples/golden/http_error_mapping.vox` | High | ✓ | A2 |
| F1 | F new-golden | Imperative core: while-loop algo, interpolation, closures/HOF chain, `range()` for-loop, `let mut`+index-assign | `examples/golden/*.vox` (5 files) | Crit | ✓ | A2 |
| F2 | F | db query-plan goldens: `.where/.filter`, `.order_by/.limit`, `Async[T]` (4 arms), `Paginated[T]` | `examples/golden/*.vox` (4 files) | Crit | ✓ | B5 |
| F3 | F | MENS decorators: wire/tombstone `@inference`; goldens + decl-coverage rows for `inference/training_step/distributed_train`; `@llm` golden | `parser`, `codegen`, `contracts/mens/golden_decl_expectations.yaml`, goldens | High | Gate | – |
| F4 | F | Cross-tier-safe regex free-function golden | `examples/golden/regex_free.vox` (new) | High | ✓ | A2 |
| F5 | F | stdlib goldens: `process.run`/`run_capture_json`, `std.path`, `std.env`, `std.time`/`uuid` | `examples/golden/*.vox` (4 files) | Med | ✓ | B4 |
| F6 | F | Web goldens to main corpus: list-render-with-key variants, `safe_area` edges, reactive on-mount `@query` load | `examples/golden/*.vox` | High | ✓ | A2 |
| F7 | F | Type goldens: guard-clause match, tuple destructure, map iteration, multi-field enum match, inferred-return-type | `examples/golden/*.vox` | High | ✓ | C2 |
| F8 | F | `@server`-only golden; `@index`/`@search_index` golden | `examples/golden/*.vox` | Med | ✓ | A2 |
| G1 | G humaneval | Runner executes `tests.vox` after typecheck; rename `pass_rate`→`validity_rate` | `crates/vox-audit/src/subcommands/humaneval.rs:117` | Crit | – | A4 |
| G2 | G | Fix `regen_held_out.rs` compile break (restore/rewrite `build_held_out_manifest`) | `crates/vox-audit/tests/regen_held_out.rs`, `vox-audit/src/subcommands/humaneval.rs` | High | ✓ | – |
| G3 | G | Reconcile held-out counts (10/31/44/30) to single SSOT; regen `held-out.v1.json` (sha256) | `contracts/eval/humaneval-vox/{manifest.v1.yaml,held-out.v1.json}` | High | ✓ | G2 |
| G4 | G | **Investigate (gated)** the 164 orphan seed dirs; verify 0 manifest refs via script, then remove | `contracts/eval/humaneval-vox/problems/` | Med | Gate | – |
| G5 | G | Add 8–12 live problems with `to Option[T]`/`to Result[T]` signatures (real, non-stub) | `contracts/eval/humaneval-vox/problems/` | High | ✓ | G1 |
| G6 | G | Implement vox-corpus held-out contamination guard OR strike the README claim | `crates/vox-corpus/`, `contracts/eval/humaneval-vox/README.md` | Med | ✓ | G3 |
| H1 | H doctest | `doctest.rs:32` require a reason on every `// vox:skip` (190/209 bare) | `crates/vox-doc-pipeline/src/doctest.rs:32` | High | – | – |
| H2 | H | De-skip trivially-compilable basic-syntax doc blocks | `docs/src/reference/ref-syntax.md`, `ref-type-system.md` | Med | ✓ | H1 |
| H3 | H | Fix wrong match-arm thin-arrow syntax in how-to docs | `docs/src/how-to/*.md` | Med | ✓ | – |
| H4 | H | Back `how-to-database`/error-handling docs with golden snippets | `docs/src/how-to/*.md` | Low | ✓ | – |
| I1 | I sanity | Clean codegen warnings (unreachable arm; unused `mut`) | `codegen_ts/rn/component.rs:1417`, `codegen_ts/vox_client.rs:798` | Low | ✓ | – |
| I2 | I | Sweep remaining `.vox` trees (scripts/apps/sandboxes/cli-tour/compile-suite/tests) for compile+run health | repo-wide `.vox` | Med | ✓ | A2 |
| I3 | I | Enforce aspirational "parse-without-panic" contract in a test | `crates/vox-compiler/tests/examples_ssot_test.rs` | Low | ✓ | – |

**Total: 9 tracks, ~63 tasks.** Track E/F/G/H/I are file-disjoint and parallelizable; A/B/C/D are mostly serial-within-track.

---

## 3. Detailed task specs

> Convention for every task: **Files** (exact), **Approach**, **Test (TDD)**, **Verify** (exact command + expected), **Done-when**. Keystone/high-risk tasks expand the full bite-sized step loop; compact tasks state the four fields (the executing agent expands the micro-loop via TDD).

### Track A — Behavioral verification substrate (P0, keystone)

#### Task A4: `vox run` signals assertion failure (prerequisite for A2/G1)

**Files:**
- Modify: `crates/vox-cli/src/commands/run.rs` (map `EvalError::AssertionFailed`/`EvalError::Panic` → non-zero exit)
- Reference: `crates/vox-compiler/src/eval/` (`EvalError` variants)

- [ ] **Step 1 — failing test.** Add `crates/vox-cli-tests` (or `vox-integration-tests`) case: write a temp `.vox` with `fn main() to str { assert(1 is 2); return "x" }`, run it via `assert_cmd` `vox run <tmp>`, assert exit code != 0 and stderr contains the assertion message.

```rust
#[test]
fn vox_run_fails_on_false_assertion() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("bad.vox");
    std::fs::write(&f, "fn main() to str {\n  assert(1 is 2)\n  return \"x\"\n}\n").unwrap();
    let mut cmd = assert_cmd::Command::cargo_bin("vox").unwrap();
    cmd.args(["run", "--mode", "interp", f.to_str().unwrap()])
        .assert().failure();
}
```

- [ ] **Step 2 — run, confirm FAIL** (`vox run` currently exits 0): `cargo test -p vox-cli-tests vox_run_fails_on_false_assertion` → FAIL.
- [ ] **Step 3 — impl.** In `run.rs`, match the interpreter result; on `Err(EvalError::AssertionFailed{..} | EvalError::Panic{..})` print the diagnostic to stderr and return the CLI's error exit code. Mirror existing error-exit handling in the same file. Do the same on the `--mode script` path if it has a separate result branch.
- [ ] **Step 4 — run, confirm PASS.**
- [ ] **Step 5 — commit** `fix(cli): vox run exits non-zero on assertion failure / panic`.

**Done-when:** a false `assert` makes `vox run` exit non-zero in both `--mode interp` and `--mode script`.

#### Task A1: `// EXPECT:` golden-output fixture harness

**Files:**
- Create: `crates/vox-integration-tests/tests/golden_output_fixtures.rs`
- Convention doc: append a "§Golden output fixtures" section to `examples/STYLE.md`

**Approach.** Define an inline fixture convention: a golden may include consecutive `// EXPECT: <line>` comments declaring the exact stdout its `fn main()` (or designated entry) prints. The harness collects every golden, and for each that has `// EXPECT:` lines: lex→parse→lower→typecheck, run via `Interpreter` capturing `print` output (route the interpreter's `print` builtin through a capture buffer — see `eval/builtins.rs` log/print arm; add a `Vec<String>` sink on `Interpreter`), then assert captured stdout equals the joined `EXPECT` lines. Files without `// EXPECT:` are skipped (kept additive). Reuse the collection logic from `golden_vox_test_runner.rs`.

- [ ] **Step 1 — failing test.** Write the harness `all_golden_expect_fixtures_match` modeled on `golden_vox_test_runner.rs` (`collect_golden_vox`, `lex/parse/lower/typecheck`, `Interpreter::new(STEP_LIMIT)`, `run_module`). Add a capture sink (smallest change: have `print` push to `interp.captured_stdout` instead of/in addition to stdout when a capture flag is set).
- [ ] **Step 2 — seed one fixture.** Add `// EXPECT: Hello Vox!` to a trivial new/edited golden that calls `print` and confirm the harness FAILS first (no capture wired), then PASSES once capture is wired.
- [ ] **Step 3 — impl** the capture sink on `Interpreter` and the comparison.
- [ ] **Step 4 — verify:** `cargo test -p vox-integration-tests --test golden_output_fixtures` → PASS.
- [ ] **Step 5 — commit** `test(golden): // EXPECT stdout fixture harness`.

**Done-when:** at least one golden's printed output is pinned and enforced; harness is additive (no-EXPECT goldens skipped).

#### Task A2: Dual-runtime assertion-pass gate

**Files:**
- Create: `crates/vox-integration-tests/tests/golden_dual_runtime.rs`

**Approach.** For every golden with `@test` (and later, every golden), run its `@test` fns under **both** the interpreter (`Interpreter`, as `golden_vox_test_runner.rs` does) **and** the native-codegen/script path (compile to Rust and execute, or invoke the prebuilt `vox run --mode script` via `assert_cmd` per file). Require assertion-pass on **both**. This is the gate that *empirically* surfaces V1/V7/V8 (dec/http/time/regex tier-divergences) as failures the moment B-track has not yet fixed them — so it will initially fail on those files; gate them behind an explicit allow-list `KNOWN_SCRIPT_ONLY` that B-track shrinks to empty.

- [ ] Step 1 — write the harness with a `KNOWN_SCRIPT_ONLY: &[&str]` allow-list seeded with `decimal_math.vox`, `regex_stdlib.vox`, `std_http_wrappers.vox` (the verified-divergent files) and a comment linking each to its B-track task.
- [ ] Step 2 — run; expect PASS (divergent files are allow-listed).
- [ ] Step 3 — as each B-task lands, delete its entry from `KNOWN_SCRIPT_ONLY` (the B-task's own test is "file removed from allow-list and gate still green").
- [ ] Verify: `cargo test -p vox-integration-tests --test golden_dual_runtime`.
- [ ] Commit `test(golden): dual-runtime (interp+script) assertion gate`.

**Done-when:** gate is green; `KNOWN_SCRIPT_ONLY` shrinks to empty by end of Track B.

#### Task A3: Tautological-test lint (compact)

- **Files:** `crates/vox-code-audit/src/detectors/tautological_test.rs` (new) + register in `detectors/mod.rs`.
- **Approach:** flag `@test` bodies whose only assertion is (a) `assert(len(x) > 0)` where `x` is the direct return of an identity fn, or (b) `assert(<literal> is <same-shape literal>)` / `assert(true)`. Pattern-match on HIR like sibling detectors (e.g. `no_test_for_pub_fn.rs`).
- **Test:** unit test with a tautological `@test` fixture string → 1 finding; a real value-equality `@test` → 0 findings.
- **Verify:** `cargo test -p vox-code-audit tautological`.
- **Done-when:** detector emits `skeleton/tautological-test` (Warning) and is referenced by the Track-E fixes (E2).

### Track B — Verified compiler tier-divergences (P1, "make it real")

#### Task B1: Interpreter `VoxValue::Decimal`

**Files:** `crates/vox-compiler/src/eval/value.rs` (add `Decimal(rust_decimal::Decimal)` variant + display/eq), `eval/expr.rs:79-85` (parse `DecimalLit` via `Decimal::from_str_exact`), `eval/builtins.rs` (decimal arithmetic + `dec`↔`str`), `eval/expr.rs` binary-op arms (Decimal+Decimal, etc.).

- [ ] **Step 1 — failing test.** In `crates/vox-compiler` interpreter tests, eval `0.1dec + 0.2dec is 0.3dec` and assert it returns `true` (currently false under f64).
- [ ] **Step 2 — run, confirm FAIL.**
- [ ] **Step 3 — impl** the `Decimal` variant + exact parse + the four arith ops + `is`/`isnt` equality + display. `vox-compiler` already transitively reaches `rust_decimal` via actor-runtime builtins; add the direct dep if needed.
- [ ] **Step 4 — confirm PASS;** also run `cargo test -p vox-integration-tests --test golden_dual_runtime` and **remove `decimal_math.vox` from `KNOWN_SCRIPT_ONLY`** (it must now pass under `--mode interp`).
- [ ] **Step 5 — commit** `feat(eval): exact decimal arithmetic via rust_decimal`.

**Done-when:** `decimal_math.vox` asserts pass under both modes; dual-runtime allow-list entry removed.

#### Task B2: Interpreter compiled-Regex value (compact)

- **Files:** `eval/value.rs` (`Regex`/`Match` variants), `eval/builtins.rs:1757-1772` (`regex.compile` returns a real compiled value), method dispatch for `re.find/matches/find_all`, `m.group`.
- **Approach:** mirror codegen's `VoxRegex/VoxMatch` (`crates/vox-actor-runtime/src/builtins/mod.rs:151-194`) using the `regex` crate already in the interpreter.
- **Test:** eval `regex_stdlib.vox`'s `re.find(t){Some(m)=>m.group(1)}` idiom under interp → expected capture.
- **Verify:** dual-runtime gate; remove `regex_stdlib.vox` from `KNOWN_SCRIPT_ONLY`.
- **Done-when:** compiled-regex idiom runs identically interp vs script.

#### Tasks B3/B4: Interpreter `std.http` / `std.time` dispatch arms (compact)

- **Files:** `eval/builtins.rs` `call_builtin_method` — add `Some("http") => …` and `Some("time") => …` arms alongside the existing `fs/process/json/regex` arms.
- **Approach:** for `http.get_text/post_json`, reuse the actor-runtime blocking-reqwest worker pattern (`builtins/mod.rs:937-1094`) or a direct blocking `reqwest` call behind the same `wasm32` guard; for `time.now_ms`, return current epoch ms (note: determinism-lint already flags it, so goldens must `@test`-guard).
- **Test:** interp eval of `std.time.now_ms()` returns an int > 0; `std.http.get_text` against a local mock (or assert it no longer raises `UndefinedVariable`/unreachable).
- **Verify:** dual-runtime gate; remove `std_http_wrappers.vox` from `KNOWN_SCRIPT_ONLY`.
- **Done-when:** `std.http`/`std.time` reachable under `--mode interp`.

#### Task B5: Unblock db `.order_by`/`.limit` in typecheck (compact, high value)

- **Files:** `typeck/checker/expr.rs:381` (delete the `not supported yet` guard, or gate it behind the existing plan-validation path), `typeck/builtins.rs:1546-1593` (add `filter/where/select/order_by/limit` to the `Ty::Table` method advertisement so diagnostics are correct), `examples/golden/db_native_ir.vox` (remove `// vox:skip`).
- **Approach:** the HIR-lowering (`hir/lower/expr_db.rs apply_order_by/apply_limit`) and Rust codegen (`tables/codegen.rs all_order_limit/filter_where_order_limit`, `method_emit.rs:293-324`) already exist — this is purely removing a stale typecheck wall and reconciling the method table.
- **Test:** typecheck a snippet `db.T.where({a:{eq:1}}).order_by(...).limit(10)` → no diagnostics; `db_native_ir.vox` parses+lowers (already covered by `golden_vox_examples_test`) and now passes `vox check` without skip.
- **Verify:** `cargo test -p vox-compiler --test golden_vox_examples_test`; `vox check examples/golden/db_native_ir.vox`.
- **Done-when:** ordered/limited queries reach codegen from source; `db_native_ir.vox` un-skipped; E3/E4/F2 unblocked.

#### Tasks B6–B11 (compact)

- **B6 TS `dec`:** `codegen_ts/.../:827` emit a decimal type (e.g. a `Decimal` wrapper or `big.js`-style) instead of a bare string; **test:** TS snapshot for `decimal_math.vox` shows decimal ops, `tsc` typechecks. Dep B1.
- **B7 for-iterate:** `eval/expr.rs:457-487` accept `Str` (chars), `Object`/map (entries or keys per spec), `Tuple`; **test:** `for c in "abc"` yields `['a','b','c']`. Decide & document map iteration shape (keys vs (k,v)) — match the existing `.items()` tuple shape.
- **B8 arity/defaults:** `eval/expr.rs:295-322` add arity check matching `Interpreter::call` (mod.rs:435); apply `HirParam.default` when an arg is omitted (`expr.rs:265-275`); **test:** too-few-args call errors `ArityMismatch`; omitted-arg-with-default uses the default.
- **B9 index-assign:** `eval/stmt.rs:219-251` handle non-`Ident` l-values (`obj.field[i]=v`); **test:** nested index-assign mutates (currently silent no-op).
- **B10 path dedup:** remove legacy `path.basename/dirname` typeck+codegen entries (`builtin_registry.rs:513-515,762-769`) OR register them as deprecation-marked aliases; **test:** only the canonical `file_name/parent/stem` resolve (or aliases warn).
- **B11 split_lines:** add `s.split_lines() -> list[str]` to interp+codegen builtins, or document `split("\n")` as canonical in `str_utils.vox`; **test:** `"a\nb".split_lines()` → `["a","b"]`.

### Track C — Type-system depth (P2, spec-gated)

#### Task C1: `Result[T,E]` representation **[Gate: write a design note first]**

**Files:** `typeck/ty.rs` (`Ty::Result(Box<Ty>, Box<Ty>)`), `typeck/registration.rs:256-258` (keep both args), `typeck/checker/expr_ops.rs:233-235` (bind `Err`/`Error` payload to real `E`), unify/resolve/instantiate/occurs arms, `?`-operator success/err threading.

- [ ] **Step 0 — spec.** Write `docs/superpowers/specs/2026-06-02-result-two-param-typeck.md` (this supersedes the older `2026-05-18-result-two-param-design.md` — read it first; it may already specify this). Define: the two-slot type, migration of all `Ty::Result(Box<Ty>)` constructors, `?` semantics, and the corpus rule "do NOT add `Result[T,E]` training data until this lands."
- [ ] **Step 1 — failing test.** Typecheck `fn f() to Result[int, MyErr] { ... } match f() { Err(e) => e.field }` and assert `e` is typed `MyErr`, not `str`.
- [ ] Steps 2–5 — TDD impl per the spec; thread `E` through every `Ty::Result` site (compiler-wide grep for `Ty::Result`).
- [ ] **Verify:** new typeck tests + full `cargo test -p vox-compiler`.
- **Done-when:** named error types check correctly; then (only then) F7/`nested_types.vox` may add `Result[T,E]` coverage.

#### Task C2: Option/Result match exhaustiveness (compact)

- **Files:** `typeck/checker/match_exhaust.rs:19-31` — add `Ty::Option` (Some/None) and `Ty::Result` (Ok/Err+Error) arms reusing the missing-case + divergent-wildcard machinery.
- **Test:** a `match` on an `Option` that omits `None` → exhaustiveness diagnostic; full match → none. Add a negative golden/`#[test]`.
- **Verify:** `cargo test -p vox-compiler match_exhaust`.

#### Tasks C3/C4/C5 (compact)

- **C3 Id[T] [Gate]:** decide newtype-vs-alias in a one-paragraph note; if newtype, carry `T` in `registration.rs:275` and reject cross-entity assignment; add a non-skipped golden. If alias, document it and downgrade.
- **C4 generics:** add a golden `fn first[T](xs: list[T]) to Option[T]` called at two instantiations; **test:** typechecks + runs both instantiations. Confirms the existing `GenericParam` instantiation path end-to-end.
- **C5 dead infer.rs:** confirm zero callers (grep), then delete `typeck/infer.rs` and its `mod.rs:45` declaration, or move behind `#[cfg(test)]` if any test uses it. **Verify before deleting** (per repo audit-retirement discipline): `cargo build -p vox-compiler` after removal.

### Track D — Durable/concurrency codegen de-no-op (P2, spec-gated)

> **[Gate]** Write `docs/superpowers/specs/2026-06-02-durable-codegen-de-noop.md` first, citing `durability-runtime-audit-2026.md` and ADR-019/021/041. The interpreter path is already real (VoxDbTracker replay tested); the work is making the *emitted binary* match it for the linear subset.

- **D1** thread a real `VoxDbTracker` into `emit_workflow_body` (`durability_lower.rs:40-43`) replacing `DefaultTracker`; **test:** an emitted workflow journals a step (integration test asserting journal rows). Train only the linear subset (planner bails on `match` at `plan.rs:437` — keep that boundary).
- **D2** wire `journal/execute.rs:15-46` to a runtime tracker (not `cfg(test)`); **test:** emitted activity records + retries.
- **D3** emit a real `match`-on-`Envelope` dispatch table (`durability_lower.rs:111-121`) instead of binding to `_`; **test:** emitted actor delivers a message to its handler.
- **D4/D5/D6/D7** corpus + lint (parallelizable, file-disjoint): reclassify `saga_compensation.vox` (no saga runtime exists — make it honest manual compensation); rewrite `scheduled_tick.vox` to real `@scheduled` (drop the false E028 claim — E028 is retired per AGENTS.md); broaden the 5-path determinism blocklist + add a negative golden; add `VoxDbTracker` patch/dedup hooks (low).

### Track E — Golden corpus repair (P1, highly parallel)

> All Track-E tasks are **file-disjoint** → run as a worktree-isolated `Workflow` fan-out. Each task's done-criterion is: file passes `golden_vox_examples_test` (parse/lower) **and** `golden_dual_runtime` (A2) assertion-pass under both modes, **and** has no `delog_issues`. The ~22 untested files for E1 and the ~13 no-frontmatter / ~38 missing-`difficulty` files for E7 are enumerated by the scout's `golden_files` report (committed alongside this plan as the findings appendix — see §5).

- **E1** add a *meaningful, value-asserting* `@test` to each of the ~22 untested goldens (NOT `assert(len>0)` — the A3 lint will reject those). One file per subagent.
- **E2** rewrite the tautological tests: the 6 `ai_fixtures/*` (identity-return+`len>0`), `test_suite.vox` (asserts literals equal themselves), `format_conversion.vox` (Ok-vs-Error, not value equality), `deferred_fill_hole.vox` (asserts a literal) → assert actual computed values.
- **E3** `iot_telemetry.vox`: replace `return 42.0` with a real average over `logs` (after B5, use `db.DeviceLog.where(...)` or compute over `all()`); delete the false "filter() not yet implemented" comment; remove unused `id` params or use them.
- **E4** `multi_tenancy.vox`: after B5, scope with `db.T.where({tenant_id:{eq:t}})`; until then, add a header comment "illustrative only — does not enforce isolation" so MENS doesn't learn cross-tenant leakage as correct.
- **E5/E6** fix `inventory_rosetta_platform.vox` undefined `has_capability` (define it or remove the call); fix `ref_actors.vox` `str+int` type error + delete stale `TASK-2.6` scaffolding.
- **E7** add `difficulty:` (beginner/intermediate/advanced) to the ~38 goldens missing it; add a full STYLE.md frontmatter block to the ~13 with none (`decimal_math, json_as_typed, reactive_counter, ref_actors/syntax/types/agents/orchestrator, mobile_camera, mobile_test, std_http_wrappers, mesh/noop`).
- **E8** remove `// vox:skip` from `db_native_ir.vox` (after B5) and re-check `index_showcase.vox` (remove if stale).
- **E9** fix `constructs:` frontmatter listing retired `@endpoint` in `auth_patterns.vox`/`background_jobs.vox` (bodies already use `@mutation`).
- **E10** de-log: remove the `scratch_extract.vox` debug-write referenced from `expl-rosetta-inventory.md`; sweep every golden's `delog_issues` from the findings appendix and strip stray debug logging/commented scaffolding.
- **E11** `agent_pipeline.vox`: either add the promised `actor` + `on` handler or rewrite the description/`@training_prompt` to match the plain-fn reality; remove the dead `TaskMessage` ADT or use it.
- **E12** `web_routing_fullstack.vox`: attach `not_found:`/`error:`/`pending:` in the `routes{}` block so the declared `NotFoundPage`/`ErrorPage`/`RoutePending` are actually wired.
- **E13** `http_error_mapping.vox`: upgrade to a real `@query`/`@mutation` returning `Result` with distinct error codes mapped to statuses + a client-consumption snippet (exercises the §6 wire-error envelope).

### Track F — New goldens for shipped-but-untrained features (P2, highly parallel)

> File-disjoint; worktree-isolated `Workflow` fan-out. Every new golden MUST: have full frontmatter (STYLE.md), a meaningful `@test`, pass A2 under both modes, and (where it prints) an A1 `// EXPECT:` fixture.

- **F1 imperative core (5 files):** `while_search.vox` (accumulator/search loop), `string_interpolation.vox` (numbers, bools, nested method calls in `${}`), `closures_hof.vox` (map+filter+fold chain, closure capturing an outer var, returning a closure), `range_loops.vox` (`for i in range(n)` and `range(s,e)`), `mutate_and_index.vox` (`let mut` counter + list/dict index-assignment). These are the **thinnest-covered core constructs** for MENS.
- **F2 db query plan (4 files, dep B5):** `db_where_filter.vox` (`.where({status:{eq:"active"}})`, `.filter`), `db_order_limit.vox` (`.order_by/.limit`), `async_view.vox` (`component` matching an `Async[T]` from a `@query` loader across all four arms fetching/empty/error/ok), `paginated_query.vox` (real `Paginated[T]` cursor `@query`).
- **F3 MENS decorators [Gate]:** first decide per-decorator: **wire `@inference` into codegen** via the model-agnostic `vox_actor_runtime::llm` facade (mirror `@llm` model-pin) **or tombstone it** with an honest deprecation marker. Same call for `@training_step`/`@distributed_train` (connect to `vox-distributed-training`/`vox-inference` or down-scope to annotations). Then: add `inference/training_step/distributed_train` rows to `contracts/mens/golden_decl_expectations.yaml` (so CI stops being blind to their absence) and add one golden each + a bare `@llm` golden (pinned + unpinned model, structured-output return). **This is the language's own AI surface and is the single thinnest-covered area.**
- **F4** `regex_free.vox` — the cross-tier-safe free-function form (`regex.replace/is_match/captures`), which works in **both** runtimes today (the compiled-object golden does not, pre-B2).
- **F5 stdlib (4 files, dep B4):** `process_run.vox` (guard-with-is-null + `run_capture_json`), `path_ops.vox` (`extension/parent/stem/file_name`), `env_vars.vox`, `time_uuid.vox` (`@test`-guarded for nondeterminism).
- **F6 web:** list-render variants into the *main* corpus (nested lists, list+conditional, derived item state, non-trivial key); `safe_area` edges (bottom/all/none) folded into the mobile goldens; a reactive `on mount:` loading from a `@query` (async-effect await).
- **F7 types (dep C2):** guard-clause `match`, tuple destructuring in `let`/`match`, map iteration via `.items()`, a 3+-variant enum with multi-field constructors + destructuring `match`, an inferred-(unannotated)-return-type fn.
- **F8** a `@server`-only golden (server-only, no client emit); an `@index`/`@search_index`-on-`@table` golden (index DDL + schema-validator emit).

### Track G — HumanEval suite integrity (P0/P1)

- **G1 (P0)** `humaneval.rs:117` — after `vox check`, also `vox run` the `tests.vox` and require assertion-pass (depends A4 so failures are detectable); **rename** `overall_pass_rate`/`median_pass_rate` → `validity_rate` until behavioral pass@k lands. **Test:** the gate now FAILS for a reference whose `tests.vox` contains a false assert.
- **G2** fix `regen_held_out.rs` compile break: restore `build_held_out_manifest` in `vox-audit/src/subcommands/humaneval.rs` (rebuild it to read the live `manifest.v1.yaml`) so `cargo check -p vox-audit --test regen_held_out` passes (currently E0425).
- **G3** reconcile the four disagreeing held-out counts (loose specs=10, manifest inline=31, `held_out_current`=44, target=30) to a single SSOT (manifest `training_eligible` flags); regenerate `held-out.v1.json` via the fixed G2 regenerator using **sha256** (manifest's hash, not the stale blake3).
- **G4 [Gate]** the 164 orphan seed dirs (inner-`spec.toml`, `@test/is`-style, ~492 files): **do NOT blind-delete.** Write a `.vox` (or Rust test) that asserts each candidate dir's slug is absent from `manifest.v1.yaml`; review the list; only then remove. One orphan (`006-parse-int`) is a stub (`return Ok(42)`) — it must be deleted with the orphans, never lifted live.
- **G5** add 8–12 *live* problems declaring `to Option[T]`/`to Result[T]` signatures with `?`/match propagation (the live corpus has **0/164** such signatures) — real references, not the orphan stubs.
- **G6** either implement the README-claimed vox-corpus contamination guard (verify held-out problems are never MENS-ingested) or strike the unsubstantiated claim from the README.

### Track H — Doctest discipline (P3)

- **H1** `crates/vox-doc-pipeline/src/doctest.rs:32` — require a one-line reason on every `// vox:skip` (190/209 are bare); **test:** a bare-skip block fails the lint, a `// vox:skip: <reason>` passes.
- **H2** de-skip the trivially-compilable basic-syntax blocks in `ref-syntax.md`/`ref-type-system.md` (make them real doctests).
- **H3** fix the wrong match-arm thin-arrow syntax taught in how-to docs (hidden behind bare skips).
- **H4** back `how-to-database.md`/error-handling docs with inline snippets matching `crud_api.vox`/`error_propagation.vox`.

### Track I — Remaining sanity (P3)

- **I1** clear the two codegen warnings surfaced by the live build: unreachable match arm `codegen_ts/rn/component.rs:1417`, unused `mut` `codegen_ts/vox_client.rs:798`.
- **I2** sweep the non-golden `.vox` trees (`scripts/**`, `apps/*/src/main.vox`, `examples/{sandboxes,cli-tour,compile-suite}`, `tests/**`) for compile+run health via a `Workflow` fan-out; file findings as follow-up tasks (do not auto-fix scripts that mutate the repo without review).
- **I3** add a test enforcing the SSOT's "aspirational files must parse without panicking" contract (currently unenforced — `examples_ssot_test.rs` only checks layout).

---

## 4. Workflow Execution Recipes for Opus 4.8 (parallel-at-speed)

Run these with the `Workflow` tool. They encode the §1 parallelism rules. **Track A first (serial), then B/C/D as three concurrent worktrees, with E/F/G/H fanned out.**

### Recipe 1 — Corpus repair fan-out (Tracks E + F, after A2 + B5)

```text
meta: { name: 'vox-corpus-repair', phases: [{title:'Fix'},{title:'Verify'}] }

// Each item = one golden file task (E1..E13, F1..F8 sub-files). Worktree isolation
// because every subagent writes a different .vox file concurrently.
const TASKS = [ /* one entry per file from the master index E/F rows */ ]

await pipeline(TASKS,
  // Stage 1: implement the fix/new-golden in an isolated worktree (no write conflicts)
  t => agent(`Implement golden task ${t.id}: ${t.instruction}. Follow examples/STYLE.md
              (full frontmatter incl. difficulty), add a MEANINGFUL value-asserting @test
              (no identity-return+len>0 — the tautological-test lint rejects those), and a
              // EXPECT: stdout fixture if it prints. Do NOT touch any other file.`,
        { isolation: 'worktree', phase: 'Fix', schema: FILE_RESULT, label: `fix:${t.id}` }),
  // Stage 2: verify under both runtimes + parse/lower (no barrier — verifies as each fix lands)
  (fix, t) => agent(`Verify ${t.id}: run \`vox check\`, \`vox run --mode interp\` and
              \`--mode script\` on ${t.file}; confirm @test asserts pass in BOTH modes and
              any // EXPECT matches. Report pass/fail + evidence.`,
        { phase: 'Verify', schema: VERIFY_RESULT, label: `verify:${t.id}` }))
```

> Worktree isolation is the key: ~30 file-disjoint golden tasks run ~16-wide with zero merge conflicts; a reviewer merges the green worktrees. `log()` any task whose verify stage fails so nothing is silently dropped.

### Recipe 2 — Compiler tracks B/C/D as three concurrent worktrees

```text
meta: { name: 'vox-compiler-real', phases: [{title:'B-eval'},{title:'C-typeck'},{title:'D-codegen'}] }

// Tasks WITHIN a track are sequential (they share eval/builtins.rs etc.);
// the three tracks run concurrently because they touch disjoint modules.
await parallel([
  () => runSerialTrack('B', [B1,B2,B3,B4,B5,B6,B7,B8,B9,B10,B11], 'eval/ + typeck db-unblock'),
  () => runSerialTrack('C', [C2,C4,C5],                          'typeck/'),   // C1/C3 gated → spec first
  () => runSerialTrack('D', [D4,D5,D6],                          'corpus/lint'), // D1-D3/D7 gated
])
// runSerialTrack = for-loop awaiting one agent() per task, each doing the TDD micro-loop,
// each ending by re-running the relevant cargo test + shrinking A2's KNOWN_SCRIPT_ONLY list.
```

### Recipe 3 — HumanEval + doctest sweep (Tracks G + H + I, file-disjoint)

```text
meta: { name: 'vox-eval-and-docs', phases: [{title:'Run'}] }
await parallel([G2,G3,G5,G6,H1,H2,H3,H4,I1,I3].map(t => () =>
  agent(`Execute ${t.id}: ${t.instruction}`, { isolation: 'worktree', label: t.id })))
// G1/G4 run separately: G1 depends on A4; G4 is gated (investigate-then-remove, human-reviewed).
```

**Cost note:** these recipes spawn dozens of Opus agents. Run them only after the per-track specs (C1/D1/F3/G4 gates) are written and the user has greenlit execution.

---

## 5. Findings appendix (commit alongside this plan)

The scout's full structured output (per-file `golden_files` verdicts, the 6 domain `feature_reports`, the HumanEval `tree_report`) is the authoritative enumeration behind the §2 index. Before executing, persist it as `docs/src/architecture/vox-golden-corpus-and-compiler-reality-findings-2026.md` (research doc — needs YAML frontmatter per AGENTS.md) so E1/E7/E10's "the ~22 / ~13 / per-file delog list" are concretely named. Set valid frontmatter on the new page (`title`, `description`, `category`, `status`). Starlight lists it. Do **not** create or edit `docs/src/architecture/research-index.md` (retired 2026-09). (Deferred from this plan-only pass; create it as task **A0** when execution begins.)

---

## 6. Verification gates / Definition of Done (whole plan)

The plan is "done" when **all** hold:

1. `cargo test -p vox-compiler --test golden_vox_examples_test` — green (unchanged invariant).
2. `cargo test -p vox-integration-tests --test golden_vox_test_runner` — green, and `@test` count has risen from 28 toward ≥1 meaningful test in **every** golden (Track E1).
3. **New** `golden_dual_runtime` (A2) — green with `KNOWN_SCRIPT_ONLY` **empty** (every golden runs identically under `--mode interp` and `--mode script`).
4. **New** `golden_output_fixtures` (A1) — green; every printing golden has a pinned `// EXPECT:`.
5. `cargo check -p vox-audit --test regen_held_out` — compiles (G2); `vox audit humaneval` executes `tests.vox` and reports `validity_rate` (G1); held-out counts reconciled (G3).
6. `vox-code-audit` reports **0** `skeleton/tautological-test` and **0** stub-in-golden findings (A3, E2, E3).
7. `contracts/mens/golden_decl_expectations.yaml` lists `inference/training_step/distributed_train`, each backed by ≥1 golden (F3).
8. `cargo run -q -p vox-arch-check` and `vox ci pre-push --full` — green.
9. No `// vox:skip` without a reason (H1); the verified compiler bugs V1–V10 each have a regression `#[test]`.

---

## 7. Self-review (writing-plans checklist)

- **Spec coverage:** every scout finding maps to a task — output-infra (A1–A4), each verified bug V1–V10 (B1/B5/B7/B2/B3-B4/G1/G2/E3/C1/D1-D3), every corpus verdict bucket (E1 needs-test, E2 tautology, E3-E6/E11-E13 suspected-bug, E7 frontmatter, E8-E9 stale-skip/deprecated-frontmatter, E10 de-log), every underrepresented-but-shipped feature (F1 imperative core, F2 db plan, F3 MENS decorators, F4 regex, F5 stdlib, F6 web, F7 types, F8 server/index), HumanEval integrity (G1–G6), doctest (H1–H4), remaining trees (I1–I3). The user's four asks — *compile + sensible output / input+output for all*, *de-log*, *core functional + fix bugs*, *gaps by training importance + make compiler real* — map to Tracks A, E10, B/C/D, and F respectively.
- **Placeholder scan:** no "TBD/handle edge cases/similar to Task N" — compact tasks carry real file:line targets, a concrete approach, a named test, and an exact verify command. Design-heavy tasks (C1, D1–D3, F3, G4) are explicitly **[Gate]**-marked to produce a spec first rather than hand-wave an impl.
- **Type/name consistency:** harness names (`golden_dual_runtime`, `golden_output_fixtures`, `KNOWN_SCRIPT_ONLY`), the `skeleton/tautological-test` detector id, and the `validity_rate` rename are used consistently across §3/§4/§6.
- **Known gap (intentional):** the per-file enumerations for E1/E7/E10 live in the findings appendix (§5, task A0) rather than being inlined here, to keep the plan navigable; A0 makes that concrete before E-track executes.
