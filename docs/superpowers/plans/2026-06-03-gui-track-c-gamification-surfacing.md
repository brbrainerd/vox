# GUI Track C — Gamification Surfacing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the mature `vox-gamify` backend visible in the Tauri GUI — a typed profile/XP HUD, persisted notifications lit up in the existing banner, ack wired to a typed command, and a gamify toggle/mode exposed in Settings — all gated by the existing opt-out config so it stays optional.

**Architecture:** Add a `crates/vox-gui/src/commands/gamify.rs` module exposing typed Tauri commands that connect to the gamify DB the exact way the CLI does (`vox_db::Codex::connect(DbConfig::resolve_for_mesh())` + `vox_gamify::db::*`). Fill the hardcoded-empty `alerts` vector in `orchestrator.rs` from persisted notifications so the existing `LudusBanner` pipeline lights up. Replace `GamifyView`'s raw text dump with a typed HUD + notifications list. Add a Settings gamify section backed by `vox_gamify::config_gate::load_disk()` + `cfg.save()`.

**Tech Stack:** Rust (`tauri`, `vox-db` (`Codex`), `vox-gamify`, `vox-config`), React 18 + TS + Vite + Tailwind, Vitest for the pure formatting helper.

---

## File Structure

- Create `crates/vox-gui/src/commands/gamify.rs` — all new typed Tauri commands + pure mapping helpers + unit tests.
- Modify `crates/vox-gui/src/commands/mod.rs` — declare `pub mod gamify;`.
- Modify `crates/vox-gui/src/main.rs:62-102` — register the new commands.
- Modify `crates/vox-gui/src/commands/orchestrator.rs:231-243` — fill `alerts` from notifications.
- Modify `crates/vox-gui/Cargo.toml` — add `vox-gamify`, `vox-config` deps.
- Create `crates/vox-gui/ui/src/lib/ludus.ts` — pure formatting helper.
- Create `crates/vox-gui/ui/src/lib/ludus.test.ts` — Vitest for the helper.
- Create `crates/vox-gui/ui/src/components/surfaces/Gamify/LudusHud.tsx` — the XP/profile widget.
- Modify `crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx` — typed HUD + notifications list.
- Modify `crates/vox-gui/ui/src/App.tsx:494-498` — ack via typed command.
- Modify `crates/vox-gui/ui/src/components/surfaces/Dashboard/LudusBanner.tsx` — add `error` styling.
- Modify `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` — gamify section.

---

## Task 1: `get_ludus_profile` typed Tauri command

**Files:**
- Create: `crates/vox-gui/src/commands/gamify.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`
- Modify: `crates/vox-gui/Cargo.toml`
- Modify: `crates/vox-gui/src/main.rs`

- [ ] **Step 1: Add crate deps**

In `crates/vox-gui/Cargo.toml`, under `[dependencies]`, add (match the workspace style used by sibling entries — path deps with `workspace = true` where applicable; mirror how `vox-db` is declared there):

```toml
vox-gamify = { path = "../vox-gamify" }
vox-config = { path = "../vox-config" }
```

- [ ] **Step 2: Write the failing unit test + the command module**

Create `crates/vox-gui/src/commands/gamify.rs`:

