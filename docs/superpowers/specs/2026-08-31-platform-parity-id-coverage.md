---
title: "Platform parity — original-ID coverage (no handwaving)"
description: "Every L/R/H/G/M ID from the 2026-08-31 limitations canvas mapped to a feasible v1 with files, tests, and named residuals."
category: "architecture"
status: "roadmap"
training_eligible: false
---

# Platform parity — original-ID coverage

**Original inventory (normative gap/fix text):** Cursor canvas `vox-platform-limitations.canvas.tsx` from the 2026-08-31 diagnosis (`ITEMS[]`, 82 IDs). Every row below quotes that `gap` / `fix`. Design types: [2026-08-31-platform-parity-design.md](2026-08-31-platform-parity-design.md). Sequencer: [2026-08-31-platform-parity-program.md](../plans/2026-08-31-platform-parity-program.md).

**Rule:** An ID is **addressed** only if this table names files + a failing-test name + a commit. “Optional”, “skip”, “or document”, and “honesty copy only” are forbidden unless the **original fix** itself allowed a binary choice (R04 drop-vs-ship, G07 editor-vs-harness, R05/R06 marketing). Then the chosen arm is listed and the other arm is a **named residual**.

**Shipped engines:** do not rebuild; the v1 column is the **remaining original fix**.

Legend: **T0A** free-tier plan · **T0B** chat unification · **T1–T6** 2026-08-31 child plans.

## Language (L01–L20)

