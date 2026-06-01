# SP-3: Ludus Bus Wiring — Design

**Date:** 2026-06-01
**Status:** Draft (spec for review)
**Umbrella:** [`2026-06-01-cli-gui-hybrid-spine-design.md`](2026-06-01-cli-gui-hybrid-spine-design.md) (Unit 3)
**Depends on:** SP-1, SP-2 — landed.

## Scope decision (refinement of the umbrella SP-3)

The umbrella SP-3 had three parts. Ground-truth exploration collapsed two of them:

- **"Register the orphaned `ludus.rs` Tauri command" — not needed.** `crates/vox-gui/src/commands/ludus.rs`
  no longer exists, and `GamifyView.tsx` already shells the real `vox ludus hud` / `vox ludus profile`
  via `execute_command`. The Gamify surface is already live against the CLI; no new Tauri command and no
  `vox-gamify` dependency in `vox-gui` are required.
- **"Make GamifyView read a live projection" — already true.** It renders real `vox ludus profile` stdout.

What remains is the **actual gap**: standard commands don't emit reward events.

- `vox_gamify::event_router::route_event(db, user_id, event_json)` is fully implemented and self-gates
  on `config_gate::is_enabled()`.
- A purpose-built fire-and-forget shim already exists:
  `vox_cli_core::gamify_shim::record_cli_event_fire_and_forget(event_type, success, capability_id, command_path)`
  — it `tokio::spawn`s, opens its own `VoxDb`, records a `cli_command` behavior event, and calls
  `route_event` only when gamification is enabled. It never blocks and never affects command exit.
- The reward policy (`vox_gamify::reward_policy::base_reward`) **already defines CLI-command event types**:
  `build_completed` (25xp), `build_failed` (5xp, "struggle XP"), `check_completed`, `check_failed`,
  `test_pass`, `test_fail`, `fmt_completed`, `bundle_completed`, plus mastery types like `build_clean`.
- **Nobody calls the shim from the main dispatch path.** Running `vox build` earns nothing today.

**SP-3 is therefore: emit a reward event from the CLI dispatch seam when a core dev-loop command
completes.** Because the GUI shells the `vox` sidecar, this covers GUI-driven *and* direct-CLI actions in
one place — realizing the umbrella's "gamification is structural" goal without touching the GUI.

## Goal

Running a core dev-loop command (`build` / `check` / `test` / `fmt` / `bundle`) emits the matching reward
event through the existing shim, so the developer earns XP/crystals (and the Gamify surface reflects it),
with zero impact on command behavior, exit code, or latency, and a no-op when gamification is disabled.

## The seam

`crates/vox-cli/src/cli_dispatch/lanes.rs::run_fabrica_cmd` is the single function every top-level
`vox build/check/test/run/dev/bundle/compile/fmt` flows through (after the fabrica shim maps them from
`Cli` variants). Today it runs each arm with `?`, which early-returns on error.

**Change:** split it into a pure inner runner plus a thin wrapper that records success/failure:

```rust
pub(crate) async fn run_fabrica_cmd(cmd: latin_cmd::FabricaCmd) -> anyhow::Result<()> {
    // Read the reward mapping BEFORE `cmd` is moved into the inner runner.
    let events = fabrica_reward_events(&cmd);
    let result = run_fabrica_cmd_inner(cmd).await;
    if let Some(ev) = events {
        let success = result.is_ok();
        if let Some(event_type) = if success { Some(ev.success) } else { ev.failure } {
            vox_cli_core::gamify_shim::record_cli_event_fire_and_forget(
                event_type,
                success,
                Some(ev.capability_id),
                Some(ev.command_path),
            );
        }
    }
    result
}
```

`run_fabrica_cmd_inner` is the current match body verbatim (renamed). The wrapper adds no `?`-free
restructuring inside arms — it only wraps the whole call.

## Event mapping

A pure helper maps each fabrica variant to its reward events. Only variants with a reward type defined in
`base_reward` are wired; the rest return `None` (no hollow events):