```rust
//! Typed gamification (Ludus) Tauri commands. These mirror the CLI's gamify DB
//! access exactly: connect via `Codex::connect(DbConfig::resolve_for_mesh())`
//! and call the `vox_gamify::db` API. See `crates/vox-cli/src/commands/extras/ludus/`.

use vox_gamify::notifications::{Notification, NotificationType};
use vox_gamify::profile::LudusProfile;

#[derive(Debug, serde::Serialize)]
pub struct LudusProfileDto {
    pub user_id: String,
    pub level: u64,
    pub xp: u64,
    pub xp_to_next_level: u64,
    pub xp_progress: f64,
    pub total_xp_earned: u64,
    pub crystals: u64,
    pub lumens: i64,
    pub energy: u64,
    pub max_energy: u64,
    pub current_streak: u64,
    pub prestige_level: u32,
    pub title: String,
    pub full_title: String,
    pub trust_tier: String,
}

impl LudusProfileDto {
    fn from_profile(p: &LudusProfile) -> Self {
        Self {
            user_id: p.user_id.clone(),
            level: p.level,
            xp: p.xp,
            xp_to_next_level: p.xp_to_next_level(),
            xp_progress: p.xp_progress(),
            total_xp_earned: p.total_xp_earned,
            crystals: p.crystals,
            lumens: p.lumens,
            energy: p.energy,
            max_energy: p.max_energy,
            current_streak: p.streak.current_streak as u64,
            prestige_level: p.prestige_level,
            title: p.title(),
            full_title: p.full_title(),
            trust_tier: format!("{:?}", p.trust_tier),
        }
    }
}

/// Map a notification kind to a banner/toast severity (`ok`/`warn`/`info`).
pub(crate) fn notification_level(t: &NotificationType) -> &'static str {
    match t {
        NotificationType::LevelUp
        | NotificationType::AchievementUnlocked
        | NotificationType::QuestCompleted
        | NotificationType::ChallengeCompleted
        | NotificationType::BattleWon
        | NotificationType::StreakContinued => "ok",
        NotificationType::StreakLost | NotificationType::BattleLost => "warn",
        _ => "info",
    }
}

#[derive(Debug, serde::Serialize)]
pub struct LudusNotificationDto {
    pub id: String,
    pub level: String,
    pub title: String,
    pub message: String,
    pub created_at: i64,
    pub kind: String,
}

impl LudusNotificationDto {
    fn from_notification(n: &Notification) -> Self {
        Self {
            id: n.id.clone(),
            level: notification_level(&n.notification_type).to_string(),
            title: n.title.clone(),
            message: n.message.clone(),
            created_at: n.created_at,
            kind: format!("{:?}", n.notification_type),
        }
    }
}

async fn open_gamify_db() -> Result<vox_db::Codex, String> {
    let config = vox_db::DbConfig::resolve_for_mesh().map_err(|e| e.to_string())?;
    vox_db::Codex::connect(config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ludus_profile() -> Result<LudusProfileDto, String> {
    let db = open_gamify_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let mut profile = vox_gamify::db::get_profile(&db, &user_id)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| LudusProfile::new_default(&user_id));
    profile.regen_energy();
    Ok(LudusProfileDto::from_profile(&profile))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_maps_core_fields_and_bounds_progress() {
        let mut p = LudusProfile::new_default("u1");
        p.level = 3;
        p.xp = 200;
        p.crystals = 42;
        let dto = LudusProfileDto::from_profile(&p);
        assert_eq!(dto.user_id, "u1");
        assert_eq!(dto.level, 3);
        assert_eq!(dto.crystals, 42);
        assert!(dto.xp_progress >= 0.0 && dto.xp_progress <= 1.0);
        assert!(!dto.title.is_empty());
    }

    #[test]
    fn notification_level_maps_severity() {
        assert_eq!(notification_level(&NotificationType::LevelUp), "ok");
        assert_eq!(notification_level(&NotificationType::StreakLost), "warn");
        assert_eq!(notification_level(&NotificationType::CompanionStatus), "info");
    }
}
```

- [ ] **Step 3: Declare the module**

In `crates/vox-gui/src/commands/mod.rs`, add (next to the other `pub mod` lines):

```rust
pub mod gamify;
```

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `cargo test -p vox-gui gamify`
Expected: 2 tests PASS. If `p.streak.current_streak` has a different integer type, the `as u64` cast still compiles; if a field name differs, fix against `crates/vox-gamify/src/profile.rs:176-220`.

- [ ] **Step 5: Register the command**

In `crates/vox-gui/src/main.rs`, add to the `tauri::generate_handler![ ... ]` list (after the `commands::secrets::remove_secret,` line):

