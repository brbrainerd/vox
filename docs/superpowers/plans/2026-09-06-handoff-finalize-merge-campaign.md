# Handoff: Finalize the Vox Merge Campaign

> **For agentic workers (any model/tool — Grok, Claude, Codex, a human):**
> This document is self-contained. You have no memory of the conversation
> that produced it. Read this whole file before touching anything. Where
> this file gives you a fact ("main is at `<sha>`"), **re-verify it** —
> several other agent tabs are working in this same repository concurrently
> and the numbers below are a snapshot, not a live value.

**Repo:** `vox-foundation/vox` (GitHub), local clone at `/Users/brbrainerd/dev/vox`.
**Your job:** land the remaining work, close the remaining gaps, clean up
finished worktrees, and report a final state — without breaking anything a
concurrent tab is mid-edit on.

**Read this first, in full, before running any command that writes to git,
GitHub, or the filesystem.** The "Hazards" section (§2) exists because each
entry cost real time or nearly caused real damage in the session that
produced this handoff. Skipping it is how you repeat them.

---

## 0. Snapshot at handoff time (2026-09-06, verify before trusting)

```
main:            2f864278c
open PRs:        1  (#513 — mergeable, checks running)
disk free:       867 GiB
main target/:    33 GB
```

Re-run this before doing anything else:

```bash
cd /Users/brbrainerd/dev/vox
git fetch -q origin && git checkout -q main && git reset -q --hard origin/main
git rev-parse --short HEAD
git status --porcelain          # should be empty or only the two files in §3.1
gh pr list -R vox-foundation/vox --limit 20 --json number,title,mergeable
git worktree list
df -g / | awk 'NR==2{print $4"Gi free"}'
```

If any of these disagree with the snapshot above, **trust what you just
measured**, not this document's numbers. Only the *instructions* below are
durable; the *state* is not.

---

## 1. What this campaign is and where it came from