| ID | P | Original gap (canvas) | Original fix (canvas) | Feasible v1 (this program) | Plan | Residual |
|---|---|---|---|---|---|---|
| L01 | P0 | No closures; transforms need named fns | RFC `\|x\|` + `.map/.filter/.and_then` | Prefix `\|` in **expr** position → existing `HirExpr::Lambda`. Golden `xs.map(\|x\| x * 2)`. Keep `fn(x)` and ADT `\| Foo \| Bar`. Tests: `parse_identity_closure`, `list_hof` bar form, `adt_bar_and_pipe_op_still_parse` | T2 T1–T3 | FnMut / recursive closures (RFC later) |
| L02 | P0 | `@ai` json_schema name-only | Full JSON Schema; reject uns serializable | Unknown type → typeck/codegen **error**. Registered typedef keeps `schema_for` body. Test: extend `ai_structured_output_emit.rs` | T2 T4 | none |
| L03 | P0 | Prompt is `Implement the function: {name}` | HirFn doc + `@prompt` as user message | `HirFn.doc_comment` from `///`; emit uses it; delete sole name prompt. Test: emit contains doc text, not only identifier | T2 T4 | none |
| L04 | P0 | Budgets parsed then dropped | Codegen/runtime read ceilings or typeck error | Bind `cost_ceiling_usd_per_call` / `max_iterations` or `budget-annotation-dropped`. Test: `budget_appears_in_emit` using golden `@ai(...)` syntax | T2 T5 | `@subagent(budget_usd)` if not on same Hir fields — same task if HIR has it |
| L05 | P0 | `vox test` = cargo stdio + exit 1 | Per-test JSON (name, file, status, duration, message) | Nest `tests[]` in **one** envelope (`BuildLaneEnvelope` or replace). Test: failing `@test` → `tests[0].status==failed` | T2 T7 | none |
| L06 | P0 | Failures point at generated Rust | Sourcemap; remap before print | `target/generated/.vox-sourcemap.json`; `vox test` remaps. Test: `source.file` ends `.vox` | T2 T7 | DWARF (overkill) |
| L07 | P1 | `@ai` effect-free | `Effect::Llm`; `@pure`+`@ai` error | `EffectAnnotation::Llm` + `HirCapability::Llm`. Test: `pure_ai_is_error` | T2 T5 | none |
| L08 | P1 | `--coverage` / snapshots / forall no-ops | Implement or delete | **Delete** clap flags unless llvm-cov is wired in the same commit. Test: `--help` omits deleted flags | T2 T7 | llvm-cov report (later) |
| L09 | P1 | stub-check not in default binary | Default-on; mutants behind feature | `Cargo.toml` `default` includes `stub-check`. Test: default `vox check --for-llm` flags `todo!()` | T2 T8a | none |
| L10 | P1 | `@ai` untestable offline | `VOX_LLM_REPLAY` cassette on facade | Register env; `llm_chat` reads JSONL. Test: tempfile cassette in `vox-actor-runtime` | T2 T8e | none |
| L11 | P1 | Model pins free-form | Typeck vs registry; did-you-mean | `contracts/models/known-slugs.v1.json` `include_str!`. Test: `gpt-4o-mimi` errors | T2 T6 | live registry (no crate edge) |
| L12 | P1 | constrained-gen not in a sampler | Hook one MENS Candle sampler; CI real mask | **Non-ignored** `mask_next` test in `vox-constrained-gen` plus one `vox-mens`/`vox-populi` call site **or** `#[test] fn sampler_calls_mask_next` that fails until wired. No `#[ignore]` | T2 T9a | Full production sampler if plugins exclude GPU |
| L13 | P1 | Single-file check/test/fmt | `--since HEAD` / dir walk | `vox check <dir>` walks `*.vox` (cap 256); `--since` uses `git diff --name-only`. Test: temp dir two files, one error | T2 T9b | `fmt --check` repo-wide already `scripts/fmt.vox` — add `--since` to check only |
| L14 | P1 | All failures exit 1 | 2 compile / 3 test / 4 infra | `std::process::exit` in `test.rs`. Test: spawn fixture | T2 T7 | document in `cli.md` same commit |
| L15 | P1 | `vox-lsp` not in install | Dist profile + `vox doctor --fix-lsp` | Add binary to `profiles.v1.yaml`; doctor installs/points PATH. Test: profile lists `vox-lsp` | T2 T8b | none |
| L16 | P1 | `vox repair` OpenRouter bypass | `llm_chat` facade | Delete hostname POST. Test: no OpenRouter URL in `repair.rs`; works with replay | T2 T8c | none |
| L17 | P1 | Dead diag URLs; no `vox explain` | `/diag/{code}` + `vox explain` | Fix host to live docs; clap `explain <code>`. Test: unknown → exit 1 | T2 T8d | generating every Astro page (can stub 404→registry text) |
| L18 | P2 | native and wasm both `.wasm`; targets roadmap | Native real binaries; wasm opt-in | **Prove** native artifact name (`vox-script.exe` / no `.wasm`). Comment-only is **not** enough — test `compile_native_artifact_name`. `--target server\|client\|fullstack` remain roadmap: doctor/help **must not advertise them as shipped** | T2 T9c | full target matrix |
| L19 | P2 | Durable HTTP boot deferred | Route-emission so `@durable` services boot HTTP | **Loud** diagnostic `durable-http-boot-unimplemented` when that path compiles. Full refactor stays `http-runtime-extraction-2026.md` | T2 T9d | HTTP boot implementation |
| L20 | P2 | stdlib holes block `scripts/` | Every `scripts/**/*.vox` `vox check`; missing ns = error | `vox ci script-check` vs `contracts/ci/scripts-check.allow.v1.txt` **tighten-only**. `scripts/fmt.vox` must pass | T2 T9e | emptying allowlist |

## Runtime (R01–R12)