```rust
            commands::gamify::get_ludus_profile,
```

- [ ] **Step 6: Build + commit**

```bash
cargo build -p vox-gui
git add crates/vox-gui/Cargo.toml crates/vox-gui/src/commands/gamify.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs
git commit -m "feat(gui): get_ludus_profile typed command + DTO"
```

---

## Task 2: notifications list + typed ack commands

**Files:**
- Modify: `crates/vox-gui/src/commands/gamify.rs`
- Modify: `crates/vox-gui/src/main.rs`

- [ ] **Step 1: Add the two commands**

Append to `crates/vox-gui/src/commands/gamify.rs` (before the `#[cfg(test)]` module):

```rust
#[tauri::command]
pub async fn list_ludus_notifications(limit: Option<u32>) -> Result<Vec<LudusNotificationDto>, String> {
    let db = open_gamify_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let notes = vox_gamify::db::list_unread_notifications(&db, &user_id, limit.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())?;
    Ok(notes.iter().map(LudusNotificationDto::from_notification).collect())
}

#[tauri::command]
pub async fn ack_ludus_notification(notification_id: String) -> Result<(), String> {
    let db = open_gamify_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    vox_gamify::db::mark_notification_read_for_user(&db, &user_id, &notification_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: Register both commands**

In `crates/vox-gui/src/main.rs`, add to the handler list after `get_ludus_profile`:

```rust
            commands::gamify::list_ludus_notifications,
            commands::gamify::ack_ludus_notification,
```

- [ ] **Step 3: Build + commit**

```bash
cargo build -p vox-gui
git add crates/vox-gui/src/commands/gamify.rs crates/vox-gui/src/main.rs
git commit -m "feat(gui): list_ludus_notifications + ack_ludus_notification commands"
```

---

## Task 3: Light the banner — fill `alerts` from notifications

**Files:**
- Modify: `crates/vox-gui/src/commands/gamify.rs` (add `fetch_gamify_alerts`)
- Modify: `crates/vox-gui/src/commands/orchestrator.rs:231-243`

- [ ] **Step 1: Add a best-effort alert fetcher**

Append to `crates/vox-gui/src/commands/gamify.rs` (before tests). It returns the exact JSON shape the frontend `mapAlert` expects (`{id, level, title, body}`), and never fails the status call:

```rust
/// Best-effort: map unread gamify notifications to the GUI alert JSON shape
/// (`{id, level, title, body}`) consumed by `LudusBanner`. Returns empty on any error.
pub async fn fetch_gamify_alerts() -> Vec<serde_json::Value> {
    let Ok(config) = vox_db::DbConfig::resolve_for_mesh() else {
        return Vec::new();
    };
    let Ok(db) = vox_db::Codex::connect(config).await else {
        return Vec::new();
    };
    let user_id = vox_gamify::db::canonical_user_id();
    let notes = match vox_gamify::db::list_unread_notifications(&db, &user_id, 10).await {
        Ok(n) => n,
        Err(_) => return Vec::new(),
    };
    notes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "level": notification_level(&n.notification_type),
                "title": n.title,
                "body": n.message,
            })
        })
        .collect()
}
```

- [ ] **Step 2: Fill `alerts` in both status commands**

In `crates/vox-gui/src/commands/orchestrator.rs`, replace the `get_orchestrator_status` command (lines 231-235) with:

```rust
#[tauri::command]
pub async fn get_orchestrator_status() -> Result<serde_json::Value, String> {
    let status = daemon_status().await?;
    let mut gui = to_gui_status(status);
    gui.alerts = crate::commands::gamify::fetch_gamify_alerts().await;
    serde_json::to_value(gui).map_err(|e| e.to_string())
}
```

Then in `get_orchestrator_status_bin` (lines 237-243), set `gui.alerts = crate::commands::gamify::fetch_gamify_alerts().await;` on the `GuiOrchestratorStatus` value *before* it is MessagePack-encoded (the existing function builds the value via `to_gui_status`; bind it to a `let mut gui = ...`, assign alerts, then encode `gui`).

> `to_gui_status` keeps `alerts: Vec::new()` (orchestrator.rs:222) — the fill happens at the command boundary so the pure mapper stays test-friendly.

- [ ] **Step 3: Build + verify the existing alert pipeline now has a data source**

Run: `cargo build -p vox-gui`
Expected: clean build. The frontend pipeline (`App.tsx` `mapAlert` → `Dashboard` → `LudusBanner`) is unchanged and now receives data.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/src/commands/gamify.rs crates/vox-gui/src/commands/orchestrator.rs
git commit -m "feat(gui): surface persisted gamify notifications in the Ludus banner"
```

