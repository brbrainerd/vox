---
title: "Handoff: 2026-05-24 lost-work forensic audit + recovery plan"
description: "Post-incident audit of what was orphaned during the 2026-05-23/24 parallel-agent commit storm. Identifies the jj-keep snapshot containing the four unfinished work products and gives exact recovery commands."
last_updated: "2026-05-24"
category: "Session handoffs"
status: active
---

# Handoff: lost-work forensic audit (2026-05-23/24)

**Trigger:** multiple parallel Claude sessions reported losing edits during the 2026-05-23 evening / 2026-05-24 early-morning crate-audit execution window. The user requested a forensic audit and a recovery plan.

**Verdict:** **no commits were destroyed.** Every named commit my session created is reachable from `main` today. What was reported as "lost" was uncommitted working-tree state that got overwritten when the harness synchronized to fresh `HEAD`s authored by parallel agents. All of it survives in a single jj-keep snapshot commit (`cea30891cb`) and can be recovered with five `git checkout` invocations.

---

## 1. Timeline of the destructive-looking operations

| Time (UTC-04:00) | SHA | Operation | Effect |
|---|---|---|---|
| 22:43:55 | — | `checkout durable-functions-clean → main` | branch switch, working tree replaced |
| 22:43:56 | `6459133dbc` | `pull --ff-only` | fast-forward to origin/main |
| **22:44:24** | `6459133dbc` | **`reset: moving to origin/main`** | **no-op — HEAD already at origin/main** |
| 23:00–04:27 | many | parallel-agent commits to `main` | each commit triggered a harness working-tree sync, overwriting any uncommitted edits |

The `22:44:24Z` reset originally flagged as the smoking gun was effectively inert — `HEAD` already pointed at `origin/main` when it ran. The real loss vector was the *harness-driven working-tree sync* that fires every time a parallel agent advances `main`. Sessions that hadn't yet committed their edits had their `.vox`/`.rs` buffers replaced by the new `HEAD`'s versions.

## 2. What is and is not in `main` today

`HEAD` is `dc25d46e42` (2026-05-24 04:27:06-04:00).

### 2.1 My session's commits — all preserved

| SHA | Subject |
|---|---|
| `d47bacdbd5` | feat(compiler+corpus): Phase K codegen wire-up + Phase L L.6/L.7 fixes |
| `ca1dba949b` | fix(build): remove duplicate-module stub files |
| `0a8d1518cb` | chore(workspace-audit): integrate parallel crate-audit-track snapshot |
| `3af1b3bc40` | feat(lexer+corpus): hash-padded raw-strings `r#"..."#` + migrate-arrows revival |
| `81bb574166` | chore(workspace-audit): second snapshot |
| `0ae8bce810` | feat(pipeline+docs): --mode script intra-project imports + @json_as RFC + Phase H audit |
| `b0ace9f24f` | test(compiler): add intra_project_imports coverage |
| `df14322b87` | fix(code-audit): flip retired-decorator direction to match Phase B |
| `ba98af25c1` | chore(corpus): retire 6 stale aspirational placeholders |

Verified reachable via `git log --all --oneline`.

### 2.2 Files currently at `HEAD` that match my session's committed work

- `examples/golden/option_type.vox` — `@query` migration **present**.
- `crates/vox-integration-tests/tests/fixtures/{chatbot,full_stack_minimal,greaterfool_reference}.vox` — `@mutation`/`@server` migrations **present**.
- `crates/vox-code-audit/src/detectors/retired_decorator.rs` — flipped direction **present**.
- `docs/src/architecture/json-as-rfc-2026-05-24.md` — full RFC **present**.
- `docs/src/architecture/json-ergonomics-rfc-2026-05-23.md` — **present**.

### 2.3 Files at `HEAD` that are missing the in-flight work

| Path | HEAD state | Desired state (from `cea30891cb`) |
|---|---|---|
| `scripts/gui-build.vox` | pre-migration (`// gui-build.vox\n// Procedural skill…`) | migrated (`// vox:caps fs process` + strict-Option/Result discipline) |
| `scripts/setup.vox` | pre-migration | migrated (`// vox:caps process`) |
| `crates/vox-integration-tests/tests/snapshots/codegen_rust_test__with_all_options_output.snap` | `return x.clone();` (line 15) | `return x;` — accepted snapshot reflecting the Phase K codegen fix |
| `crates/vox-compiler/src/lexer/token.rs` | no `AtJsonAs` token | `AtJsonAs`, `AtFieldName`, `AtDefault`, `AtSkipIfNone` tokens added |
| `crates/vox-compiler/src/ast/decl/typedef.rs` | no `JsonAsAnnotation` | `JsonAsAnnotation`, `JsonAsFieldAttr`, `json_as` field on `TypeDefDecl`, `json_as_attr` on `VariantField` |
| `crates/vox-compiler/src/parser/descent/decl/mid.rs` | no `parse_json_as` | `parse_json_as` (~147 lines) + `parse_bool_literal` helper + construction-site defaults |
| `crates/vox-compiler/src/parser/descent/mod.rs` | no dispatch arm | `Token::AtJsonAs => self.parse_json_as()` |