| ID | P | Original gap | Original fix | Feasible v1 | Plan | Residual |
|---|---|---|---|---|---|---|
| R01 | P0 | Required CI Linux-only | Required Win+mac smoke: compile + interp + wasm run + GUI sidecar | (1) Branch-protection on **existing** 3-OS `cargo check --workspace` PR job. (2) merge_group/schedule: `cargo test -p vox-compiler --lib` **one** hosted OS. (3) merge_group **one** OS: `vox run --interp` on a golden `.vox`. (4) wasm+subprocess doctor. **Not** GUI sidecar on hosted (anti-stacking) | T5 T1 + T1b | GUI sidecar + wasm run on hosted Win/mac |
| R02 | P0 | WASM sold as runs-everything | Capability manifest; doctor mismatch | `contracts/runtime/capabilities.v1.yaml`; doctor error wasm+subprocess/gpu | T5 T2 | none |
| R03 | P1 | macOS warning-only sandbox | Seatbelt + Win job object + CI fail if degraded | `sandbox_status()` → `landlock` / `job-object` / `warning-only`. `VOX_REQUIRE_SANDBOX=1` → doctor **nonzero** on warning-only. Unit test existing Landlock/job-object modules still compile | T5 T4a | macOS Seatbelt implementation |
| R04 | P1 | Mobile emit unfinished | Golden RN app **or drop v1 claim** | **Chosen: drop.** Doctor `mobile-emit-incomplete` on `--target mobile`; docs “not a v1 target”. No fake golden | T5 T4b | RN emit program |
| R05 | P1 | Cloud runs models not agents | LAN-mesh+Fly **or stop marketing cloud runtime** | **Chosen: stop marketing** in packaging/canonical-runtime docs. Product arm: G04 **local daemon** background turn (not rented VM) | T5 T4c + T6 G04 | rented VM / Fly workers |
| R06 | P1 | crates.io disabled | Publish **or document not-a-library** | **Chosen: document.** YAML comment + reference sentence. No flip `publish.enabled` | T5 T4d | actual publish |
| R07 | P1 | CLI skill path skips MCP sandbox | One `SandboxedSkillRunner` | CLI `vox skill run` calls runner; ARS echo stub gone. Test: `assert!(!src.contains("stub echo"))` plus runner hook | T5 T3 | none |
| R08 | P1 | Unsigned Windows installer | Authenticode in release; doctor unsigned | Release job comment + `windows-unsigned` doctor **warn**. Test: doctor code path | T5 T5a | paid cert in CI secrets (admin) |
| R09 | P2 | No `vox build --target triple` | Document linux-x64, win-x64, darwin-arm64; CI artifacts | Packaging SSOT table + `vox build --help` lists triples. Golden **compile** not full nextest | T5 T5b | per-triple golden app artifacts |
| R10 | P2 | CUDA skip silent | Inherit PATH; fail-loud if GPU requested | `VOX_REQUIRE_CUDA=1` doctor exit nonzero without nvcc. Register env. Test: `doctor_require_cuda_fails_without_nvcc` | T5 T5c | GUI inheriting user PATH (VS Code setting already) |
| R11 | P2 | Durable subset not always-loud | Determinism lint default-on; GUI “cannot replay” | Fixture: `workflow` + `time.now()` fails `vox check`. GUI: PlanPanel/workflow card string when lint fires | T5 T5d + T6 T27 | none |
| R12 | P2 | Deploy is text gen | `vox deploy --dry-run` validates; one golden in CI | Flag exists. Test fixture exit 0; missing → 4. CI job **or** `vox ci` wrapping dry-run on `examples/` compose | T5 T5e | live Fly apply |

## Harness (H01–H18)

