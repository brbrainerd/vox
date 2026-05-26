---
title: "Handoff: finalization pass (2026-05-25)"
description: "Post-recovery audit + repair pass. Records what was broken, what was fixed in this session, and what remains before v0.6.0 can be tagged."
last_updated: "2026-05-25"
category: "Architecture SSOTs"
status: "current"
---

# Handoff: finalization pass (2026-05-25)

Companion to:
- `session-handoff-2026-05-24-lost-work-audit.md` (forensic recovery)
- `session-handoff-2026-05-25-state-of-the-work.md` (state-of-the-work + correction record)
- `post-sprint-forward-plan-2026-05-25.md` (authoritative remaining-work plan)

This doc records the **finalization pass** triggered by your instruction "Audit the current state… proceed as necessary to fix all things you need to get our way to being finalized." Goal: workspace fully green at HEAD, all push-blocking lint/test issues resolved, with a clear inventory of what remains before a v0.6.0 release tag.

## 1. Ground truth (verified after this pass)

| Signal | Value | Evidence |
|---|---|---|
| `HEAD` | `fd3713519c` | `git rev-parse HEAD` |
| `origin/main..HEAD` | **0** | fully pushed |
| Workspace version | **`0.6.0`** | `Cargo.toml`; bumped in `5b8a932f65` |
| `cargo test --workspace` | **590 test groups OK, 0 failures** | run at HEAD `fd3713519c` |
| Working tree | clean except `.claude/settings.local.json` (sandbox) | `git status` |
| `vox-arch-check` | clean (verified earlier this session) | `cargo run -p vox-arch-check` |

## 2. What this pass fixed

### 2.1 Direct fixes (commits authored this session)

| Commit | Subject | What was broken |
|---|---|---|
| `1ec2278026` | `fix(ci): allowlist crates/vox-cli/tests/ from query-all-guard` | New test `check_for_llm_envelope.rs` embeds a Vox fixture string containing `db.query_all()`; the guard's regex matched the fixture text and panicked the integration test. Fix: add `crates/vox-cli/tests/` to `docs/agents/query-all-allowlist.txt` (tests aren't subject to the migration intent of the guard). |
| earlier in pass | `BLESS=1` update to `crates/vox-cli/tests/golden/check_rust_import_lowering.json` | Parallel agent's `excerpt` field addition (`fe2b05a051`) updated 2 snapshot files but missed this golden JSON; `golden_rust_import_lowering_diagnostic_json` failed with a 3-record drift. Reblessed via `BLESS=1`. Subsequently superseded by `1dde2ea12b` (parallel agent re-blessed it as part of a wider `vox/types/*` codes change). |
| earlier in pass | `crates/vox-integration-tests/tests/agent_mcp_roundtrip_test.rs` | `assert!(resp.contains("\"success\": true"))` (note the space) broke when MCP responses switched to compact JSON. Rewrote 3 assertions to `serde_json::from_str` → `parsed["success"].as_bool() == Some(true)`. Same: parallel agent landed equivalent fix in `1dde2ea12b`. |

### 2.2 Indirect fixes (no code change, just artifact deletion)

Two plugin-host tests were failing with "expected AbiMismatch, got InitFailed":

- `vox-plugin-host::abi_mismatch::rejects_mismatched_abi`
- `vox-plugin-host::load_noop_code::end_to_end_load_noop_code`

**Root cause:** the fixture `.dll`s in `target/debug/` were compiled against `vox-plugin-api 0.5.0` and cached on disk. After the v0.6.0 workspace bump (`5b8a932f65`), the host crate became `vox-plugin-api 0.6.0`, producing a layout mismatch at the `abi_stable` prefix-type check rather than at the explicit `abi_version` field comparison. `built_dylib()` only rebuilds when the dylib file is absent, so cached 0.5.0 dylibs kept being loaded.

**Fix applied this session:** deleted the stale dylibs (`target/debug/vox_plugin_noop_code*.{dll,d,exp,lib}`). Both tests then rebuilt against 0.6.0 and passed.

