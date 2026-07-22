---
title: Orchestrator/GUI build-version parity — soft-warn on staged-binary mismatch
status: approved
---

# Build-version parity — design

## Context

Vox Axis (the GUI, `vox-gui`) auto-resolves a separate `vox-orchestrator-d` daemon binary at startup (`crates/vox-gui/src/commands/daemon.rs`, `resolve_or_stage_daemon`/`resolve_managed_binary_path` from `vox-cli-core`). Investigation confirmed the concrete, real mechanism for exactly the "one side updated, other wasn't" bug class the user described:

- `daemon.rs` computes `target_sibling` — the daemon binary expected to sit alongside the *currently running GUI binary*. When found, it's staged into `~/.vox/bin`, and re-staged whenever its mtime is newer than what's already there (`process_supervision.rs`'s `stage_binary`) — this path self-heals correctly in the normal dev loop (rebuild both from the same checkout, launch from `target/debug`).
- **The gap**: if `target_sibling` doesn't exist (GUI launched from an installed/packaged location with no co-located daemon binary, or only the GUI was rebuilt/repackaged), resolution silently falls through to whatever is *already* sitting in `~/.vox/bin` or on `PATH` — with zero freshness or version check. An arbitrarily old staged daemon can pair with a brand-new GUI build, silently.
- Both crates already inherit one shared workspace version (`version.workspace = true` in both `Cargo.toml`s, resolving to root `Cargo.toml`'s `[workspace.package]`), and their RPC contract types are compile-enforced shared-source (`vox-foundation`'s `DispatchRequest`/`orch_daemon_method`, cross-checked against a JSON schema in an existing test) — so *shape* drift within one checkout is already impossible. The gap is purely at the *binary-pairing* level, not the source level.
- The daemon's ping response (`{"ok": true, "repository_id", "protocol": "vox.orchestrator_daemon/v1"}`) carries a fixed protocol string but no version field, and the GUI never inspects it beyond a raw liveness check.
- A ready-made, currently-unused building block already exists: `process_supervision.rs`'s `probe_binary_version()` (runs `--version` on a resolved binary), today only wired into an unrelated sidecar path (`vox-cli`'s `openclaw.rs`), never called from the daemon-ensure path.
- No CI job builds/tests `vox-gui` and `vox-orchestrator-d` together or checks their versions match; no existing documentation (`AGENTS.md`, `where-things-live.md`) covers GUI↔orchestrator version coupling.

Per the user's approved choice: a mismatch should be a **soft warning**, not a hard block — the app stays functional, but the risk is visibly surfaced.

## Approach

### 1. Daemon reports its real version in the ping response

The daemon's ping handler (`crates/vox-orchestrator/src/orch_daemon/mod.rs`, the `{"ok": true, "repository_id", "protocol"}` response) gains a `version` field — the running daemon binary's own workspace version (available at compile time via `env!("CARGO_PKG_VERSION")` or equivalent, already resolving to the shared workspace version per the Context section). This is the most direct, lowest-friction source of truth: it reflects exactly which binary answered, regardless of how it got resolved/staged.

### 2. GUI compares the daemon's reported version against its own, warns on mismatch

`daemon.rs`'s `ping()` call site (already parsing the JSON response) reads the new `version` field and compares it against the GUI's own compile-time version (`env!("CARGO_PKG_VERSION")`). On mismatch, instead of silently proceeding: surface a visible, dismissible warning through the same channel the existing offline/reconnecting state already uses (`useOrchestratorStatus.ts`'s status plumbing → a banner component, following the precedent of the existing `BackendBanner`/offline-state UI rather than inventing a new notification mechanism) — naming both versions explicitly (e.g. "GUI v0.6.0 / daemon v0.5.9 — restart the daemon to update") so the message is actionable, not just alarming. The app continues to function normally otherwise (soft warning, per the approved choice) — this is a visibility fix, not a new failure mode.

### 3. Wire `probe_binary_version()` into the fallback resolution path, as a second layer

Independent of the ping-based check (§1–2, which only fires *after* a daemon is already running and reachable), `resolve_or_stage_daemon`'s fallback branch (the one identified as the concrete bug mechanism — falling through to an old `~/.vox/bin` binary with zero check) gets `probe_binary_version()` wired in: when falling through to an already-staged binary (not a freshly-found sibling), probe its version and log/surface the same kind of soft warning *before* even attempting to launch it, giving the earliest possible signal — this catches the case where the stale binary might not even be launchable or might fail differently than a live version-mismatched-but-running daemon would.

### 4. CI job: build both together, assert reported version matches expected

A new CI job (coordinated with, not duplicating, the CR-U6 smoke-test CI job from the sibling `orchestrator-launch-v1-readiness` spec — ideally the *same* job runs both, since both need "build vox-gui + vox-orchestrator-d from the same commit" as a precondition) builds both binaries, launches the daemon, pings it, and asserts the reported `version` field matches the workspace version at that commit — a real, automated guard against ever *shipping* a version-reporting regression (e.g. someone hardcoding a stale version string), not just a guard against runtime staleness (which is what §1–3 handle for the already-deployed case).

## What this does not include

- No change to how the daemon binary is staged/resolved beyond adding the version probe (§3) — the staging mechanism itself (mtime-based re-staging when a sibling exists) is not being redesigned.
- No automatic replacement/deletion of a stale staged binary — per the soft-warning choice, the fix is visibility, not automatic remediation; a user or separate tooling decides what to do with the warning.
- No independent versioning scheme for the two crates (they already share one workspace version, which is sufficient for this design — introducing independent versioning would be a much larger, unrelated change with no clear benefit here).
- No protocol-level version negotiation/backward-compatibility layer — this only detects and warns about a mismatch, it doesn't attempt to make two mismatched versions interoperate.

## Testing

Unit test for the ping-response version comparison logic (mismatched/matched/missing-field cases). Integration test extending `gui_relaunch_smoke.rs`'s existing pattern: stage a daemon binary with a deliberately different reported version, confirm the GUI surfaces the warning without failing to connect. The CI job from §4 is itself the primary regression guard for "did we ship a version-reporting bug" — verified during implementation by deliberately breaking the version string once and confirming the job fails, then fixing it and confirming it passes.