| ID | P | Original gap | Original fix | Feasible v1 | Plan | Residual |
|---|---|---|---|---|---|---|
| H01 | P0 | Sequential; max 8 | Parallel reads; 8/32/128 by mode; **surface remaining budget in rail** | `join_all` ungated; mode budgets; chat event `iterations_left`. Rail shows `8/32` | T1 T7 + T6 T28 | 80-turn Cursor parity |
| H02 | P0 | 200-char args | Unified diff on mutating approval; DiffView | `PendingApprovalInfo.unified_diff`; Approvals `<pre>` / DiffReview | T1 T2–T4 + T6 T5 | none |
| H03 | P0 | 300s in-memory; restart loses inbox | Persist in vox-db; timeout configurable/none; resume | Timeout on `HitlPolicy` default 86400, `0`=never. Persist args+digest. `reregister_after_restart` **lists** inbox. Waiter is in-process: after restart GUI **polls list** (no fake resume of oneshot). Test: rehydrate lists row | T1 T2 T4 | cross-process oneshot |
| H04 | P0 | Modified uses original args; `/rollback` omits id | Modified-args E2E **or remove**; oplog picker | Keep Modified with args; `vox_oplog` + `.id` → `vox_undo`. Never `{}` | T1 T1 T5 T6 | none |
| H05 | P1 | No user hooks | `.vox/hooks.json` Pre/Post/Stop | Parse + `vox_process_run*`; Pre nonzero blocks. Tests: missing ok; exit 1 blocks | T3 T2 | porting CC hook scripts 1:1 |
| H06 | P1 | No always-injected VOX.md | VOX.md + AGENTS.md + `.vox/rules` every turn + Rules GUI | Extend `load_project_context`; Rules GUI is G05 | T3 T1 + T6 T16 | none |
| H07 | P1 | All tools eager | 10–20 core + search | Cap 20; pin `vox_tool_search` in `TurnContext::pin_names` | T3 T3 | none |
| H08 | P1 | No worktree isolation; flags off | git worktree cwd; GUI shows cwd | Spawn helper under `.vox/worktrees/<id>`; SubAgents shows cwd. Default flag stays false until tests pass; then default-on **in same track** if green | T3 T4 | overlay FS |
| H09 | P1 | Plan not a hard gate | Plan denies writes; **ExitPlanMode** only promotion | `plan_blocks_execution`; MCP tool `vox_exit_plan_mode` sets `accept_edits`. PlanPanel Approve calls it (G10) | T1 T8 T13 + T6 T21 | none |
| H10 | P1 | No checkpoint UX | Drawer: restore, diff vs now, keep-N | CheckpointDrawer + `vox_oplog`; restore via `rollbackLast(id)` | T6 T9 | keep-N GC policy (oplog already bounded?) — add `keep_n` Settings using existing `checkpointMins` |
| H11 | P1 | No cache structuring | Stable prefix; **measure cache-hit %** | Frozen prefix in `build_system_prompt_with_skill`. Footer `cache_hit: bool` / `$` if provider returns it; else honest “cache n/a” | T3 T5 + T6 T14 | Anthropic cache $ if header unsupported |
| H12 | P1 | No thinking UI | Collapsible thinking; route thinking models | Transcript disclosure; selector already intelligence axis — if model slug lists thinking, show block | T6 T10 | force thinking-only catalog |
| H13 | P1 | Secretary auto-dispatch | Propose-only; auto only `accept_all` + high confidence | Propose-only **already shipped**. Remaining: `maybe_autodispatch` returns None unless mode=`accept_all` **and** `ClassifyResult.confidence >= 0.9` (add field default 0 → never auto). Test: “I already fixed it” no dispatch | T3 T5b | none |
| H14 | P1 | No eval gate | Nightly 20 transcripts + 5 repo tasks | `vox harness eval --min-success 0.8` on **one** frozen fixture (5 turns). Nightly workflow optional | T3 T7 | 20-transcript nightly |
| H15 | P2 | ACI default off | Default-on mutating; envelope on pending | Flip `agentos_guardrail_kernel_enabled` default true; attach class to approval `risk_class` | T3 T6 + T1 | none |
| H16 | P2 | No batch resolve | Multi-select; approve reads / deny network | Checkboxes + sequential resolve. Filter helper `idsToApprove`. Cost column if `estimated_cost_usd` set | T1 T12 + T6 T6 | “approve all reads” preset |
| H17 | P2 | Four pollers | One badge: approvals+feedback+hopper+failing | `attentionCount` = approvals + needsYou + blockedTasks; **one poller** (G22) | T6 T7 T11 | none |
| H18 | P2 | No provenance on edits | Git trailers / ACI: model, $, tokens, intent | Oplog fields `model` + cost when write succeeds | T3 T6 | git interpret-trailers |

## ChatGUI (G01–G18)

