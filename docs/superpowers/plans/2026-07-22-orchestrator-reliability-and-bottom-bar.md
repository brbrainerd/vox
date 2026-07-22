# Orchestrator Reliability, Build Parity, and Bottom Status Bar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Waves A and B below are mutually independent — REQUIRED SUB-SKILL for cross-wave dispatch: superpowers:dispatching-parallel-agents. Within Wave A, Tasks 1/3/6 are independent of each other and of Task 2; Task 2 depends on Task 1's wire-format change landing first. Within Wave B, Tasks 7 and 11 are independent leaf fixes; Tasks 8-10 and 12 have real sequential dependencies (see each task's header). Every task with a "Write the failing test" step follows superpowers:test-driven-development.

**Goal:** Implement all three specs from the 2026-07-22 brainstorming session — (1) promote the orchestrator-daemon relaunch smoke test to a required CI gate and audit v1.0 readiness against the foundation-criteria doc, (2) detect and soft-warn on a version mismatch between the running GUI and the orchestrator-daemon binary it's talking to, and (3) consolidate the top `TopHud`/`StatusBar` KPI strips into one configurable, VS-Code-style bottom status bar.

**Architecture:** Specs 1 and 3 share Rust backend surface (`crates/vox-gui/src/commands/daemon.rs`, `crates/vox-orchestrator/src/orch_daemon/mod.rs`, `crates/vox-cli-core/src/daemon_ipc/process_supervision.rs`) and share ONE CI job rather than two competing ones. Spec 2 is a pure `crates/vox-gui/ui` frontend change, fully independent of the other two, and consolidates the already-working `useHudTiles`/`useHudTilesConfig` tile-config system (today only surfaced in Settings) into a new `BottomStatusBar.tsx` component, retiring `TopHud.tsx` and `StatusBar.tsx`.

**Tech Stack:** Rust (Tokio, serde_json) for the daemon/GUI backend; React + TypeScript + Tailwind + Vitest for the frontend; GitHub Actions for CI.

**Execution structure:**
- **Wave A** (Rust backend + CI, specs "orchestrator-launch-v1-readiness" and "build-version-parity"): Task 1 (ping response gains a `version` field) → Task 2 (GUI compares versions, soft-warns) is sequential on Task 1. Task 3 (wire `probe_binary_version` into the staging fallback) is independent of 1/2 — different code path (the pre-connect staging fallback vs. the post-connect ping check). Task 4 (promote CR-U6's smoke test + add the shared CI job) depends on Tasks 1 and 2 existing (the CI job's new version-assertion step needs the `version` field to assert against) but not on Task 3. Task 6 (v1.0 readiness audit) is pure documentation, independent of everything else in this plan — dispatch it any time.
- **Wave B** (frontend, spec "bottom-status-bar"), fully independent of Wave A, dispatch in parallel with it: Task 7 (fix the `'compute'` dead-route bug) and Task 11 (richer mesh data source) are independent leaf fixes with no shared file. Task 8 (build `BottomStatusBar.tsx`, consolidating tile rendering) must land before Task 9 (add the configurability dropdown to it) and Task 10 (move the Panels ▾ portal target into it). Task 12 (wire `BottomStatusBar` into `AppShell.tsx`, retire `TopHud`/`StatusBar`) requires Tasks 7-11 all complete.
- Recommended dispatch: parallel wave of {Task 1, Task 3, Task 6, Task 7, Task 11} → then {Task 2 (needs 1), Task 8 (needs 7 landed first since it touches the same tile-render logic, and 11 for the mesh data)} → then {Task 4 (needs 1+2), Task 9, Task 10 (need 8)} → then Task 12 (needs 7-11) → Task 5 folds into Task 4's CI job, not separate.

---

### Task 1: Daemon ping response reports its own version