Grep at `HEAD` for `json_as|AtJsonAs|JsonAsAnnotation|parse_json_as` under `crates/vox-compiler/src` returns **zero matches**, confirming none of Phase M Step 1 landed.

## 3. Where the missing work lives

A jj-managed snapshot commit `cea30891cb` (parent: `ba98af25c1`, empty commit message, dated 2026-05-24 01:43:09-04:00) holds the unfinished WIP state. Verified by direct `git show`:

```
git show cea30891cb:scripts/gui-build.vox     # → // vox:caps fs process …
git show cea30891cb:scripts/setup.vox         # → // vox:caps process …
git show cea30891cb:crates/vox-integration-tests/tests/snapshots/codegen_rust_test__with_all_options_output.snap
                                              # → line 15: `return x;`
git show cea30891cb:crates/vox-compiler/src/lexer/token.rs | grep AtJsonAs
                                              # → AtJsonAs, AtFieldName
git show cea30891cb:crates/vox-compiler/src/ast/decl/typedef.rs | grep -c JsonAsAnnotation
                                              # → 4
```

The full diffstat is `38 files changed, 990 insertions(+), 1078 deletions(-)`. Most of the 38 files are now identical to HEAD (parallel-agent rewrites). The seven files listed in §2.3 are the only ones whose `cea30891cb` version is materially different and still desired.

## 4. Recovery plan

### 4.1 Prerequisite

`vox-rename-registry` was previously a stale workspace member that blocked `cargo build`. **Resolved** — commit `c3ae1ffc4c` (2026-05-24 02:24Z) extracted it as a real L0 crate. `ls crates/vox-rename-registry` shows `Cargo.toml`/`src/`. No prerequisite fixup needed.

### 4.2 Recovery commands

Run from repo root in the order shown. Each step ends with `git add` + `git commit` so nothing re-orphans.

**Step 1 — recover the four script/snapshot files (low-risk, no compile dependency):**

```sh
git checkout cea30891cb -- \
  scripts/gui-build.vox \
  scripts/setup.vox \
  crates/vox-integration-tests/tests/snapshots/codegen_rust_test__with_all_options_output.snap

git commit -m "recover: re-land gui-build/setup migrations + codegen_with_all_options snapshot

Re-applies the migrated strict-Option/Result-discipline rewrites of
scripts/gui-build.vox and scripts/setup.vox, plus the accepted
codegen_with_all_options snapshot (no .clone() on the moved return
value), all of which were orphaned during the 2026-05-23/24
parallel-agent commit storm. Source: jj-keep snapshot cea30891cb.

See docs/src/architecture/session-handoff-2026-05-24-lost-work-audit.md
for the forensic audit."
```

Verify before committing:
```sh
head -3 scripts/gui-build.vox          # expect: // vox:caps fs process
sed -n '15p' crates/vox-integration-tests/tests/snapshots/codegen_rust_test__with_all_options_output.snap
                                       # expect: return x;
cargo test -p vox-integration-tests --test codegen_rust_test -- with_all_options
```

**Step 2 — recover Phase M Step 1 (compiler-coupled; verify build before commit):**

```sh
git checkout cea30891cb -- \
  crates/vox-compiler/src/lexer/token.rs \
  crates/vox-compiler/src/ast/decl/typedef.rs \
  crates/vox-compiler/src/parser/descent/decl/mid.rs \
  crates/vox-compiler/src/parser/descent/mod.rs

cargo build -p vox-compiler          # must succeed before commit
cargo test  -p vox-compiler --lib    # parser/lexer baseline

git commit -m "feat(compiler): Phase M Step 1 — @json_as AST + parser

Adds AtJsonAs/AtFieldName/AtDefault/AtSkipIfNone lexer tokens,
JsonAsAnnotation/JsonAsFieldAttr AST nodes, parse_json_as descent
(~147 lines), and dispatch arm. Per the json-as RFC §9 step 1.

Recovered from jj-keep snapshot cea30891cb after the 2026-05-23/24
parallel-agent commit storm orphaned the working-tree state. See
docs/src/architecture/session-handoff-2026-05-24-lost-work-audit.md."
```