```rust
struct FabricaRewardEvents {
    success: &'static str,
    failure: Option<&'static str>,
    capability_id: &'static str,
    command_path: &'static str,
}

fn fabrica_reward_events(cmd: &latin_cmd::FabricaCmd) -> Option<FabricaRewardEvents> {
    use latin_cmd::FabricaCmd;
    Some(match cmd {
        FabricaCmd::Build(_)  => FabricaRewardEvents { success: "build_completed", failure: Some("build_failed"), capability_id: "cli.build",  command_path: "build" },
        FabricaCmd::Check(_)  => FabricaRewardEvents { success: "check_completed", failure: Some("check_failed"), capability_id: "cli.check",  command_path: "check" },
        FabricaCmd::Test(_)   => FabricaRewardEvents { success: "test_pass",       failure: Some("test_fail"),    capability_id: "cli.test",   command_path: "test" },
        FabricaCmd::Bundle(_) => FabricaRewardEvents { success: "bundle_completed", failure: None,                capability_id: "cli.bundle", command_path: "bundle" },
        FabricaCmd::Fmt(_)    => FabricaRewardEvents { success: "fmt_completed",    failure: None,                capability_id: "cli.fmt",    command_path: "fmt" },
        // Run / Dev / Compile / Script: no reward type defined yet — deferred (see Non-goals).
        _ => return None,
    })
}
```

| Command | success event | failure event | base reward (xp, crystals) |
| --- | --- | --- | --- |
| `build` | `build_completed` | `build_failed` | (25,5) / (5,0) |
| `check` | `check_completed` | `check_failed` | (15,3) / (3,0) |
| `test` | `test_pass` | `test_fail` | (55,10) / (10,0) |
| `bundle` | `bundle_completed` | — | (50,10) |
| `fmt` | `fmt_completed` | — | (2,0) |

All five event types already exist in `base_reward`, so each emission earns a real, non-zero reward
(except the deliberate "struggle XP" failure cases the policy itself defines).

## Why this layer, not the GUI execute path

- The GUI's `execute_command` shells `vox <args>` through the sidecar, so the sidecar's own dispatch is
  the real single execute path. Wiring there covers GUI and CLI uniformly.
- Keeps `vox-gui` free of a `vox-gamify` dependency and free of gamification logic.
- The shim already handles DB acquisition, the config gate, the emergency off-switch, and async spawning —
  we add one call, not a subsystem.

## Error handling / safety

- Emission is `tokio::spawn` fire-and-forget; a DB error or disabled gamification is a silent no-op
  (the shim returns early). It can never change the command's `Result`, exit code, or output.
- The reward event reflects the command's actual outcome (`success = result.is_ok()`).
- No new panics, no new blocking I/O on the command's critical path.

## Testing

- **Unit (pure, CI-friendly):** `fabrica_reward_events` mapping — assert Build → `build_completed` /
  `build_failed` / `cli.build` / `build`, Bundle/Fmt have `failure: None`, and Run/Dev/Compile → `None`.
  This is the load-bearing logic; the shim's side effect (DB write) is environment-dependent and already
  covered by `vox-gamify` integration tests.
- **Manual/integration (documented, not CI-gated):** with `[gamify] enabled = true`, run `vox build` then
  `vox ludus profile`; XP increases. Run from the GUI Catalog panel; same effect (sidecar path).
- **Regression:** `cargo build -p vox-cli` and the existing fabrica dispatch tests stay green; command
  behavior is unchanged.

## Non-goals

- **Run / Dev / Compile / Script and non-fabrica commands** (scientia, audit, …) are **not** wired.
  They have no reward type in `base_reward`; adding new event types + reward values + the matrix doc is a
  separate slice. SP-3 wires only the commands the policy already rewards.
- No `build_clean` / mastery-tier detection (requires warning-count introspection) — `build_completed`
  is the honest success signal for now.
- No GUI changes, no new Tauri command, no `vox-gamify` dependency in `vox-gui`.
- No change to the shim or the reward policy.