**Files:**
- Modify: `crates/vox-orchestrator/src/orch_daemon/mod.rs:376-383`
- Test: `crates/vox-orchestrator/src/orch_daemon/mod.rs` (inline `#[cfg(test)]` module — read the file's existing test module structure first to match its conventions; if none exists in this file, add one following the pattern used by sibling files in `crates/vox-orchestrator/src/orch_daemon/`)

Current code (confirmed live in the file while writing this plan):
```rust
orch_daemon_method::PING => response_result(
    &req.id,
    serde_json::json!({
        "ok": true,
        "repository_id": repository_id,
        "protocol": "vox.orchestrator_daemon/v1",
    }),
),
```

- [ ] **Step 1: Write the failing test**

Find `dispatch_request`'s test coverage (grep `dispatch_request` and `orch_daemon_method::PING` across `crates/vox-orchestrator/src/orch_daemon/` for an existing test harness that constructs a fake `Orchestrator` and calls `dispatch_request` — reuse that exact harness rather than building a new one). Add:

```rust
#[tokio::test]
async fn ping_response_includes_the_running_binary_version() {
    let (repository_id, orch) = /* use the existing test harness's setup, e.g. test_orchestrator() or similar helper already in this test module */;
    let req = DispatchRequest {
        id: "1".to_string(),
        method: orch_daemon_method::PING.to_string(),
        params: serde_json::json!({}),
    };
    let resp = dispatch_request(&repository_id, orch, &req).await;
    let value = resp.payload_as_result_value(); // adapt to however this test module already extracts the Ok payload from a DispatchResponse — check an existing test for the real accessor
    assert_eq!(
        value.get("version").and_then(|v| v.as_str()),
        Some(env!("CARGO_PKG_VERSION")),
        "ping response must report the running daemon's own workspace version"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator ping_response_includes_the_running_binary_version`
Expected: FAIL — `value.get("version")` is `None` today.

- [ ] **Step 3: Add the `version` field**

```rust
orch_daemon_method::PING => response_result(
    &req.id,
    serde_json::json!({
        "ok": true,
        "repository_id": repository_id,
        "protocol": "vox.orchestrator_daemon/v1",
        "version": env!("CARGO_PKG_VERSION"),
    }),
),
```

`env!("CARGO_PKG_VERSION")` resolves at compile time to this crate's own `Cargo.toml` version, which — per this session's own investigation — is `version.workspace = true`, i.e. the shared root-workspace version. This is exactly the value that changes when the workspace version bumps, so it faithfully reports "which build is this."

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator ping_response_includes_the_running_binary_version`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/orch_daemon/mod.rs
git commit -m "feat(orchestrator): ping response reports the running daemon's workspace version"
```

---

### Task 2: GUI compares daemon-reported version against its own, soft-warns on mismatch

**Files:**
- Modify: `crates/vox-gui/src/commands/daemon.rs`
- Test: `crates/vox-gui/src/commands/daemon.rs` (inline test module — read the file's existing tests first, likely already covering `PersistentDaemon::ensure`/`reensure`, to match conventions)
- Modify: `crates/vox-gui/ui/src/hooks/useOrchestratorStatus.ts` (surface the mismatch to the frontend)
- Modify: `crates/vox-gui/ui/src/lib/backendGuard.ts` or wherever `BackendBanner`'s state is sourced (read the actual current wiring first — this session's earlier work referenced `backendAvailable()`/`__TAURI_INTERNALS__` in this file; confirm whether banner state is driven from here or from `useOrchestratorStatus.ts` directly before deciding where the new mismatch flag lives)
- Test: `crates/vox-gui/ui/src/hooks/useOrchestratorStatus.test.ts` (or wherever its existing tests live)

Requires Task 1 landed (the `version` field must exist in the ping response for this task's Rust-side comparison to have anything real to compare against).

**Step-by-step is split Rust-side (detect + expose via a Tauri command) then TS-side (surface as UI state)** since this crosses the IPC boundary — read both halves before starting either, the TS half's test can be written first against a mocked Rust response shape.

- [ ] **Step 1: Write the failing Rust-side test**

In `crates/vox-gui/src/commands/daemon.rs`, find the ping call site inside `PersistentDaemon::ensure_live`/`reensure` (confirmed at approximately lines 113-166 and 194-232 while writing this plan — re-read fresh, these are read-only trace points, not being restructured). Add a small, separately-testable helper function rather than inlining the comparison logic directly in the async connect loop:

```rust
/// Compares the daemon's self-reported `version` (from its ping response)
/// against this GUI binary's own compile-time version. Returns `None` when
/// they match (or the daemon's response is missing the field — an older
/// daemon binary pre-dating Task 1, treated as "unknown, don't warn" rather
/// than "mismatch", since we can't distinguish an old-but-compatible daemon
/// from a genuinely incompatible one without the field). Returns
/// `Some((daemon_version, gui_version))` on a confirmed mismatch.
pub fn detect_version_mismatch(ping_response: &serde_json::Value) -> Option<(String, String)> {
    let daemon_version = ping_response.get("version")?.as_str()?.to_string();
    let gui_version = env!("CARGO_PKG_VERSION").to_string();
    if daemon_version != gui_version {
        Some((daemon_version, gui_version))
    } else {
        None
    }
}

#[cfg(test)]
mod version_mismatch_tests {
    use super::*;

    #[test]
    fn no_mismatch_when_versions_match() {
        let resp = serde_json::json!({"ok": true, "version": env!("CARGO_PKG_VERSION")});
        assert_eq!(detect_version_mismatch(&resp), None);
    }

    #[test]
    fn mismatch_detected_when_versions_differ() {
        let resp = serde_json::json!({"ok": true, "version": "0.0.1-stale"});
        let result = detect_version_mismatch(&resp);
        assert_eq!(
            result,
            Some(("0.0.1-stale".to_string(), env!("CARGO_PKG_VERSION").to_string()))
        );
    }

    #[test]
    fn no_mismatch_reported_when_version_field_missing() {
        let resp = serde_json::json!({"ok": true});
        assert_eq!(detect_version_mismatch(&resp), None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-gui version_mismatch_tests`
Expected: FAIL — `detect_version_mismatch` doesn't exist yet.

- [ ] **Step 3: Confirm it passes (the implementation above is already the minimal correct version)**

Run: `cargo test -p vox-gui version_mismatch_tests`
Expected: PASS (3/3).

- [ ] **Step 4: Wire the check into the connect path and expose it via a Tauri command**

Read `PersistentDaemon`'s struct definition (confirmed to hold a `resolved: RwLock<Option<(String, String)>>` cache of `(addr, token)` while writing this plan) and add a sibling field caching the last-detected mismatch:

```rust
// Add alongside the existing `resolved` field on PersistentDaemon:
pub last_version_mismatch: tokio::sync::RwLock<Option<(String, String)>>,
```

(Initialize it to `None` wherever the struct's constructor/`Default` currently initializes `resolved` — read that constructor fresh, don't guess its exact shape.)

At each of the three `ping()` call sites already tracked in this file (`ensure_live`, `reensure`'s adopt branch, `reensure`'s spawn-and-poll branch), after a successful `.ping().await` that returns `Ok(value)`, call `detect_version_mismatch(&value)` and write the result into `last_version_mismatch`. Add a new `#[tauri::command]`:

```rust
#[tauri::command]
pub async fn orchestrator_version_mismatch(
    state: tauri::State<'_, PersistentDaemon>,
) -> Option<(String, String)> {
    state.last_version_mismatch.read().await.clone()
}
```

Register it in `main.rs`'s `tauri::generate_handler!` list alongside the other daemon commands (read the current list fresh to find the right spot — do not guess its current contents).

- [ ] **Step 5: TS-side — write the failing test for the mismatch banner state**

Read `crates/vox-gui/ui/src/hooks/useOrchestratorStatus.ts` in full first — find how it already polls/derives state (it drives the offline/reconnecting banner per earlier investigation) and match its exact polling/invoke pattern for the new check rather than inventing a separate one. Add a test (in that hook's existing test file) asserting: when `invoke('orchestrator_version_mismatch')` resolves to `['0.0.1-stale', '0.6.0']`, the hook's returned state includes a `versionMismatch: {daemon: '0.0.1-stale', gui: '0.6.0'}` (or whatever field name matches this hook's existing naming convention — check its other exposed fields, e.g. `usesPolling`/`listenFailed`, for the house style before naming this one).

- [ ] **Step 6: Run test to verify it fails, then implement, then verify it passes**

Run the hook's test file before and after adding the `orchestrator_version_mismatch` invoke call to confirm RED then GREEN — exact commands depend on this project's existing per-file test-run convention (`npx vitest run src/hooks/useOrchestratorStatus.test.ts`, confirmed as the pattern used throughout this session for `crates/vox-gui/ui`).

- [ ] **Step 7: Surface the mismatch as a visible banner**

Find wherever `BackendBanner` (or the offline/reconnecting banner it's a sibling of) actually renders today (read `crates/vox-gui/ui/src/lib/backendGuard.ts` and its consumer component fresh — this session's earlier work referenced it but did not modify it, confirm its current render location before adding to it) and add a second banner variant (or extend the existing one) that shows when `versionMismatch` is set: `"GUI v{gui} / daemon v{daemon} — restart the daemon to update"`, dismissible, non-blocking (the app continues to function — this is the approved soft-warning choice, not a hard block).

- [ ] **Step 8: Run the full suite**

Run: `cargo test -p vox-orchestrator -p vox-gui` and `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-gui/src/commands/daemon.rs crates/vox-gui/src/main.rs crates/vox-gui/ui/src/hooks/useOrchestratorStatus.ts
git commit -m "feat(gui): detect orchestrator-daemon version mismatch, soft-warn via banner"
```

---

### Task 3: Wire `probe_binary_version` into the stale-staged-binary fallback path

**Files:**
- Modify: `crates/vox-cli-core/src/daemon_ipc/process_supervision.rs`
- Test: `crates/vox-cli-core/src/daemon_ipc/process_supervision.rs` (existing `#[cfg(test)] mod tests` block, confirmed present at the bottom of this file while writing this plan)

Independent of Tasks 1/2 — this is the *pre-connect* fallback (an old binary sitting in `~/.vox/bin` with no fresh sibling to re-stage from), a different code path from the *post-connect* ping-based check.

- [ ] **Step 1: Write the failing test**

Current `resolve_or_stage_daemon` (confirmed live in the file):
```rust
pub fn resolve_or_stage_daemon(src: &Path, dest_dir: &Path) -> std::io::Result<PathBuf> {
    if src.exists() {
        return stage_binary(src, dest_dir);
    }
    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    Ok(resolve_managed_binary_path(name))
}
```

Add a test in this file's existing `#[cfg(test)] mod tests` block asserting the fallback branch (the one taken when `src` doesn't exist) logs/returns a probed version alongside the path, rather than silently returning a bare `PathBuf` with no version information at all:

```rust
#[test]
fn resolve_or_stage_reports_none_version_hint_when_falling_back_with_no_probeable_binary() {
    // When src doesn't exist AND no binary is resolvable via PATH/~/.vox/bin
    // either (a clean test env), resolve_or_stage_daemon_with_version_hint
    // should still return a path (matching today's behavior) but a None
    // version hint, not panic or silently claim a version.
    let tmp = tempfile::tempdir().unwrap();
    let nonexistent_src = tmp.path().join("does-not-exist-vox-orchestrator-d");
    let dest_dir = tmp.path().join("dest");
    let (_path, version_hint) =
        resolve_or_stage_daemon_with_version_hint(&nonexistent_src, &dest_dir);
    assert_eq!(version_hint, None);
}
```

(Use this crate's existing `tempfile` dev-dependency if already present — check `Cargo.toml`; if absent, use `std::env::temp_dir().join(format!("vox-test-{}", std::process::id()))` with manual cleanup instead of adding a new dependency, consistent with YAGNI.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli-core resolve_or_stage_reports_none_version_hint`
Expected: FAIL — `resolve_or_stage_daemon_with_version_hint` doesn't exist yet.

- [ ] **Step 3: Add the version-probing variant, wired to the fallback branch**

```rust
/// Like `resolve_or_stage_daemon`, but when falling through to an
/// already-staged (not freshly re-staged from a sibling) binary, also probes
/// its reported `--version` so callers can warn on a stale/mismatched daemon
/// BEFORE even attempting to launch it — the earliest possible signal for
/// the "old staged binary paired with a new GUI build" bug class.
pub fn resolve_or_stage_daemon_with_version_hint(
    src: &Path,
    dest_dir: &Path,
) -> (std::io::Result<PathBuf>, Option<String>) {
    if src.exists() {
        // Freshly re-staged from a live sibling — this IS the current build,
        // no version hint needed (there's nothing to compare against yet;
        // Task 2's ping-based check covers this case once the daemon is
        // actually running).
        return (stage_binary(src, dest_dir), None);
    }
    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let resolved = resolve_managed_binary_path(name);
    let version_hint = probe_binary_version(name);
    (Ok(resolved), version_hint)
}
```

Update `resolve_or_stage_daemon` (the existing function) to delegate to this new one, discarding the hint, so existing callers are unaffected:

```rust
pub fn resolve_or_stage_daemon(src: &Path, dest_dir: &Path) -> std::io::Result<PathBuf> {
    resolve_or_stage_daemon_with_version_hint(src, dest_dir).0
}
```

Update `crates/vox-gui/src/commands/daemon.rs`'s call site (confirmed at approximately line 187 while writing this plan, inside `reensure`) to call the new `_with_version_hint` variant instead, and when the hint is `Some(daemon_version)` and differs from `env!("CARGO_PKG_VERSION")`, write it into the same `last_version_mismatch` field Task 2 introduced (reuse that field — don't add a second one) — this means the mismatch banner can fire even before the daemon has successfully answered a single ping, closing the gap the investigation specifically flagged as unguarded today.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli-core resolve_or_stage_reports_none_version_hint`
Expected: PASS.

- [ ] **Step 5: Run the full crate suite (this function has existing callers)**

Run: `cargo test -p vox-cli-core -p vox-gui`
Expected: PASS, no regression in `resolve_or_stage_daemon`'s existing callers/tests.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli-core/src/daemon_ipc/process_supervision.rs crates/vox-gui/src/commands/daemon.rs
git commit -m "feat(daemon-ipc): probe staged daemon binary version in the stale-fallback path"
```

---

### Task 4: Promote CR-U6 smoke test to a required CI gate (shared job for specs 1 and 3)

**Files:**
- Modify: `crates/vox-gui/tests/gui_relaunch_smoke.rs`
- Modify: `.github/workflows/ci.yml`

Requires Tasks 1 and 2 landed (this task's new CI assertion step needs the `version` field and the mismatch-detection helper to exist).

- [ ] **Step 1: Remove the `#[ignore]` gate, keep the env-var opt-out as a local-dev convenience**

Current test attribute (confirmed live while writing this plan):
```rust
#[tokio::test]
#[ignore = "needs a built vox-orchestrator-d; set VOX_GUI_RELAUNCH_SMOKE=1"]
async fn gui_relaunch_boots_daemon_and_core_surfaces_respond() {
    if !relaunch_smoke_enabled() {
        eprintln!("skipping gui relaunch smoke: set VOX_GUI_RELAUNCH_SMOKE=1");
        return;
    }
    ...
```

Remove the `#[ignore]` attribute entirely (the test's own internal `relaunch_smoke_enabled()` early-return already makes it a safe no-op for anyone running `cargo test` locally without the daemon binary built — the `#[ignore]` was double-gating the same condition two different ways, which is exactly why it was easy to forget to also enable it in CI). The test now runs by default whenever `VOX_GUI_RELAUNCH_SMOKE=1` is set in the environment, with no separate `--ignored` flag needed.

- [ ] **Step 2: Add the version-parity assertion to the same test**

Extend `gui_relaunch_boots_daemon_and_core_surfaces_respond` (the existing test body, after the existing `ping().await.is_ok()` poll loop succeeds) with a new assertion using this plan's Task 1/2 additions:

```rust
    // (3) The relaunched daemon's self-reported version matches this build's
    // own workspace version — a real CI guard against ever shipping a
    // version-reporting regression (e.g. a hardcoded stale version string).
    let ping_response = client.ping().await.expect("ping should succeed");
    assert_eq!(
        ping_response.get("version").and_then(|v| v.as_str()),
        Some(env!("CARGO_PKG_VERSION")),
        "relaunched daemon's ping response version must match this build's workspace version"
    );
```

- [ ] **Step 3: Run the test locally to confirm it passes with a real daemon build**

Run: `cargo build -p vox-orchestrator-d && VOX_GUI_RELAUNCH_SMOKE=1 cargo test -p vox-gui --test gui_relaunch_smoke -- --nocapture`
Expected: PASS (both the original assertions and the new version-parity one).

- [ ] **Step 4: Add the shared CI job**

Read `.github/workflows/ci.yml` in full first (it's large — confirmed to `--exclude vox-gui` in most existing jobs per this session's earlier investigation) to match its existing job structure/naming conventions exactly. Add a new job (name suggestion: `gui-orchestrator-relaunch-smoke`, adjust to match this file's actual naming convention once read) that:
1. Checks out the repo.
2. Builds `vox-orchestrator-d` and `vox-gui` (just the Rust crate, not the Tauri frontend bundle — this test doesn't need the UI built) — e.g. `cargo build -p vox-orchestrator-d -p vox-gui` (confirm the exact build invocation this repo's other Rust CI jobs use, don't invent a different one).
3. Runs `VOX_GUI_RELAUNCH_SMOKE=1 cargo test -p vox-gui --test gui_relaunch_smoke`.
5. Is added to whatever "required checks" list/branch-protection config this repo uses for merge-blocking (check if `.github/workflows/ci.yml` itself defines a summary/gate job that aggregates required jobs — if so, add this new job to that list; if required-checks are configured outside this file, e.g. GitHub branch protection settings, flag this as a manual follow-up step for the human to apply, since it may not be file-based).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/tests/gui_relaunch_smoke.rs .github/workflows/ci.yml
git commit -m "ci: promote CR-U6 relaunch smoke test to a required gate, assert version parity"
```

---

### Task 5: (folded into Task 4 — no separate task)

The build-version-parity spec's CI requirement ("assert the reported version field matches the workspace version") is implemented as Task 4 Step 2's assertion, running inside the same job as the CR-U6 promotion, per this plan's explicit "one shared job, not two competing ones" design decision. Do not create a second, separate CI job for this.

---

### Task 6: v1.0 readiness inventory (documentation, independent of all other tasks)

**Files:**
- Create: `docs/src/architecture/v1-readiness-status-2026-07.md`
- Read (not modified): `docs/src/architecture/v1-foundation-criteria-research-2026.md`

- [ ] **Step 1: Read the full foundation-criteria doc**

Read `docs/src/architecture/v1-foundation-criteria-research-2026.md` in full — every CR-F, CR-K, and CR-U criterion it defines, not just CR-U6 (already handled by Tasks 1-4).

- [ ] **Step 2: For each criterion, determine and record its real current status**

For each CR-*, check the actual codebase/CI for evidence: is there a passing, non-ignored, CI-enforced test/check that verifies it (✅ Built & Verified), does the relevant code/mechanism exist but lack that verification (⚠️ Built, Unverified — CR-U6 was exactly this before Task 4), or does neither exist (❌ Unbuilt)? Cross-reference against project memory's prior claim that "CR-F/K/U harnesses UNBUILT" to confirm whether that's still accurate given this session's other work, or whether it's stale.

- [ ] **Step 3: Write the status doc**

```markdown
---
title: v1.0 Readiness Status (2026-07-22 audit)
---

# v1.0 Readiness Status

Audit of docs/src/architecture/v1-foundation-criteria-research-2026.md's CR-F/CR-K/CR-U
criteria as of 2026-07-22, following the CR-U6 promotion in this same effort
(docs/superpowers/plans/2026-07-22-orchestrator-reliability-and-bottom-bar.md).

| Criterion | Status | Evidence | Follow-up needed |
|---|---|---|---|
| CR-U6 | ✅ Built & Verified | crates/vox-gui/tests/gui_relaunch_smoke.rs, now a required CI gate | none |
| ... | ... | ... | ... |

## Summary

[N] of [total] criteria confirmed Built & Verified. [M] Built-but-Unverified
(candidates for a follow-up promotion effort, same pattern as CR-U6). [K]
genuinely Unbuilt (candidates for new specs, not attempted in this effort).
```

Fill in the real table from Step 2's findings — this is the actual deliverable, do not leave placeholder rows.

- [ ] **Step 4: Commit**

```bash
git add docs/src/architecture/v1-readiness-status-2026-07.md
git commit -m "docs: v1.0 readiness audit against foundation-criteria doc"
```

---

### Task 7: Fix TopHud's mesh tile dead-route bug

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/TopHud.tsx:198`
- Test: `crates/vox-gui/ui/src/components/layout/TopHud.test.tsx` (read existing file first for conventions; if none exists, check `StatusBar.test.tsx` or `Sidebar.test.tsx` for this codebase's render-helper pattern)

Independent leaf fix — read `TopHud.tsx` fresh before editing (confirmed current content while writing this plan, but Task 8/12 will heavily modify this file later in this same plan, so land this fix first and let Task 8 build on top of it, not the other way around).

- [ ] **Step 1: Write the failing test**

```tsx
it('mesh tile navigates to the real agents/mesh view, not a nonexistent compute route', () => {
  const onNavigate = vi.fn();
  render(
    <TopHud
      kpis={mockKpis} // use this file's existing mock kpis fixture; if none, construct the minimal shape TopHud's renderTile('mesh_peers') branch reads (kpis.mesh.value/unit/delta/spark/peers)
      onCommand={vi.fn()}
      onNavigate={onNavigate}
      visibleTiles={['mesh_peers']}
    />,
  );
  fireEvent.click(screen.getByText('Mesh').closest('button')!);
  expect(onNavigate).toHaveBeenCalledWith('mesh');
  expect(onNavigate).not.toHaveBeenCalledWith('compute');
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/TopHud.test.tsx -t "mesh tile navigates"`
Expected: FAIL — currently calls `onNavigate('compute')`.

- [ ] **Step 3: Fix the navigation target**

Find (line ~198):
```tsx
onClick={() => onNavigate?.('compute')}
```
inside the `case 'mesh_peers':` branch of `renderTile`. Replace with:
```tsx
onClick={() => onNavigate?.('mesh')}
```
(`'mesh'` is the real child view key, resolving via `PARENT_CHILD_MAP`/`resolveNavigation` to `{parent: 'agents', child: 'mesh'}` — confirmed in `lib/navigation.ts` during this session's investigation; there is no `'compute'` top-level parent in this codebase.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/TopHud.test.tsx`
Expected: PASS, including all pre-existing tests in this file.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/TopHud.tsx crates/vox-gui/ui/src/components/layout/TopHud.test.tsx
git commit -m "fix(gui): mesh HUD tile navigates to the real agents/mesh view, not a nonexistent compute route"
```

---

### Task 8: `BottomStatusBar.tsx` — consolidate TopHud + StatusBar tile rendering

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/BottomStatusBar.test.tsx`

Requires Task 7 (the mesh-route fix) landed first, since this task moves `renderTile`'s cases wholesale — build on the corrected version, not the buggy one. Independent of Task 11 for its initial version (Task 11 upgrades the mesh tile's data richness afterward); this task can use the existing bare-peer-count mesh rendering as a starting point.

**Read `TopHud.tsx` and `StatusBar.tsx` fresh in full before starting** (both fully re-read while writing this plan; re-confirm nothing changed if executing this task later than the others).

- [ ] **Step 1: Write the failing test**

```tsx
// crates/vox-gui/ui/src/components/layout/BottomStatusBar.test.tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BottomStatusBar } from './BottomStatusBar';
import { defaultHudTiles } from '../../hooks/useHudTiles';
import { INITIAL_KPIS } from '../../data/initialState';

describe('BottomStatusBar', () => {
  it('renders every enabled tile as a compact one-line segment', () => {
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={defaultHudTiles()}
        onNavigate={vi.fn()}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    expect(screen.getByTestId('bottom-status-bar')).toBeInTheDocument();
    expect(screen.getByText('Agents')).toBeInTheDocument();
    expect(screen.getByText('Mesh')).toBeInTheDocument();
  });

  it('a disabled tile in hudTilesConfig does not render', () => {
    const config = defaultHudTiles();
    config.tiles = config.tiles.map((t) =>
      t.kind === 'mesh_peers' ? { ...t, enabled: false } : t,
    );
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={config}
        onNavigate={vi.fn()}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    expect(screen.queryByText('Mesh')).not.toBeInTheDocument();
  });

  it('clicking the agents segment navigates to the agents view', () => {
    const onNavigate = vi.fn();
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={defaultHudTiles()}
        onNavigate={onNavigate}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    fireEvent.click(screen.getByText('Agents').closest('button')!);
    expect(onNavigate).toHaveBeenCalledWith('agents');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/BottomStatusBar.test.tsx`
Expected: FAIL — `Cannot find module './BottomStatusBar'`.

- [ ] **Step 3: Write the component**

Build `BottomStatusBar.tsx` by combining `StatusBar.tsx`'s compact `Segment` component/layout (one-line, `Glass` row, `h-7`) with `TopHud.tsx`'s full `HudTileKind` switch (all 7 kinds, not just the 5 `StatusBar` shows today — `STATUS_BAR_TILE_KINDS`'s carve-out filter is removed since there's no more separate `TopHud` to own the other two), filtered by `resolveVisibleHudTiles(hudTilesConfig)` instead of a static `visibleTiles` prop:

```tsx
// crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx
import React from 'react';
import { Glass } from '../ui/Glass';
import { formatBudgetCap } from '../../config/budget';
import { useFreshness } from '../../hooks/useFreshness';
import { resolveVisibleHudTiles, type HudTilesConfig, type HudTileKind } from '../../hooks/useHudTiles';
import { INITIAL_KPIS } from '../../data/initialState';

type KpiState = typeof INITIAL_KPIS;

export interface BottomStatusBarProps {
  kpis: KpiState;
  hudTilesConfig: HudTilesConfig;
  onNavigate: (view: string) => void;
  lastOrchEventAt: number | null;
  orchUsesPolling: boolean;
  liveFreshMs: number;
  activeModel?: string | null;
  openrouterSpendUsd?: number | null;
  pendingApprovals?: number | null;
}

function freshnessClasses(tone: 'live' | 'poll' | 'stale') {
  if (tone === 'live') return { pill: 'border-emerald-400/20 bg-emerald-400/[0.04] text-emerald-300', dot: 'bg-emerald-400', label: 'Live' };
  if (tone === 'poll') return { pill: 'border-amber-400/20 bg-amber-400/[0.04] text-amber-300', dot: 'bg-amber-400', label: 'Poll' };
  return { pill: 'border-border-subtle bg-overlay-subtle text-text-muted', dot: 'bg-text-muted', label: 'Offline' };
}

function Segment({ testId, label, value, onClick }: { testId: string; label: string; value: string; onClick: () => void }) {
  return (
    <button type="button" data-testid={testId} onClick={onClick}
      className="inline-flex items-center gap-1.5 rounded px-2 py-0.5 text-[10px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary transition">
      <span className="uppercase tracking-[0.14em] text-text-muted">{label}</span>
      <span className="font-mono tabular-nums text-text-secondary">{value}</span>
    </button>
  );
}

export function BottomStatusBar({
  kpis,
  hudTilesConfig,
  onNavigate,
  lastOrchEventAt,
  orchUsesPolling,
  liveFreshMs,
  activeModel = null,
  openrouterSpendUsd = null,
  pendingApprovals = null,
}: BottomStatusBarProps) {
  const tone = useFreshness(lastOrchEventAt, { freshMs: liveFreshMs, usesPolling: orchUsesPolling });
  const fresh = freshnessClasses(tone);
  const visible = resolveVisibleHudTiles(hudTilesConfig);

  const budgetSource = kpis.budgetBurn?.source ?? 'fallback';
  const capDisplay = formatBudgetCap(budgetSource === 'daemon' ? kpis.budgetBurn.cap : null, budgetSource);
  const budgetValue = `$${kpis.budgetBurn.value.toFixed(2)}/${capDisplay}`;

  const renderSegment = (kind: HudTileKind): React.ReactNode => {
    switch (kind) {
      case 'active_agents':
        return <Segment key={kind} testId="bottom-status-bar-agents" label="Agents" value={String(kpis.activeAgents.value)} onClick={() => onNavigate('agents')} />;
      case 'queue_depth':
        return <Segment key={kind} testId="bottom-status-bar-queue" label="Queue" value={String(kpis.queueDepth.value)} onClick={() => onNavigate('runs')} />;
      case 'budget_burn':
        return <Segment key={kind} testId="bottom-status-bar-budget" label="Budget" value={budgetValue} onClick={() => onNavigate('settings')} />;
      case 'mesh_peers':
        // Bare peer count for now — Task 11 upgrades this to online/total or queue-depth.
        return <Segment key={kind} testId="bottom-status-bar-mesh" label="Mesh" value={`${kpis.mesh.peers} peers`} onClick={() => onNavigate('mesh')} />;
      case 'active_model':
        return <Segment key={kind} testId="bottom-status-bar-model" label="Model" value={activeModel ?? 'auto-route'} onClick={() => onNavigate('models')} />;
      case 'openrouter_spend':
        return <Segment key={kind} testId="bottom-status-bar-openrouter" label="OR Spend" value={openrouterSpendUsd == null ? '—' : `$${openrouterSpendUsd.toFixed(2)}`} onClick={() => onNavigate('settings')} />;
      case 'pending_approvals':
        return <Segment key={kind} testId="bottom-status-bar-approvals" label="Approvals" value={String(pendingApprovals ?? 0)} onClick={() => onNavigate('approvals')} />;
      default:
        return null;
    }
  };

  return (
    <Glass data-testid="bottom-status-bar" role="status" aria-label="Operator status"
      className="flex h-7 items-center gap-1 px-3 text-[10px] text-text-muted">
      <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        {visible.map((kind) => renderSegment(kind))}
      </div>
      <div data-testid="bottom-status-bar-freshness"
        className={`ml-auto inline-flex shrink-0 items-center gap-1.5 rounded border px-2 py-0.5 ${fresh.pill}`}>
        <span className={`size-1.5 rounded-full ${fresh.dot}`} />
        <span className="uppercase tracking-[0.14em]">{fresh.label}</span>
      </div>
    </Glass>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/BottomStatusBar.test.tsx`
Expected: PASS (3/3).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx crates/vox-gui/ui/src/components/layout/BottomStatusBar.test.tsx
git commit -m "feat(gui): add BottomStatusBar, consolidating TopHud+StatusBar tile rendering"
```

(Note: `TopHud.tsx`/`StatusBar.tsx` are NOT deleted by this task — that's Task 12, after the new bar is fully wired and proven. This task only adds the new component alongside the old ones.)

---

### Task 9: Configurability dropdown on `BottomStatusBar` (Panels ▾ pattern)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/BottomStatusBar.test.tsx`

Requires Task 8 landed. Read `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (the Panels ▾ implementation, confirmed at approximately lines 950-1038 during this session's investigation) fresh before starting, to mirror its exact interaction pattern.

- [ ] **Step 1: Write the failing test**

```tsx
it('the configure trigger opens a live-apply checkbox menu that stays open across toggles', () => {
  const onHudTilesChange = vi.fn();
  render(
    <BottomStatusBar
      kpis={INITIAL_KPIS}
      hudTilesConfig={defaultHudTiles()}
      onHudTilesChange={onHudTilesChange}
      onNavigate={vi.fn()}
      lastOrchEventAt={null}
      orchUsesPolling={false}
      liveFreshMs={10_000}
    />,
  );
  fireEvent.click(screen.getByRole('button', { name: /configure/i }));
  const meshCheckbox = screen.getByRole('checkbox', { name: /mesh peers/i });
  expect(meshCheckbox).toBeChecked();
  fireEvent.click(meshCheckbox);
  expect(onHudTilesChange).toHaveBeenCalledTimes(1);
  // Menu stays open — a second checkbox is still reachable without re-opening.
  const budgetCheckbox = screen.getByRole('checkbox', { name: /budget burn/i });
  fireEvent.click(budgetCheckbox);
  expect(onHudTilesChange).toHaveBeenCalledTimes(2);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/BottomStatusBar.test.tsx -t "configure trigger"`
Expected: FAIL — no "Configure" trigger exists yet.

- [ ] **Step 3: Add the trigger + dropdown**

Add to `BottomStatusBarProps`: `onHudTilesChange: (config: HudTilesConfig) => void;`. Add local `menuOpen` state and the trigger+dropdown, mirroring `ChatSurface.tsx`'s Panels ▾ structure (outside-click/Escape-to-close via a ref-based listener, checkboxes bound directly to `hudTilesConfig.tiles`, live-apply via `toggleHudTile` on every change, no separate "Apply" step):

```tsx
import { useEffect, useRef, useState } from 'react';
import { toggleHudTile, HUD_TILE_LABELS } from '../../hooks/useHudTiles';
// ... inside BottomStatusBar, alongside existing hooks:
const [menuOpen, setMenuOpen] = useState(false);
const menuRef = useRef<HTMLDivElement>(null);
const triggerRef = useRef<HTMLButtonElement>(null);

useEffect(() => {
  if (!menuOpen) return;
  const onOutside = (e: MouseEvent) => {
    if (menuRef.current?.contains(e.target as Node) || triggerRef.current?.contains(e.target as Node)) return;
    setMenuOpen(false);
  };
  const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setMenuOpen(false); };
  document.addEventListener('mousedown', onOutside);
  document.addEventListener('keydown', onKey);
  return () => {
    document.removeEventListener('mousedown', onOutside);
    document.removeEventListener('keydown', onKey);
  };
}, [menuOpen]);

// ... in the JSX, before the freshness pill:
<div className="relative shrink-0">
  <button ref={triggerRef} type="button" onClick={() => setMenuOpen((o) => !o)}
    aria-expanded={menuOpen} aria-label="Configure status bar"
    className="rounded px-1.5 py-0.5 text-[10px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary transition">
    Configure ▾
  </button>
  {menuOpen && (
    <div ref={menuRef} className="absolute bottom-full right-0 mb-1 w-56 rounded-lg border border-border-subtle bg-bg-base p-2 shadow-2xl">
      {hudTilesConfig.tiles.map((tile) => (
        <label key={tile.id} className="flex items-center gap-2 rounded px-2 py-1 text-[11px] text-text-secondary hover:bg-overlay-subtle">
          <input
            type="checkbox"
            checked={tile.enabled}
            onChange={(e) => onHudTilesChange(toggleHudTile(hudTilesConfig, tile.id, e.target.checked))}
            className="rounded border-border-subtle bg-bg-base text-brass focus:ring-brass/40 focus:ring-offset-bg-base size-3.5"
          />
          {HUD_TILE_LABELS[tile.kind]}
        </label>
      ))}
    </div>
  )}
</div>
```

(Dropdown is positioned `bottom-full` — opening upward from the bottom bar, mirroring VS Code's own upward-opening status-bar menus, rather than `top-full` which would open off-screen below the viewport edge.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/BottomStatusBar.test.tsx`
Expected: PASS, including all Task 8 tests still passing.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx crates/vox-gui/ui/src/components/layout/BottomStatusBar.test.tsx
git commit -m "feat(gui): add live-apply configure menu to BottomStatusBar, matching Panels ▾'s pattern"
```

---

### Task 10: Move the Panels ▾ portal target into `BottomStatusBar`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/BottomStatusBar.test.tsx`

Requires Task 8 landed. This is a narrow, mechanical move: `StatusBar.tsx`'s `#workbench-tabbar-trailing-slot` div (confirmed at lines 181-185 during this session, with its explanatory comment about why it needs a persistent non-wrapping home) moves into `BottomStatusBar.tsx`. `StatusBar.tsx` itself is NOT deleted yet (Task 12's job) — this task only adds the slot to the new component so Task 12 has somewhere to move the real portal-consuming code to.

- [ ] **Step 1: Write the failing test**

```tsx
it('renders the workbench-tabbar-trailing-slot portal target', () => {
  render(
    <BottomStatusBar
      kpis={INITIAL_KPIS}
      hudTilesConfig={defaultHudTiles()}
      onHudTilesChange={vi.fn()}
      onNavigate={vi.fn()}
      lastOrchEventAt={null}
      orchUsesPolling={false}
      liveFreshMs={10_000}
    />,
  );
  expect(document.getElementById('workbench-tabbar-trailing-slot')).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/BottomStatusBar.test.tsx -t "trailing-slot"`
Expected: FAIL — the slot doesn't exist in this component yet.

- [ ] **Step 3: Add the slot**

Copy the slot div (and its explanatory comment, updated to reflect its new home) from `StatusBar.tsx` into `BottomStatusBar.tsx`, placed after the Configure trigger and before/alongside the freshness pill (exact visual ordering is an implementation-time call — match whatever reads cleanest once both are in place, verified visually in Task 12's live-CDP pass, not a hard requirement here):

```tsx
<div
  id="workbench-tabbar-trailing-slot"
  data-testid="workbench-tabbar-trailing-slot"
  className="ml-2 flex shrink-0 items-center"
/>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/BottomStatusBar.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx crates/vox-gui/ui/src/components/layout/BottomStatusBar.test.tsx
git commit -m "feat(gui): add workbench-tabbar-trailing-slot portal target to BottomStatusBar"
```

(Note: `StatusBar.tsx` still ALSO has this same `id` at this point in the plan — having it defined in two components simultaneously, before Task 12 removes it from `StatusBar.tsx`, would be a real duplicate-DOM-id bug if both rendered at once. Task 12 must remove `StatusBar.tsx`'s copy in the SAME commit that stops rendering `StatusBar` at all — sequence Task 12 to do that removal as its very first step, before anything else, to avoid a window where both are mounted together.)

---

### Task 11: Richer mesh indicator sourced from real per-node data

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/BottomStatusBar.test.tsx`
- Read (not modified): `crates/vox-gui/ui/src/components/surfaces/Mesh/MeshView.tsx`

Independent of Tasks 8-10's own landing order in principle (this task's logic could be written standalone), but since it modifies the same `mesh_peers` case in `BottomStatusBar.tsx` that Task 8 first introduces, land it after Task 8 to avoid a merge conflict on the same lines — dispatch it in the same wave as Task 8 but sequence the actual file edit after, or have whichever subagent lands second rebase onto the other's change.

- [ ] **Step 1: Read `MeshView.tsx`'s data-fetching fresh**

Confirm the exact MCP tool call shape (`vox_mesh_nodes`/`vox_mesh_queue_stats`, ~5s `REFRESH_MS` per this session's investigation) and the shape of the per-node `status` field (online/maintenance/quarantined) it already parses — reuse that exact parsing logic (extract a small shared helper if `MeshView.tsx`'s parsing is a self-contained function already; otherwise duplicate the minimal parsing needed rather than doing a larger extraction refactor of a file this plan doesn't otherwise touch, per YAGNI).

- [ ] **Step 2: Write the failing test**

```tsx
it('mesh segment shows online/total node count from real mesh data, not a bare peer count', () => {
  render(
    <BottomStatusBar
      kpis={INITIAL_KPIS}
      hudTilesConfig={defaultHudTiles()}
      onHudTilesChange={vi.fn()}
      onNavigate={vi.fn()}
      lastOrchEventAt={null}
      orchUsesPolling={false}
      liveFreshMs={10_000}
      meshNodes={[
        { status: 'online' }, { status: 'online' }, { status: 'quarantined' },
      ]}
    />,
  );
  expect(screen.getByTestId('bottom-status-bar-mesh')).toHaveTextContent('2/3 online');
});
```

(Adapt the `meshNodes` prop shape to whatever `MeshView.tsx`'s real per-node type actually is once read in Step 1 — this sketch assumes a minimal `{status: string}[]`, correct it to match reality.)

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/BottomStatusBar.test.tsx -t "online/total"`
Expected: FAIL — current mesh segment shows `"${kpis.mesh.peers} peers"`.

- [ ] **Step 4: Implement**

Add a `meshNodes` prop (typed against `MeshView.tsx`'s real node shape) to `BottomStatusBarProps`, and change the `case 'mesh_peers':` branch:

```tsx
case 'mesh_peers': {
  const onlineCount = meshNodes?.filter((n) => n.status === 'online').length ?? 0;
  const totalCount = meshNodes?.length ?? 0;
  const meshValue = meshNodes == null ? `${kpis.mesh.peers} peers` : `${onlineCount}/${totalCount} online`;
  return <Segment key={kind} testId="bottom-status-bar-mesh" label="Mesh" value={meshValue} onClick={() => onNavigate('mesh')} />;
}
```

(Falls back to the original bare-peer-count when `meshNodes` isn't supplied — e.g. before the mesh data has loaded — rather than showing a broken `"0/0 online"` on first render.) Wire the actual data fetch at the `BottomStatusBar`'s call site in `App.tsx`/`AppShell.tsx` (Task 12's job, since that's where the component gets mounted for real) — this task only adds the prop and rendering logic, matching this plan's file-boundary discipline of not editing `AppShell.tsx`/`App.tsx` until Task 12.

- [ ] **Step 5: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/BottomStatusBar.test.tsx`
Expected: PASS, all prior tests in this file still passing (including Task 8's mesh test, which must still pass with `meshNodes` undefined — the fallback path).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx crates/vox-gui/ui/src/components/layout/BottomStatusBar.test.tsx
git commit -m "feat(gui): BottomStatusBar mesh segment shows online/total node count, not a bare peer count"
```

---

### Task 12: Wire `BottomStatusBar` into `AppShell.tsx`, retire `TopHud`/`StatusBar`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/AppShell.tsx`
- Modify: `crates/vox-gui/ui/src/App.tsx`
- Delete: `crates/vox-gui/ui/src/components/layout/TopHud.tsx`, `TopHud.test.tsx`
- Delete: `crates/vox-gui/ui/src/components/layout/StatusBar.tsx`, `StatusBar.test.tsx`

Requires Tasks 7-11 all complete. **Read `AppShell.tsx` and `App.tsx` fresh in full before starting** — both were re-read while writing this plan, but this is the last task to touch them and other work may have landed in between.

- [ ] **Step 1: Add `<BottomStatusBar>` to `AppShell.tsx`, remove `<TopHud>` and `<StatusBar>`**

Current header block (confirmed live in `AppShell.tsx` while writing this plan):
```tsx
<div className="p-4 pb-0">
  <TopHud kpis={kpis} onCommand={onCommand} onOpenCommandPalette={onOpenCommandPalette ?? onCommand}
    lastOrchEventAt={lastOrchEventAt} orchUsesPolling={orchUsesPolling} liveFreshMs={liveFreshMs}
    onNavigate={onNavigate} hudMode={hudMode} setHudMode={setHudMode} workspaceTitle={workspaceTitle}
    visibleTiles={visibleTiles} activeModel={activeModel} openrouterSpendUsd={openrouterSpendUsd}
    pendingApprovals={pendingApprovals} />
  <BreadcrumbBar viewKey={activeView} onNavigate={onNavigate} gamifyEnabled={gamifyEnabled} />
  <StatusBar kpis={kpis} lastOrchEventAt={lastOrchEventAt} orchUsesPolling={orchUsesPolling}
    liveFreshMs={liveFreshMs} onNavigate={onNavigate} gamifyEnabled={gamifyEnabled}
    onOpenAchievements={onOpenAchievements} />
</div>
```

Replace with (keeping `BreadcrumbBar` and the command-palette trigger — `TopHud`'s `omnisearch-trigger` button — since those aren't part of what's moving to the bottom; the command palette trigger needs a new small home in this header row, e.g. inline with `BreadcrumbBar`, since `TopHud` is what used to render it):

```tsx
<div className="p-4 pb-0 flex items-center justify-between">
  <BreadcrumbBar viewKey={activeView} onNavigate={onNavigate} gamifyEnabled={gamifyEnabled} />
  <button type="button" data-testid="omnisearch-trigger" onClick={onOpenCommandPalette ?? onCommand}
    className="inline-flex items-center gap-1.5 rounded border border-border-subtle bg-overlay-subtle px-2 py-0.5 text-xs text-text-muted hover:border-brass/40 hover:text-brass transition">
    <span>Search or jump…</span>
    <span className="rounded border border-border-subtle bg-overlay-subtle px-1 text-[9px] tracking-widest text-text-muted">⌘K</span>
  </button>
</div>
```

Add `<BottomStatusBar>` as a new sibling at the BOTTOM of the shell's `<main>` (after the `chatDocked` block, so it's always the last thing rendered regardless of whether Chat's dock is showing):

```tsx
{chatDocked && chatDock != null && (
  <div className="p-4 pt-0 mt-auto" data-testid="loquela-dock">
    {chatDock}
  </div>
)}
<div className="px-4 pb-2">
  <BottomStatusBar
    kpis={kpis}
    hudTilesConfig={hudTilesConfig}
    onHudTilesChange={onHudTilesChange}
    onNavigate={onNavigate}
    lastOrchEventAt={lastOrchEventAt}
    orchUsesPolling={orchUsesPolling}
    liveFreshMs={liveFreshMs}
    activeModel={activeModel}
    openrouterSpendUsd={openrouterSpendUsd}
    pendingApprovals={pendingApprovals}
    meshNodes={meshNodes}
  />
</div>
```

Update `AppShellProps` accordingly: remove `hudMode`/`setHudMode`/`onCommand`/`visibleTiles` (TopHud-specific, no longer needed — `onOpenCommandPalette` is kept for the relocated search trigger), add `hudTilesConfig: HudTilesConfig`, `onHudTilesChange: (c: HudTilesConfig) => void`, `meshNodes: MeshNode[] | undefined` (type name/shape matching whatever Task 11 actually defined).

- [ ] **Step 2: Update `App.tsx`'s `<AppShell>` call site**

Remove the props no longer accepted (`hudMode`, `setHudMode`, `visibleTiles`, and `onCommand` IF it was only ever used for TopHud — check whether `onCommand` has another consumer in `App.tsx` before removing it entirely, it may still be needed elsewhere e.g. a keybind). Add `hudTilesConfig`/`onHudTilesChange` (already available in `App.tsx` via the existing `useHudTilesConfig()` call, confirmed present in this session's earlier investigation — just pass it through, don't re-derive it). Add `meshNodes` — wire it to whatever hook/state already backs `MeshView.tsx`'s data (if that's currently local to `MeshView.tsx` with no shared hook, extract a minimal `useMeshNodes()` hook so both `MeshView` and this new `AppShell` call site can share one polling loop rather than two independently-polling sources drifting out of sync, per this plan's spec's own explicit requirement).

- [ ] **Step 3: Run the full suite, fix any remaining reference**

Run: `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`
Fix any compile/test failure surfaced by the prop changes (this task's own excerpted reads of `AppShell.tsx`/`App.tsx` may have missed a reference — this is expected for a file this large, fix for real rather than guessing).

- [ ] **Step 4: Delete `TopHud.tsx`/`StatusBar.tsx` and their tests**

First grep to confirm no remaining imports:
```bash
cd crates/vox-gui/ui && grep -rn "from '.*TopHud'\|from '.*StatusBar'" src --include=*.ts --include=*.tsx | grep -v "\.test\."
```
Expected: no output (this task's own Steps 1-2 should have removed every real usage). Then:
```bash
git rm crates/vox-gui/ui/src/components/layout/TopHud.tsx crates/vox-gui/ui/src/components/layout/TopHud.test.tsx
git rm crates/vox-gui/ui/src/components/layout/StatusBar.tsx crates/vox-gui/ui/src/components/layout/StatusBar.test.tsx
```

- [ ] **Step 5: Run the full suite one more time**

Run: `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`
Expected: PASS, clean.

- [ ] **Step 6: Live CDP verification**

Rebuild (`pnpm build` + `cargo build -p vox-gui`), launch with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, confirm: the bottom bar renders at the bottom of the window, one line, shows real KPI/mesh data; the Configure ▾ dropdown live-toggles tiles; the Chat surface's Panels ▾ trigger is reachable from its new home (was portaled into `StatusBar.tsx`, now portals into `BottomStatusBar.tsx` via the same DOM id); no duplicate `id="workbench-tabbar-trailing-slot"` warning in the console (confirming Task 10's noted sequencing risk was actually avoided). Screenshot the result.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(gui): wire BottomStatusBar into AppShell, retire TopHud and StatusBar"
```

---

## Self-Review

**Spec coverage** — orchestrator-launch-v1-readiness: CR-U6 promotion → Task 4; broad v1.0 audit → Task 6. build-version-parity: ping reports version → Task 1; GUI detects+warns → Task 2; stale-staged-binary fallback probe → Task 3; CI parity assertion → folded into Task 4 (explicitly noted, not a gap). bottom-status-bar: mesh dead-route fix → Task 7; tile-system consolidation → Task 8; configurability menu → Task 9; Panels ▾ new home → Task 10; richer mesh data → Task 11; final wiring/retirement → Task 12.

**Placeholder scan** — every code step shows real, complete code grounded in files read fresh while writing this plan; the few remaining "read fresh and adapt" notes (e.g. Task 2 Step 1's exact `DispatchResponse` payload accessor, Task 11's exact `MeshNode` shape) are explicitly flagged as needing a live read rather than asserted as fact, consistent with this session's established practice for genuinely-unconfirmed details — these are disclosed uncertainties with a concrete resolution instruction, not vague "TBD"s.

**Type consistency** — `HudTilesConfig`/`toggleHudTile`/`resolveVisibleHudTiles`/`HUD_TILE_LABELS` (all pre-existing, confirmed via a fresh read of `useHudTiles.ts`) are used with identical names/shapes across Tasks 8-12. `detect_version_mismatch`/`last_version_mismatch` (Task 2) and `resolve_or_stage_daemon_with_version_hint` (Task 3) are each defined once and reused by name in every later task that touches them (Task 3 explicitly reuses Task 2's `last_version_mismatch` field rather than introducing a second one — called out explicitly in Task 3's own text to prevent exactly the kind of drift this checklist item exists to catch).

**Sequencing risk called out explicitly** — Task 10's note about the duplicate-DOM-id window between landing the new slot and removing the old one is flagged with a concrete mitigation (Task 12 removes `StatusBar.tsx`'s copy as its first step) rather than left as an implicit assumption.