| ID | P | Original gap | Original fix | Feasible v1 | Plan | Residual |
|---|---|---|---|---|---|---|
| G01 | P0 | DiffReview raw text | File+hunk Accept/Reject/Undo; keep buffer; conflict UI | `parseUnifiedDiff` + `apply_worktree_hunks`; Reject vs HEAD; **keep `diff` string until applied**; apply mismatch → error toast `hunk-did-not-apply` | T1 T9–T10 | 3-way merge editor |
| G02 | P0 | No first-run | Wizard: Ollama → PKCE → free → first turn | Execute T0A as written | T0A | none |
| G03 | P0 | No Ask/Plan/Agent | Composer 3-way → permission_mode + budget + tools | `sessionMode.ts` + `setPermissionMode` + `message.rs` budgets + Plan tool subset | T1 T11 + T6 T2–T3 | none |
| G04 | P1 | No cloud/async agents | Queue session on worker (LAN daemon **or** rented VM) + NeedsYou | **LAN daemon:** checkbox “Run in background” → Tauri `queue_background_turn` posts to existing `vox` daemon / MCP session; poll NeedsYou. **Not** rented VM | T6 T15 | Cursor-like cloud VMs |
| G05 | P1 | No Rules editor | Settings CRUD VOX.md + glob rules + preview tokens | Settings CRUD `.vox/rules/*.md` + preview truncated inject (`load_project_context` char count) | T6 T16 | glob-scoped on/off per file |
| G06 | P1 | Meter unwired; compaction invisible | Live tokens; warn 70%; compact button; show dropped | Meter **already live**. Remaining: warn CSS at 70%; Compact button calling existing `compact_auto` / session compact; last-compact “dropped N tokens” string from engine if field exists else “compacted” | T6 T23 | none |
| G07 | P1 | No Axis editor | Embed editor **or** harness-not-IDE | **Chosen: harness.** Settings sentence + no Monaco. Extension remains ghost-text | T6 T15b | Axis buffer editor |
| G08 | P1 | No durable intent | Intent table; composer fields; PlanPanel writes | Persist `intent_id` on session (reuse plan-node id **or** existing DB row — no new crate). Fields: goal, constraints, budget_usd, acceptance already in IntentPanel → **also** JSON on `chat_turn` `#[serde(default)]`. PlanPanel `insert_plan_node` includes goal | T6 T22 | dedicated `intents` table if plan-nodes insufficient |
| G09 | P1 | Composer is a text box | Intent panel default-on for Plan | When composer=`plan`, IntentPanel **open by default**. Test: `plan` → panel visible | T6 T22b | none |
| G10 | P1 | `vox plan` not GUI; Approve is a chip | Render PlanNode; edit; approve = ExitPlanMode | PlanPanel already lists nodes. **Approve plan** invokes `vox_exit_plan_mode`. Empty state names session nodes SSOT | T6 T21 | CLI `vox plan` JSON import |
| G11 | P1 | No analytical canvas | Table/chart/diff side panel | Chat artifact `kind: table` (2-row vitest). Chart YAGNI | T6 T17 | charts |
| G12 | P1 | Keybinds incomplete | Bindable: send, plan, accept hunk, reject, compact, new session | Add those **action ids** + handlers (G01/G06/G03). Defaults documented. Test: registry includes all six | T6 T12 | Cursor keymap import file |
| G13 | P2 | Missing `window_id` | Stamp on every subagent event | Event JSON `window_id`; rail uses it | T6 T18 | none |
| G14 | P2 | Compute cards hollow | Live backends **or** stop pretending | **Chosen: honesty badge** “CLI (not a live dashboard)” + NAV titles | T6 T13 | live Mens/Populi dashboards |
| G15 | P2 | Browser pane ≠ agent tools | snapshot/click/type as MCP + gate | Allowlist live `vox_browser_*` (screenshot/page_info/text/click). **No** invented snapshot name. Gated like writes | T6 T19 | accessibility snapshot tool |
| G16 | P2 | Cannot see why model won | Footer: slug, axes, $, fallback; Explain | Footer + `explain_model_selection` | T6 T14 | none |
| G17 | P2 | Vision routing not first-class | Image → vision slug or refuse | `vision-model-required` structured error | T6 T20 | none |
| G18 | P2 | No Bugbot | PR open → read-only review subagent | `vox review pr --readonly`; clap test; dry-run no `gh` comments. GUI button optional | T6 T25 | GitHub App |

## Models (M01–M14)