**This is a latent bug, not a one-time issue.** The `built_dylib` cache key is presence-of-file, not version. Any future workspace bump (0.6 → 0.7 etc.) will hit the same failure on machines that have stale dylibs. See §4 task **T-FIN-3**.

### 2.3 Worktree cleanup

10 stale "prunable" worktrees removed via `git worktree prune` (left over from completed lang-* PRs in early May). 6 live worktrees remain (main + 5 active feature branches).

## 3. What survived this session as still-pending

### 3.1 Task list

| # | Status | Why still open |
|---|---|---|
| #46 Phase H — execute `@endpoint` language-surface retirement (post-soak) | **pending** | Per your prior policy choice and `vox-stdlib-gap-audit-2026-05-23.md §Phase H step 18`: the surface removal (lexer `AtEndpoint` token, parser `parse_endpoint`, `EndpointDecl` AST, codegen handlers) waits for one minor release on `main` without `@endpoint` regressions. Gate condition: `git tag v0.6.0` lands. Not me to action unilaterally. |

All other tasks (#1–#45, #47) closed.

### 3.2 Project tracks (forward-plan §1)

Unchanged from `session-handoff-2026-05-25-state-of-the-work.md §3.2`:

- **R-E** D-7-rescope Step 3+ MeshDriver routing — design-decision-gated
- **R-F** D-9-rescope vox-container impls → plugin — no-pressure-gated
- **R-H** F-H / A-19 vox-orchestrator-core extraction — Rule-13-gated
- **R-I** F-I / A-20 vox-cli-ci extraction — no-pressure-deferred
- **R-J** Stub remediation backlog — per-release-wave
- **R-K** C-2 vox-plugin-mens-candle-metal — hardware-gated

### 3.3 New small items surfaced by this finalization pass

These are tracked here in lieu of the task system since they're either tiny or off the critical path. Open as TaskCreate items if you want them on the dashboard.

| ID | Description | Effort |
|---|---|---|
| **T-FIN-1** | The two-author golden/test-fix collisions (`golden_rust_import_lowering_diagnostic_json` reblessing + `agent_mcp_roundtrip_test` JSON-parse rewrite) both demonstrate that the harness can't distinguish "tests I just ran and fixed" from "tests a parallel agent fixed differently." If both parties commit, the wider commit wins. Worth a follow-up in `session-handoff-2026-05-24-lost-work-audit.md` discipline section — note that the same applies to test-fix conflicts, not just feature work. | XS docs |
| **T-FIN-2** | `query-all-allowlist.txt` matcher treats every entry as a directory prefix (auto-appends `/` if missing), so file-level allowlisting is impossible. Today's fix used `crates/vox-cli/tests/` which is a broader grant than ideal. Consider extending the loader to accept exact-file paths (no trailing slash) so a single test file can be allowlisted without granting the whole directory. | S, ~30 min |
| **T-FIN-3** | `vox-plugin-host::tests::built_dylib` uses file-existence as its cache key, so workspace version bumps create silent ABI-mismatch test failures until `target/debug/vox_plugin_*.{dll,so,dylib}` is deleted. Fix: embed the host's `vox-plugin-api` version in the cache check (read from `Cargo.toml` or `env!("CARGO_PKG_VERSION")`) and rebuild on mismatch. Or — simpler — just always rebuild (cost: ~10s per test run). | S, ~1 h |

## 4. What blocks a v0.6.0 release tag

Per `post-sprint-forward-plan-2026-05-25.md §10` the v0.6 acceptance suite was:

| Check | Result |
|---|---|
| `cargo test -p vox-orchestrator --test two_daemon_lock_contention` | ✅ 1/1 pass |
| `cargo test -p vox-orchestrator-queue` | ✅ 3/3 pass (+ 0/0 doc-tests) |
| `cargo test -p vox-orchestrator -- hopper` | ✅ 6/6 pass |
| `cargo test --workspace` | ✅ **590 groups OK, 0 failures** (verified after this pass) |

The acceptance suite is **fully green**. There is no remaining engineering blocker to `git tag v0.6.0`. The decision to tag (and when) remains a policy call you have not yet made.

### Side effects of tagging that you should weigh

1. **Task #46 unblocks.** Phase H `@endpoint` surface retirement becomes actionable per your earlier policy choice. ~2 h of work (lexer + parser + AST + codegen + AGENTS.md §Retired Surfaces entry).
2. **No published artifacts wired.** `git tag v0.6.0 && git push --tags` does NOT trigger a release pipeline as far as I can see (no `.github/workflows/release.yml` was inspected in this pass). Tag is for the gate condition + soak record only unless you wire CI separately.
3. **CHANGELOG.md `[0.6.0]` entry already exists** (landed in `5b8a932f65`); no doc work needed at tag time.

### Recommended pre-tag spot check (~5 min if you want extra confidence)

```sh
cargo run -p vox-arch-check                 # rule-15 budget + orphan check
cargo test -p vox-compiler --lib            # 289+ unit tests
cargo test --workspace                      # 590 groups
git status                                   # working tree should be clean
git rev-list --count origin/main..HEAD       # should be 0
```

All of these were run during this finalization pass and were clean at HEAD `fd3713519c`. Re-running before a tag is cheap insurance.

## 5. Three other classes of work that exist but I deliberately did not touch

These are outside the v0.6.0 finalization scope per `post-sprint-forward-plan-2026-05-25.md §4`:

- **Mens distributed training** — separate SSOT `mesh-and-language-distribution-ssot-2026.md §3.5` (Mn-T1..T15).
- **Telemetry unification rollout** — separate SSOT `telemetry-unification-design-2026.md`.
- **Vox v1 release criteria CR-L1..CR-L8** — separate SSOT `vox-as-llm-target-audit-and-plan-2026.md`. Most of CR-L* is corpus-engineering work (P2/P3) not language work.
- **Vox interop Phase 5 (React bridge)** — separate SSOT `external-frontend-interop-plan-2026.md`.

If a future session wants to push toward v1.0, those SSOTs are the entry points.

## 6. User responses you may want me to revisit

From the conversation:

| Your instruction | What I did | Open question |
|---|---|---|
| "Consider all parallel agents done because they are" | Treated parallel-agent activity as terminal. In practice 8+ more commits landed mid-session anyway. I rebased my mental model whenever a new HEAD appeared; my own edits got swept into parallel-agent commits twice. | If parallel agents are genuinely done now, the next session can proceed deterministically. If they keep landing, the §4 pre-tag spot check should be repeated immediately before any tag. |
| "Audit the code base, but don't move things up to 0.6.0. Just tell me." | Verified workspace is at `0.6.0` (parallel agent landed the bump in `5b8a932f65`). Did not create or push a `v0.6.0` git tag. | You may need to clarify: "don't tag" vs "don't merge to a 0.6 branch." I read it as "don't tag," and the version bump was done by someone else anyway. |
| "Get done as much as you can" | Fixed 4 distinct test breakages + 1 lint blocker + worktree cleanup; full workspace test pass restored. | None — handoff is this document. |

## 7. Suggested next-session priorities

1. **If you want to tag v0.6.0:** run the §4 pre-tag spot check, then `git tag -a v0.6.0 -m "Single-machine multi-agent, no data loss"` + `git push --tags`. Optionally action task #46 immediately after (or open a CR-L6-style soak window of N days first).
2. **If you want to defer the tag:** action **T-FIN-3** (the dylib-cache bug) so this same failure doesn't re-bite at the eventual tag. Action **T-FIN-2** (allowlist matcher gets exact-file support) if you want stricter allowlisting hygiene.
3. **Either way:** the documented audit-handoff trilogy (2026-05-24-lost-work + 2026-05-25-state-of-the-work + this doc) is the canonical post-incident record. Future agents diagnosing similar parallel-commit churn should read all three.

## 8. Related

- `session-handoff-2026-05-24-lost-work-audit.md`
- `session-handoff-2026-05-25-state-of-the-work.md`
- `work-loss-audit-and-handoff-2026-05-24.md` (parallel agent's whole-workspace forensic)
- `post-sprint-forward-plan-2026-05-25.md` §10 (v0.6 acceptance criteria)
- `vox-stdlib-gap-audit-2026-05-23.md §Phase H` (the soak-gated retirement procedure for task #46)