If `cargo build` fails after the checkout, the WIP was incomplete — read the error and either finish the missing wiring inline or `git restore --source=HEAD --staged --worktree crates/vox-compiler/src/{lexer,ast,parser}` to back out cleanly.

### 4.3 Discipline going forward

The root cause is uncommitted edits sitting in the working tree while parallel agents commit to `main`. The remediation is mechanical:

1. **Commit immediately after any compiling edit set.** Don't batch unrelated edits across agent boundaries.
2. **For multi-step features, commit a WIP commit at every green checkpoint** (`cargo check -p <crate>` passes), then squash before push if desired.
3. **Before starting work, capture `git rev-parse HEAD`.** If it changes mid-session without your action, the harness sync'd to a parallel-agent commit — `git stash` your edits before continuing.

## 5. Items deliberately not included in recovery

- 31 other files in `cea30891cb`'s diff are now superseded by parallel-agent commits (`d5ae8e59ba`, `c3ae1ffc4c`, `b9390e1fe7`, `017e05dd3b`, `92760761ef`, `ec5b7bb747`, `3054a88dbb`, `dcbddaf6e3`, `3ef863c862`, `9c83a0d4d0`, `dc25d46e42`). Pulling those back would regress crate-audit progress. Confirmed by spot-comparing diff hunks against `git log -p main -- <path>`.
- Phase M Steps 2–6 (HIR lowering, typeck registration, eval, codegen, golden test) were never started. The RFC at `docs/src/architecture/json-as-rfc-2026-05-24.md` §9 enumerates them.

## 6. Quick sanity-check script

```sh
# Should print three "OK" lines after recovery is complete.
[ "$(head -1 scripts/gui-build.vox)" = "// vox:caps fs process" ] && echo OK || echo FAIL gui-build
[ "$(head -1 scripts/setup.vox)" = "// vox:caps process" ] && echo OK || echo FAIL setup
grep -q "AtJsonAs" crates/vox-compiler/src/lexer/token.rs && echo OK || echo FAIL token
```

## 7. Recovery outcome (2026-05-24 ~05:00Z)

Both recovery steps executed successfully — but with a twist on Step 2.

| Step | Commit | Notes |
|---|---|---|
| 1 | `eeffc5a6be` | `recover: re-land gui-build/setup migrations + codegen_with_all_options snapshot` — three files via `git checkout cea30891cb`. Clean. |
| 2 | `2884287d08` | **Swept into a parallel agent's commit.** I checked out the four Phase M Step 1 files, fixed three `.into()` ambiguity errors (bytes + winnow both impl `From<&str>` after recent dep updates; `ParseError::classified` takes `impl Into<String>` which accepts `&str` directly), ran `cargo build -p vox-compiler` → OK and `cargo test -p vox-compiler --lib` → 277 passed. While I was staging for commit, parallel agent `AI Assistant` ran `git add` + commit (`refactor(container): extract vox-container-types`, 04:56:24-04:00) that swept my staged compiler files into their commit. Verified at HEAD: `AtJsonAs`=2, `JsonAsAnnotation`=4, `parse_json_as`=1 fn, dispatch arm=1 — all present. Build + tests still green. |

**Verification evidence (run at HEAD = `2884287d08`):**
- `git show HEAD:crates/vox-compiler/src/lexer/token.rs | grep -c AtJsonAs` → `2`
- `git show HEAD:crates/vox-compiler/src/ast/decl/typedef.rs | grep -c JsonAsAnnotation` → `4`
- `git show HEAD:crates/vox-compiler/src/parser/descent/decl/mid.rs | grep -c "fn parse_json_as"` → `1`
- `git show HEAD:crates/vox-compiler/src/parser/descent/mod.rs | grep -c AtJsonAs` → `1`
- `cargo build -p vox-compiler` → `Finished` (exit 0)
- `cargo test -p vox-compiler --lib` → `277 passed; 0 failed; 8 ignored`

**New datapoint about the workspace dynamics:** parallel agents are running `git add` on the whole working tree before committing their own work. This means staged-but-not-committed files from another session can ride along on an unrelated commit. The mitigation is the same as §4.3 — commit immediately, don't leave things staged.

## 8. Related

- `docs/src/architecture/json-as-rfc-2026-05-24.md` — Phase M target design.
- `docs/src/architecture/json-ergonomics-rfc-2026-05-23.md` — the strict-Option Json surface Phase M builds atop.
- `docs/src/architecture/intra-project-imports-rfc-2026-05-23.md` — the `T::from_json(j)` dispatch path Phase M consumes.
- `docs/src/architecture/crate-audit-and-plan-2026.md` — the parallel work track that drove the commit storm.