| ID | P | Original gap | Original fix | Feasible v1 | Plan | Residual |
|---|---|---|---|---|---|---|
| M01 | P0 | Zero-key hard-error | PKCE + Ollama detect + free catalog | Execute T0A | T0A | none |
| M02 | P0 | Budgets defined unenforced | Warn 80%, downgrade 100%, GUI remaining $ | Guard **shipped**. Remaining: Chat rail `$ remaining` from budget API; toast already T0A T5. Test: vitest remaining label | T0A T5 + T6 T29 | auto-downgrade to local (if not in T0A) — if missing, T4 T6: on Exceeded, `prefer_local` retry once |
| M03 | P1 | History swamps complexity | Cap prior; 5-task winner moves | Named `HISTORY_PRIOR_MAX=0.15`; fixture-registry `trivial_and_hard_codegen_differ_in_winner` | T4 T1 | none |
| M04 | P1 | No coding benchmark ingest | Nightly SWE/LiveCodeBench | v1: `coding-prior.v1.json` ≤20 slugs; scorer reads `swe_score`. Test: 0.9 beats 0.1 on intelligence-tie | T4 T5a | nightly ingest job |
| M05 | P1 | AGENTS.md locality axis vs 3-axis filter; VRAM advisory | Axis **or** document filter; **VRAM hard admit when prefer_local** | Caption: filter not axis. `prefer_local` + catalog `vram_gb` > detected → **skip candidate**. Test: 70GB model excluded when detect=8 | T4 T3b | nvidia-smi parse failures → admit with warn |
| M06 | P1 | Direct OpenAI/Anthropic roadmap | First-class providers + failover | `LlmConfig::openai` exists; add `anthropic` + `ollama` if missing; secrets + parity CI | T4 T4a | none |
| M07 | P1 | Autonomic off | Nightly refresh; shadow; promote after 5-task | Flag-off noop test; `autonomic_disabled_is_noop`. Enable cron **docs** not CI | T4 T5c | enabled nightly |
| M08 | P1 | Prompt cache unwired | Provider headers; H11 prefix; show cache $ | `cache_headers`; chat cascade uses them; footer honesty | T4 T4b + H11 | none |
| M09 | P1 | GUI must call decide() after scorer boxed | 5-task eval then bind chat | MCP `resolve_mcp_chat_model*` **already** `decide()`. Regression test. Do not double-bind GUI | T4 T2 | none |
| M10 | P2 | Premium alias surprise cost | Never leave cost axis; confirm chip | `cost >= 70` blocks opus alias. GUI chip if alias would fire | T4 T3a | none |
| M11 | P2 | Static :free list | Live `is_free` from price; prune 404 | price 0 ⇒ `is_free`. Test without `:free` suffix | T4 T5b | live OpenRouter crawl |
| M12 | P2 | No best-of-N | n=3 Quality+Plan; pick by tests | `VOX_BEST_OF_N=1` + plan + intelligence≥80; default n=1 | T4 T5d | pick-by-tests (needs L05) |
| M13 | P2 | Local/cloud clutch no SLA | Bind tier; **local if TTFT < 2s else cloud** | Map `local\|mesh\|cloud\|auto`. `auto`: probe local TTFT once/session (timeout 2s) else cloud. Test: mock slow local → cloud slug | T4 T3c | mesh SLA |
| M14 | P2 | No 429 failover | Cascade in chat: next provider → local → NeedsYou | `cascade.rs` on chat egress 429/5xx | T4 T4c | none |

## Extra IDs (audit, not in original 82)

H19 write handler · G19–G27 IA/plumbing — required to **implement** original G/H without React bugs. They do not replace any original ID.

## Forbidden executor phrases

- “skip if already shipped” without naming the **remaining original fix**
- “optional deepen”
- “or document” when the original fix was software (except R04/R05/R06/G07 which are explicit or-gates — pick one arm)
- One commit “remaining GUI IDs”
- `vox_oplog_list`, `HirExpr::Closure`, `vox_browser_snapshot`, `Dispatch::Auto`, `VoxConfig.harness`