---

## Task 4: Pure XP-bar helper + LudusHud component

**Files:**
- Create: `crates/vox-gui/ui/src/lib/ludus.ts`
- Create: `crates/vox-gui/ui/src/lib/ludus.test.ts`
- Create: `crates/vox-gui/ui/src/components/surfaces/Gamify/LudusHud.tsx`

- [ ] **Step 1: Write the failing Vitest for the pure helper**

Create `crates/vox-gui/ui/src/lib/ludus.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { xpBarPct, LudusProfile } from './ludus';

describe('xpBarPct', () => {
  it('clamps to 0..100 and renders a percent string', () => {
    expect(xpBarPct(0)).toBe('0%');
    expect(xpBarPct(0.42)).toBe('42%');
    expect(xpBarPct(1)).toBe('100%');
    expect(xpBarPct(1.5)).toBe('100%');
    expect(xpBarPct(-0.2)).toBe('0%');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `pnpm --dir crates/vox-gui/ui test -- ludus`
Expected: FAIL with "Cannot find module './ludus'".

- [ ] **Step 3: Write the helper + the shared type**

Create `crates/vox-gui/ui/src/lib/ludus.ts`:

```ts
export interface LudusProfile {
  user_id: string;
  level: number;
  xp: number;
  xp_to_next_level: number;
  xp_progress: number;
  total_xp_earned: number;
  crystals: number;
  lumens: number;
  energy: number;
  max_energy: number;
  current_streak: number;
  prestige_level: number;
  title: string;
  full_title: string;
  trust_tier: string;
}