A design spec (`docs/superpowers/specs/2026-09-04-distribution-and-plugin-architecture.md`)
was split into 7 parallel workstreams (P1–P7), indexed in
`docs/superpowers/plans/2026-09-05-00-INDEX.md` — **read that file's §2
(settled decisions), §5 (what to do when you need a file you don't own), and
§7 (definition of done) before writing any code.** It defines file ownership
boundaries that still apply.

Two later, unrelated plans reused the "P8"/"P9" numbering for different work
(a Tailwind v4 migration and an Astro v7 migration — see §5 below). This is a
known naming collision, not a versioning scheme to extend. Don't add a "P10".

Status of the original 7 plans, verified against `main` at handoff time:

| Plan | Status |
|---|---|
| P1 (plugin capability + checksum gate) | **Done**, merged (#504) |
| P2 (toolchain SSOT) | **Done**, merged (#502) |
| P3 (build broker) | **Done**, merged (#501) |
| P4 (CI lanes) | **Done**, merged (#502 + follow-ups); also fixed a bug not in the original plan — see §4.1 |
| **P5 (desktop/installer)** | **NOT STARTED.** Zero commits touching any owned path. This is the largest real gap — see §4.2 |
| P6 (payload SSOT) | **Done**, merged (#501) |
| P7 (lean core) | **Done**, merged directly |

---

## 2. Hazards — read before you touch anything

Every item here happened for real in this session. Some cost an hour; one
came within one command of destroying another tab's work.

### 2.1 This repo has multiple agent tabs working in it concurrently, right now

`/Users/brbrainerd/dev/vox` is the **shared main worktree**. Other Claude
Code / agent sessions read and write files in it directly, live, while you
are also working there. Symptoms observed this session: untracked files
appearing mid-task that nobody in the current conversation created; a
tracked file's diff changing between two `git status` calls; a worktree's
"unmerged commit count" going from >0 to 0 without this session merging it.

**Rules:**
- Before any command that could discard uncommitted work (`git checkout --`,
  `git restore`, `git reset --hard`, `git clean`, `git stash` on paths you
  didn't create), run `git status --porcelain` on exactly those paths first.
  If you see a change you don't recognize authoring, **do not touch it** —
  it is very likely another tab's in-flight work. Leave it, note it in your
  final report, move on.
- Never `git stash` broadly ("stash everything") in the shared worktree —
  you cannot tell your own scratch state from someone else's real work.
- For any branch-creation, rebase, or file-surgery task, prefer an
  **isolated clone** over working directly in the shared worktree:
  ```bash
  git clone -q /Users/brbrainerd/dev/vox /private/tmp/scratch-$$
  cd /private/tmp/scratch-$$
  git remote set-url origin https://github.com/vox-foundation/vox.git
  ```
  Do your rebase/patch/verify/push there, then `rm -rf` it. This sidesteps
  every concurrent-edit race described above.
- `gh pr create` / `gh pr merge` / `gh pr comment` operate against GitHub,
  not local files — safe to run from anywhere, including inside a scratch
  clone, using `-R vox-foundation/vox` to be explicit about the repo.

### 2.2 `npm i --no-save` silently does nothing in a pnpm workspace

It reports success, changes nothing installed, and a subsequent build
produces a **byte-identical** output — which you might mistake for "the
bump is safe" when actually your install never took effect. Check for
`pnpm-lock.yaml` vs `package-lock.json` in the target directory and use the
matching manager. **After any install, re-read the version straight out of
`node_modules/<pkg>/package.json`** and confirm it actually changed before
trusting any before/after comparison.

### 2.3 An identical build/bundle output means opposite things depending on context

- For a **types-only** package (`@types/node`, etc.): identical output is
  the *correct*, expected result — types don't emit anything.
- For a **runtime** package (a build plugin, a UI library): identical
  output is a **red flag** that the install silently failed (see §2.2).

Always confirm which case you're in before interpreting "nothing changed."

### 2.4 A pnpm lockfile can merge with no textual conflict and still be broken

pnpm encodes peer-resolution into lockfile entry **keys** (e.g. `vitest`'s
key embeds the exact `jsdom` version it resolved against). If PR A bumps
`jsdom` and PR B (touching the same `pnpm-lock.yaml`, different lines)
merges after it, Git may auto-merge the YAML with **no conflict markers**
while leaving the *content* inconsistent — `pnpm install --frozen-lockfile`
(what CI runs) then fails with `ERR_PNPM_LOCKFILE_MISSING_DEPENDENCY`.

**Rule: after merging two or more PRs that touch the same `pnpm-lock.yaml`
in sequence, immediately verify:**
```bash
cd <package dir>
pnpm install --frozen-lockfile   # must exit 0
```
If it fails, fix with `pnpm install --no-frozen-lockfile` (regenerates the
lockfile correctly) — never hand-edit the YAML conflict.

### 2.5 A commit whose parent predates recent merges can conflict even with correct content

If you rebase a branch by copying *file content* from `origin/main` into an
old commit (`git checkout origin/main -- <file>`) rather than actually
rebasing/rebuilding the commit as a child of `origin/main`, GitHub's 3-way
merge can still report `CONFLICTING` — because it merges by **commit
ancestry**, not by final content. If a PR shows `CONFLICTING` despite your
diff looking trivial, check whether the branch's parent commit is actually
`origin/main`'s current tip:
```bash
git log -1 --format=%H origin/main
git merge-base <your-branch> origin/main   # should equal the line above
```
If not, rebuild the branch as a true child of `origin/main`:
```bash
git checkout -B <branch> origin/main
# ...make your edit...
git commit -m "..."
git push -f origin <branch>:<remote-branch-name>
```

### 2.6 Every dependency bump that caused real damage this session had one shape

**A new major version targets a host framework/runtime version this repo
does not pin, and declares no `peerDependencies` strong enough to warn you.**

- `tailwind-merge` 2→3 targets Tailwind v4; repo pins Tailwind ^3.4.19. v3
  silently dropped `focus-visible:outline` from every button (the keyboard
  focus ring), because on Tailwind v4 `outline-2` implies `outline-style:
  solid` so merging them is correct there — but on v3 they're separate CSS
  properties. **No peerDependency warned. CI was green.** Found only by
  diffing merged output over 1457 real class strings; PR closed.
- `starlight-llms-txt` 0.11.0 pulled `@astrojs/mdx` 7 (peer `astro ^7.0.0`);
  repo pins astro ^6. `pnpm` **warns, does not fail**, on an unmet peer, so
  it installed anyway and broke the docs build for all 931 pages on `main`.

**Before merging any major-version dependency bump:** identify what host
framework/runtime version the new major targets (read its `package.json`
`peerDependencies`, and its actual behavior if the peer field is absent or
loose) and compare against what this repo pins. An absent or wide
`peerDependencies` is not evidence of safety — it's the absence of a check
that would otherwise catch this exact failure.

### 2.7 Assert on artifacts, never on exit codes alone

`cmd > /tmp/x.log 2>&1; echo $?` in a shell one-liner is unreliable — a
compound command's `$?` reflects the *last* thing that ran, which after a
pipeline or a `tail` is not your command. **Always separate the run from
the check:**
```bash
pnpm test > /tmp/x.log 2>&1
echo "exit=$?"
grep -E "Tests +[0-9]+ passed" /tmp/x.log || { echo "NO TESTS RAN"; exit 1; }
```
A test run reporting `0 passed` (or `running 0 tests`) is a **failure**, not
a pass — it means nothing was collected (wrong file filter, wrong feature
flag, a `#![cfg(...)]` gate nothing enables). This exact bug was found live
in this repo: `crates/vox-ml-cli/tests/no_runtime_cargo.rs` is gated behind
`--features gpu`, which **no CI lane ever passes** — so a real regression
guard had never executed, ever, despite existing and passing its own tests.
Before trusting any test-suite result, confirm the count is nonzero and
plausible.

### 2.8 A control that looks like a control is the worst kind of bug

This repo's **sole required branch-protection check**,
`Check, Build, and Test (Rust)`, was satisfied on every PR within one second
of opening — by a *different* workflow (`ci-fallback-hosted.yml`) that
shared the same job name and fired on every `synchronize` event, then
**skipped** (no `fleet-down` label) and posted a `skipped` conclusion under
that name. GitHub counts a skipped required check as **satisfied**. So every
PR was mergeable before a single crate had compiled.

Fixed by: restricting that workflow's trigger to `labeled` only, and adding
`vox ci required-context-guard` (in `crates/vox-cli-ci/src/required_context_guard.rs`),
wired into `ci.yml`'s `lints` job, which fails if any workflow but `ci.yml`
claims the required context name while reachable from an ordinary PR event.

**When you add or modify a CI gate, guard, or required check: prove it can
fail, not just that it can pass.** Deliberately break the condition it's
supposed to catch and confirm the guard goes red, then restore and confirm
green. A guard nobody has watched fail is not a guard — see §2.7's
`no_runtime_cargo.rs` example and this one for two independent instances of
exactly this failure mode in one repo.

### 2.9 Union-resolving a merge conflict needs a *reason*, not just a shape match

Two real merge conflicts this repo has hit were each resolved *plausibly and
wrongly*: a moved-plus-reformatted function produced a silent duplicate
definition with no conflict markers at all; two branches each correctly
edited a different row of the same table, and taking either side wholesale
passed its own tests while asserting something false about the merged tree.

**Union-resolve (keep both sides) is safe only for append-only registries** —
index tables, `mod` lists, match arms appended at the end, where two
additions provably cannot mean different things. It is **actively wrong**
anywhere one side may have *moved*, *renamed*, or *replaced* something. See
`docs/superpowers/plans/2026-09-05-00-INDEX.md` §7.1 for the full worked
examples before resolving any non-trivial Rust conflict.

---

## 3. Immediate open items (verify counts against §0 before starting)

### 3.1 Two untracked files sitting in the shared `main` worktree

```
docs/src/architecture/true-workflow-durability-design-2026.md
docs/superpowers/plans/2026-09-05-true-workflow-durability.md
```

These were **not created by this session** and are not tracked in git. They
appear to belong to a concurrent tab's in-progress work (mesh /
interpreter-first-execution design). **Do not delete them.** Options, in
order of preference:
1. If you can identify the owning tab/session (check `.claude/worktrees/*`
   for a matching branch or in-progress design doc), ask them to commit or
   discard.
2. If genuinely abandoned (check the file's own content/timestamp — is it a
   stale duplicate of something already committed elsewhere under
   `docs/superpowers/plans/`? Several `2026-09-05-*` plan docs already
   exist), copy them somewhere safe (`/private/tmp/`) before touching `main`,
   then ask the user before deleting.
3. If truly unowned and truly redundant, commit them properly (`git add` +
   a real commit) rather than leaving `main`'s working tree permanently
   dirty — a dirty shared worktree is itself a hazard (see §2.1).

### 3.2 PR #513 — should be close to landing

`fix(cli): vox test states its cargo dependency honestly` — branch
`claude/vox-cli-cargo-cleanup-470499`. Verify current CI status:
```bash
gh pr checks 513 -R vox-foundation/vox --json name,bucket --jq 'group_by(.bucket)|map("\(.[0].bucket): \(length)")|.[]'
gh pr checks 513 -R vox-foundation/vox --json name,bucket --jq '.[]|select(.bucket=="fail")|.name'
```
If 0 failures and checks have settled: merge it (see §6 for the merge
command pattern; this repo currently requires `--admin` because the
self-hosted fleet is slow, not because the gate is broken — see §4.1 for why
that's now safe to trust).

If there ARE failures, diagnose each one individually — do not assume they
relate to this PR's content. In this session, all four of this PR's earlier
failures were **unrelated infrastructure gaps** it happened to expose (see
§4.3), not bugs in the PR's own 39-line diff.

### 3.3 Worktrees needing triage (verify against `git worktree list` in §0)

For **every** worktree, before removing it: `git -C <path> cherry main HEAD
| grep -c '^+'` must print `0` (zero unmerged commits) **and**
`git -C <path> status --porcelain` must be empty (no uncommitted changes).
Never remove a worktree that fails either check — that is how work gets
destroyed. If unmerged commits exist, either merge them (open a PR, verify,
merge) or leave the worktree alone and report it in your final summary.

At handoff time:

| Worktree | Branch | Unmerged | Dirty | Action |
|---|---|---|---|---|
| `vox-fix-lint` | `fix/items-after-test-module-lint` | 1 | 0 | Review the 1 commit; if it's a trivial lint fix, open a PR and merge; otherwise report |
| `vox-mens-hub` | `mens/mac-hub-enablement` | 10 | 2 | **Active work, likely another tab's.** 2 dirty files means someone is mid-edit. Do not touch without confirming it's abandoned. |
| `agent-a2397266d152b5e29` | `mesh-phase3-populi-httpop` | 3 | 0 | Mesh work — leave running (see §3.4) |
| `agent-aaeae3f3c6956474e` | `mesh-phase3-queue-stats` | 4 | 0 | Mesh work — leave running |
| `agent-ab95c8baec40253a8` | `mesh-phase3-a2a-mailbox` | 4 | 0 | Mesh work — leave running |
| `blissful-ptolemy-9d8527` | `mesh-phase3-plan` | 11 | 0 | Mesh work — leave running |
| `fervent-nightingale-1b550b` | `claude/vox-populi-mesh-dev-d21a17` | 0 | 0 | **Safe to remove** if truly inactive — verify no session is attached to it first |
| `kind-panini-9530f5` | `mesh-phase3` | 8 | 1 | Mesh work — leave running |
| `vox-cli-cargo-cleanup-470499` | `claude/vox-cli-cargo-cleanup-470499` | 1 | 0 | This is PR #513 (§3.2) — remove once merged |
| `xenodochial-poincare-816141` | `claude/plan-p3p6-broker-payload-33cc68` | 0 | 0 | **Safe to remove** — same branch name as the already-merged #501, this is a fresh worktree with nothing new on it |

**The user said "populi mesh development continues"** — do not merge, close,
or remove any `mesh-phase3-*` worktree or branch without explicit
instruction, even though several show unmerged commits. That is expected
and intentional for ongoing work, not a gap to close.

### 3.4 Disk hygiene

`target/` directories are per-worktree and grow to tens of GB each. Safe,
reversible cleanup (costs only rebuild time, never git history):
```bash
# Only after confirming no cargo/rustc process is running:
ps -Ao pid,etime,comm | grep -E "[c]argo|[r]ustc"   # must be empty
# Then, for worktrees confirmed fully merged and about to be removed,
# the target/ goes with them automatically via `git worktree remove`.
# For worktrees staying (mesh-phase3-*), leave target/ alone — active work.
```
Do not proactively `rm -rf` a `target/` dir belonging to a worktree that is
still in use — only remove worktrees wholesale once verified merged per §3.3.

---

## 4. Real gaps to close (ranked by value)

### 4.1 Highest value, cheapest fix: no SSOT guard for pnpm or Node versions

`contracts/toolchain/workspace-toolchain.v1.yaml` declares:
```yaml
versions:
  rust: "1.98.1"
  node: "22.0.0"
  pnpm: "9.1.0"
```
But every CI workflow actually installs **pnpm 11** and **Node 24**
(`grep -rh "version: '\?1[01]" .github/workflows/*.yml` and
`grep "node-version" .github/workflows/ci.yml` will confirm this). This is
the *exact* shape of bug P2 was built to fix for Rust — a contract that
lies, with nothing enforcing it — just not yet fixed for the other two
toolchains.

**Task:** write `vox ci node-pnpm-ssot-guard` (or extend an existing guard),
modeled directly on `crates/vox-cli-ci/src/toolchain_workflow_lint.rs` and
`crates/vox-cli-ci/src/required_context_guard.rs` — both are short,
well-commented, and structurally exactly what's needed here: parse every
`.github/workflows/*.yml`, extract `node-version:` and pnpm `version:`
values, and fail if any disagrees with the SSOT file (or if the SSOT file
itself is stale — in which case, fix the SSOT to say `pnpm 11` / `node 24`
and add the guard to prevent future drift either way). Wire it into `ci.yml`
next to the existing guards in the `lints` job. **Prove it can fail before
declaring it done** — see §2.8.

### 4.2 Largest gap: P5 (desktop/installer) was never executed

Owns: `crates/vox-gui/tauri.conf.json`, `crates/vox-gui/icons/`,
`crates/voxup/`, `Formula/`, `crates/vox-cli/wix/`, `scripts/install.sh`,
`scripts/install.ps1`, `docs-astro/public/voxup*`.

Read `docs/superpowers/plans/2026-09-05-p5-desktop-installer.md` in full —
it has its own task breakdown, already written, never executed. Follow it
directly; do not re-plan from scratch. Two things to check are still current
before you start, since time has passed since it was written:
- §2.1 of the INDEX file (tier↔bundle taxonomy) is binding on this plan's
  Task 6 — confirm `bundle_resolved()` at
  `crates/vox-plugin-catalog/src/lib.rs:72` is still the single
  implementation (§2 of the INDEX forbids a second spelling).
- Confirm no other tab has since touched any of P5's owned paths (see the
  ownership table in `2026-09-05-00-INDEX.md` §3) before starting, to avoid
  a conflict with concurrent work.

### 4.3 A `vox compile` scaffold's `pnpm install` fails — real, unrelated to any current PR

Reproduce:
```bash
D=/private/tmp/vox-compile-repro && rm -rf "$D" && mkdir -p "$D"
cargo run -q -p vox-cli -- build examples/golden/option_type.vox -o "$D"
# Watch for "Step 3/5: Installing dependencies & building" then
# "Error: pnpm install / build failed" — this is what CI's
# "vox compile --help (Linux self-hosted)" check hits.
```
This failed in CI with only `Error: pnpm install failed` and no underlying
pnpm stderr surfaced — the tool swallows the real error. First step:
find where `vox compile`/`vox build`'s scaffolding invokes `pnpm install`
(search `crates/vox-cli` and `crates/vox-codegen` for the literal string
`"pnpm install / build failed"`) and make it print the actual pnpm stderr,
then re-run to see the real cause. Given this session's pattern (§2.6, §4.1),
check first whether the generated scaffold's own `package.json`/
`pnpm-workspace.yaml` template is missing an `allowBuilds` entry, same as
four other packages already were — that is the most likely single-line fix,
but **verify it, don't assume it** (see §2.7's "prove before you trust"
principle throughout this whole document).

### 4.4 A macOS-hosted-runner CI flake, low priority

`GUI/orchestrator relaunch smoke (CR-U6) — macOS` fails with:
```
error: could not execute process `sccache ...` (never executed)
No such file or directory (os error 2)
```
This is `mozilla-actions/sccache-action` not installing the binary on that
specific hosted runner — infra, not this repo's code. Lowest priority; if
you have time, check whether pinning that action to a specific version (vs
`@latest`) or adding a `which sccache || true` diagnostic step helps
localize it further. Do not spend more than a cursory look on this relative
to §4.1–4.3.

### 4.5 Two dependency-suppression migrations, already scoped, not yet executed

Both are **complete, self-contained implementation plans** — read and follow
them directly, do not re-plan:

- `docs/superpowers/plans/2026-09-05-p8-tailwind-v4-migration.md` — migrates
  `crates/vox-gui/ui` from Tailwind v3 to v4. Retires the `tailwind-merge`
  dependabot ignore in `.github/dependabot.yml`. Includes a measured
  170-rename inventory and an ordering hazard (`rounded-sm`→`rounded-xs`
  must run before `rounded`→`rounded-sm`, or you'll double-rewrite 154
  classes silently). The plan's Task 6 lists the exact acceptance check
  (generated-CSS rule diff, not just "does it build" — see §2.6 for why a
  green build is not sufficient evidence for this specific package).
- `docs/superpowers/plans/2026-09-05-p9-astro-v7-migration.md` — migrates
  `docs-astro` from Astro v6 to v7. Retires **two** dependabot ignores
  (`@astrojs/starlight`, `starlight-llms-txt`) at once. Includes the
  measured version matrix (note: Starlight 0.42 wants `astro ^7.2.10`, not
  the `^7.0.2` that 0.41 wants — pin Astro to satisfy the Starlight version
  you choose, not the reverse) and a page-inventory-diff acceptance test,
  because the failure mode here is silent content loss (an `{{#include}}`
  directive rendering as empty rather than erroring), not a build failure.

Do these **before** re-opening or re-triaging the closed PRs that
correspond to them (search closed PRs for `tailwindcss` and `astro` in the
title) — they are the reason those PRs were closed rather than merged, and
they become mergeable/mootable once the migration lands.

### 4.6 One dependabot ignore has no path to resolution — needs a human decision

```yaml
- dependency-name: "recharts"
  # ... grew the dashboard chart/grid chunk from under 120 KiB gzipped to
  # ~123.2 KiB, failing the explicit budget assertion ...
```
This isn't a migration — it's a product/perf-budget call
(`DASHBOARD_CHUNK_GZIP_BUDGET_BYTES` in the dashboard bundle budget test).
Not yours to decide; flag to the user with the two options (raise the
budget, or hold `recharts` back further) rather than picking one.

---

## 5. Verified NOT gaps (to save you re-investigating them)

These were checked in the session that produced this handoff and confirmed
fine — don't spend time re-litigating unless something has since changed:

- **Generated `@table` projects compile fine.** `vox_db::DbConfig::resolve_canonical()`
  is gated behind `vox-db`'s `host-integration` feature, and the codegen's
  generated `Cargo.toml` never explicitly requests it — this looks like a
  bug on inspection. It is not: `host-integration` is enabled **transitively**
  through `vox-actor-runtime`/`vox-orchestrator`'s own feature requirements
  in the same Cargo build graph. Confirmed by actually generating a
  table-bearing project and reading the real `rustc --cfg` invocation
  (`--cfg 'feature="host-integration"'` was present). If you're tempted to
  "fix" this, generate a real project and compile it first — see §2.7.
- **`cross-platform-summary`'s `if: always()` + skipped-as-pass pattern is
  correct**, unlike the required-context bug in §2.8. Its `path-check` job
  cannot itself be skipped on a `pull_request` event, so the failure mode
  described in §2.8 doesn't apply there.
- `setup-e2e.yml`'s `rustup default stable` is a deliberate clean-room
  bootstrap after `rustup self uninstall`, not a toolchain-pin bypass —
  `rust-toolchain.toml` still governs in-repo `cargo` regardless.

---

## 6. How to merge things, given the current gate

The required check, `Check, Build, and Test (Rust)`, is real and enforced
correctly now (§2.8's fix), but the self-hosted fleet is often slow, leaving
PRs `BLOCKED` with 0 failures and many `pending`. That is a real
verification gap in progress, not a broken gate — **do not treat a pending
gate as equivalent to a broken one**, and do not disable or bypass the gate
itself. If you need to merge before the fleet finishes:

```bash
gh pr checks <N> -R vox-foundation/vox --json name,bucket \
  --jq '.[]|select(.bucket=="fail")|.name'
# must be EMPTY before you proceed
gh pr merge <N> -R vox-foundation/vox --merge --admin \
  --body "Admin merge, authorized by repo owner. Zero failing checks; remainder pending on a slow self-hosted fleet. <cite what you verified locally, e.g. cargo check exit codes, test counts>"
```

`--admin` bypasses branch protection — only ever use it when you have
independently confirmed zero failures (not just "not yet failed"), and say
so explicitly in the merge message. Never use `--admin` to skip a check that
is still running with unknown outcome, and never to merge over an actual
failure.

---

## 7. Definition of done for this handoff

You are finished when, **all simultaneously true and re-verified, not
assumed from this document**:

1. `main`'s working tree is clean, or every remaining untracked/dirty file
   is explicitly accounted for in your final report with a reason it wasn't
   touched (see §3.1's handling rules).
2. §4.1 (pnpm/Node SSOT guard) is implemented, wired into CI, and proven to
   fail before being proven to pass (§2.8).
3. §4.2 (P5) is either fully executed against its existing plan, or you have
   made real, checkpointed progress and left a clear resume point — don't
   silently drop it.
4. §4.3 and §4.4 are at minimum root-caused (even if not fully fixed) and
   reported with your findings.
5. §4.5 (Tailwind v4, Astro v7) are executed following their existing plans,
   or explicitly deferred with a stated reason.
6. §4.6 is surfaced to the user as a decision, not resolved unilaterally.
7. Every worktree in §3.3 marked "safe to remove" has been removed **after**
   re-verifying zero unmerged commits and zero dirty files at removal time
   (state may have changed since this document was written).
8. Every `mesh-phase3-*` worktree is untouched, per explicit user instruction.
9. A final report is produced in the same shape as §0 of this document —
   current main SHA, open PR count, disk free, worktree list — so whoever
   reads it next doesn't have to re-derive the starting point you had.

Do not consider any item "probably fine" — this document exists because
several things that looked fine on inspection were not (§4's "verified NOT
gaps" section exists to save you from re-discovering the ones that *were*
fine; everything else needs your own verification, not trust in a prior
agent's summary, including this one).