/** Clamp a 0..1 progress fraction to a `NN%` width string. */
export function xpBarPct(progress: number): string {
  const clamped = Math.max(0, Math.min(1, progress));
  return `${Math.round(clamped * 100)}%`;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --dir crates/vox-gui/ui test -- ludus`
Expected: PASS.

- [ ] **Step 5: Write the LudusHud component**

Create `crates/vox-gui/ui/src/components/surfaces/Gamify/LudusHud.tsx`:

```tsx
import React from 'react';
import { LudusProfile, xpBarPct } from '../../../lib/ludus';

export function LudusHud({ profile }: { profile: LudusProfile }) {
  return (
    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4">
      <div className="flex items-baseline justify-between">
        <div className="font-display text-sm tracking-wider text-brass uppercase">{profile.full_title}</div>
        <div className="font-mono text-[11px] text-zinc-500">Lv {profile.level} · prestige {profile.prestige_level}</div>
      </div>
      <div className="mt-3 h-2 overflow-hidden rounded-full bg-white/[0.05]">
        <div className="h-full rounded-full bg-brass/70 transition-all" style={{ width: xpBarPct(profile.xp_progress) }} />
      </div>
      <div className="mt-1 flex justify-between font-mono text-[10px] text-zinc-500">
        <span>{profile.xp} XP</span>
        <span>{profile.xp_to_next_level} to next</span>
      </div>
      <div className="mt-3 grid grid-cols-2 gap-2 text-[11px] sm:grid-cols-4">
        <Stat label="Crystals" value={`${profile.crystals} 💎`} />
        <Stat label="Lumens" value={`${profile.lumens}`} />
        <Stat label="Energy" value={`${profile.energy}/${profile.max_energy}`} />
        <Stat label="Streak" value={`${profile.current_streak} 🔥`} />
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-white/[0.02] px-2 py-1.5 ring-1 ring-white/5">
      <div className="text-[9px] uppercase tracking-wide text-zinc-500">{label}</div>
      <div className="font-mono text-zinc-200">{value}</div>
    </div>
  );
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/lib/ludus.ts crates/vox-gui/ui/src/lib/ludus.test.ts crates/vox-gui/ui/src/components/surfaces/Gamify/LudusHud.tsx
git commit -m "feat(gui): LudusHud XP widget + pure xpBarPct helper (vitest)"
```

---

## Task 5: Rebuild GamifyView around the typed profile + notifications

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx` (full replace)

- [ ] **Step 1: Replace the shell-out dump with typed data**

Replace the entire contents of `crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx`:

```tsx
import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { LudusProfile } from '../../../lib/ludus';
import { LudusHud } from './LudusHud';

interface GamifyViewProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

interface LudusNotification {
  id: string;
  level: 'ok' | 'warn' | 'info';
  title: string;
  message: string;
  created_at: number;
  kind: string;
}

export function GamifyView({ pushToast }: GamifyViewProps) {
  const [profile, setProfile] = useState<LudusProfile | null>(null);
  const [notes, setNotes] = useState<LudusNotification[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [p, n] = await Promise.all([
        invoke<LudusProfile>('get_ludus_profile'),
        invoke<LudusNotification[]>('list_ludus_notifications', { limit: 20 }),
      ]);
      setProfile(p);
      setNotes(n);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Ludus load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 15000);
    return () => clearInterval(id);
  }, [refresh]);

  const ack = async (id: string) => {
    try {
      await invoke('ack_ludus_notification', { notificationId: id });
      setNotes(curr => curr.filter(x => x.id !== id));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Ack failed', body: String(err) });
    }
  };

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Gamification</h2>
        <button onClick={refresh} disabled={loading}
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs hover:bg-white/[0.06]">
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>

      {profile ? <LudusHud profile={profile} /> : (
        <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4 text-sm text-zinc-500">No profile yet.</div>
      )}

      <div>
        <div className="mb-2 font-display text-[12px] uppercase tracking-wide text-zinc-400">Notifications</div>
        {notes.length === 0 ? (
          <div className="rounded-lg border border-white/5 bg-white/[0.02] p-3 text-[12px] text-zinc-500">No unread notifications.</div>
        ) : (
          <ul className="space-y-2">
            {notes.map(n => (
              <li key={n.id} className="flex items-start justify-between gap-3 rounded-lg border border-white/10 bg-white/[0.02] p-3">
                <div className="min-w-0">
                  <div className="text-[12px] text-zinc-200">{n.title}</div>
                  <div className="text-[11px] text-zinc-500">{n.message}</div>
                </div>
                <button onClick={() => ack(n.id)} className="shrink-0 rounded-md border border-white/5 bg-white/[0.03] px-2 py-1 text-[10px] text-zinc-400 hover:text-zinc-100">Ack</button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Build + commit**

```bash
pnpm --dir crates/vox-gui/ui build
git add crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx
git commit -m "feat(gui): GamifyView shows typed profile HUD + notifications with ack"
```

---

## Task 6: Wire dashboard ack to the typed command + fix banner `error` styling

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx:494-498`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/LudusBanner.tsx`

- [ ] **Step 1: Switch `handleAckAlert` to the typed Tauri command**

In `crates/vox-gui/ui/src/App.tsx`, replace the body of `handleAckAlert` (lines 494-498). Ensure `invoke` is imported at the top of the file (it already is, used elsewhere):

```tsx
  const handleAckAlert = useCallback(async (note: LudusAlert) => {
    setData(prev => ({ ...prev, alerts: prev.alerts.filter(x => x.id !== note.id) }));
    await invoke('ack_ludus_notification', { notificationId: note.id })
      .catch((err) => pushToast({ tone: 'warn', title: 'Alert ack failed', body: String(err) }));
  }, [pushToast]);
```

- [ ] **Step 2: Add `error`-level styling to the banner**

In `crates/vox-gui/ui/src/components/surfaces/Dashboard/LudusBanner.tsx`, add an `error` entry to `stylingMap` (after the `info` entry):

```tsx
    error:  { ring: "ring-rose-400/25", bg: "bg-gradient-to-br from-rose-500/[0.08] via-rose-500/[0.02] to-transparent", text: "text-rose-300", icon: <Icon.alert className="size-4"/> },
```

- [ ] **Step 3: Build + commit**

```bash
pnpm --dir crates/vox-gui/ui build
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/Dashboard/LudusBanner.tsx
git commit -m "refactor(gui): ack alerts via typed command; banner error styling"
```

---

## Task 7: Settings gamify section (toggle + mode)

**Files:**
- Modify: `crates/vox-gui/src/commands/gamify.rs` (settings get/set)
- Modify: `crates/vox-gui/src/main.rs` (register)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`

- [ ] **Step 1: Add the settings commands (mirrors `extras/ludus/profile.rs:478-560`)**

Append to `crates/vox-gui/src/commands/gamify.rs` (before tests):

```rust
#[derive(Debug, serde::Serialize)]
pub struct GamifySettingsDto {
    pub enabled: bool,
    pub mode: String,
}

#[tauri::command]
pub async fn get_gamify_settings() -> Result<GamifySettingsDto, String> {
    let cfg = vox_gamify::config_gate::load_disk();
    Ok(GamifySettingsDto {
        enabled: cfg.gamify_enabled,
        mode: cfg.gamify_mode.as_config_str().to_string(),
    })
}

#[tauri::command]
pub async fn set_gamify_settings(enabled: bool, mode: String) -> Result<(), String> {
    let mut cfg = vox_gamify::config_gate::load_disk();
    cfg.gamify_enabled = enabled;
    cfg.gamify_mode = match mode.to_lowercase().as_str() {
        "serious" => vox_config::GamifyMode::Serious,
        "learning" => vox_config::GamifyMode::Learning,
        _ => vox_config::GamifyMode::Balanced,
    };
    cfg.save().map_err(|e| format!("save config: {e}"))?;
    Ok(())
}
```

- [ ] **Step 2: Register both**

In `crates/vox-gui/src/main.rs`, add to the handler list:

```rust
            commands::gamify::get_gamify_settings,
            commands::gamify::set_gamify_settings,
```

- [ ] **Step 3: Add a Gamify section to SettingsView**

In `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`:

(a) Add `'gamify'` to the `SECTIONS` array (after `'theme'`):

```tsx
  { key: 'gamify', label: 'Gamification' },
```

(b) Add state + load + render. Near the top of the component body add state:

```tsx
  const [gamify, setGamify] = useState<{ enabled: boolean; mode: string }>({ enabled: true, mode: 'balanced' });
```

In the existing hydrate `useEffect`, add a fetch:

```tsx
    invoke<{ enabled: boolean; mode: string }>('get_gamify_settings').then(setGamify).catch(() => {});
```

Add a handler:

```tsx
  const updateGamify = async (patch: Partial<{ enabled: boolean; mode: string }>) => {
    const next = { ...gamify, ...patch };
    setGamify(next);
    try {
      await invoke('set_gamify_settings', { enabled: next.enabled, mode: next.mode });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Gamify save failed', body: String(err) });
    }
  };
```

(c) In the section renderer, add the `gamify` panel (mirror how the existing sections render rows):

```tsx
      {section === 'gamify' && (
        <div className="space-y-3">
          <label className="flex items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] p-3 text-sm">
            <span>Gamification enabled</span>
            <input type="checkbox" checked={gamify.enabled} onChange={e => updateGamify({ enabled: e.target.checked })} />
          </label>
          <label className="flex items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] p-3 text-sm">
            <span>Mode</span>
            <select value={gamify.mode} disabled={!gamify.enabled}
              onChange={e => updateGamify({ mode: e.target.value })}
              className="rounded bg-black/40 px-2 py-1 text-zinc-200">
              <option value="balanced">Balanced</option>
              <option value="serious">Serious (silent)</option>
              <option value="learning">Learning</option>
            </select>
          </label>
          <p className="text-[11px] text-zinc-500">Serious mode keeps rewards active but hides overlays and hints.</p>
        </div>
      )}
```

- [ ] **Step 4: Build + commit**

```bash
cargo build -p vox-gui
pnpm --dir crates/vox-gui/ui build
git add crates/vox-gui/src/commands/gamify.rs crates/vox-gui/src/main.rs crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx
git commit -m "feat(gui): Settings gamify toggle + mode (Balanced/Serious/Learning)"
```

---

## Task 8: Full verification + regenerate coverage report

**Files:**
- Modify: `contracts/reports/gui-surface-coverage.v1.json` (regenerated — new IPC commands change the report)

- [ ] **Step 1: Architecture check**

Run: `cargo run -p vox-arch-check`
Expected: clean. If adding `vox-gamify`/`vox-config` to `vox-gui` trips a layer rule, add a `[[known_inversions]]` row with a one-line reason in `docs/src/architecture/layers.toml` (vox-gui is the top operator surface and already depends on heavy crates like `vox-orchestrator-mcp`).

- [ ] **Step 2: Regenerate the surface-coverage report (new IPC commands shift it)**

The new `commands::gamify::*` handlers are picked up by `gui_surface_coverage`'s IPC regex, so the committed report drifts. Regenerate:

Run: `cargo run -p vox-cli -- ci gui-surface-coverage --write`
Run: `cargo run -p vox-cli -- ci gui-surface-coverage`
Expected: second run passes.

- [ ] **Step 3: Rust tests + frontend build + tests**

Run: `cargo test -p vox-gui gamify`
Run: `pnpm --dir crates/vox-gui/ui build`
Run: `pnpm --dir crates/vox-gui/ui test`
Expected: all pass.

- [ ] **Step 4: Commit the regenerated report**

```bash
git add contracts/reports/gui-surface-coverage.v1.json docs/src/architecture/layers.toml
git commit -m "chore(gui): regenerate surface-coverage report after gamify commands"
```

---

## Self-Review

- **Spec coverage:** typed profile HUD (Tasks 1, 4, 5); persisted notifications in the banner (Tasks 2, 3); typed ack (Tasks 2, 6); achievement/level-up visibility (banner via Task 3, list via Task 5); Settings toggle/mode keeping gamification optional (Task 7). The audit's `vox_gamify_notification_ack`-already-works finding is honored: Task 6 *replaces* the MCP-bridge call with a typed command rather than implementing a "missing" backend.
- **Type consistency:** `LudusProfile` TS interface (ludus.ts) field names match `LudusProfileDto` serde output (snake_case preserved across IPC). `LudusNotification.level` (`ok|warn|info`) matches `notification_level`'s outputs. Ack arg `notificationId` (JS camelCase) → `notification_id` (Rust) per the codebase's Tauri convention (`set_active_model`/`modelId`).
- **No placeholders:** every code step is complete; commands have expected output.

## Deferred (out of scope, intentionally)

- Leaderboards, companions (SVG sprites), quests, and battles surfaces — data exists; each is its own surface and should be classified into the Track A registry when built.
- Per-event `event_config` reward tuning UI (the audit notes it also lacks a clear CLI surface — a backend gap to resolve first).
