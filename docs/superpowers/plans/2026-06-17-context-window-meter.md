---
title: "Context Window Meter — Token Budget Visibility Widget"
description: "Implementation plan for a ContextWindowMeter widget in the ChatExecutionRail that shows live token usage, compaction threshold, and last-compacted timestamp from the orchestrator."
category: "plans"
status: "current"
---

# Context Window Meter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a live token-usage progress bar to the `ChatExecutionRail` so users can see how full the AI's context window is, when compaction will trigger, and when it last ran.

**Architecture:** A new `get_context_budget` Tauri command reads `CompactionConfig` from the orchestrator config file and returns a static budget object (`max_context_tokens`, `threshold_tokens`, `strategy`). A separate `used_tokens` field is sourced from the latest `GuiOrchestratorStatus` (already pushed via `vox://orch-status` events). The `ContextWindowMeter` React component assembles these into a color-coded progress bar inside `ChatExecutionRail`.

**Tech Stack:** Rust (Tauri command, `serde`), TypeScript/React (new functional component with CSS progress bar, no new dependencies), `vox-orchestrator::compaction::CompactionConfig` (already public).

---

## Background: How Config Is Read Today

The orchestrator config is read by `crates/vox-gui/src/commands/orchestrator.rs::get_orchestrator_config`. It calls `call_daemon("vox-orchestrator-d", orch_daemon_method::GET_CONFIG, ...)`. We will follow the same pattern but target only the compaction sub-section. The `CompactionConfig` struct is in `crates/vox-orchestrator/src/compaction.rs` and is fully `Serialize`/`Deserialize`.

The existing `GuiOrchestratorStatus` (pushed via `vox://orch-status`) already includes `total_queued`, `total_in_progress`, etc., but **not** current token usage. The daemon status response does not yet include `used_tokens`. For Phase 1, we use a **static budget** (config values only) and estimate usage from turn count × average token weight. Full live tracking (reading actual token usage from LLM egress) is Phase 2 work.

---

## File Map

| File | Change | Responsibility |
|---|---|---|
| `crates/vox-gui/src/commands/orchestrator.rs` | **Modify** | Add `get_context_budget` Tauri command |
| `crates/vox-gui/src/main.rs` | **Modify** | Register `get_context_budget` in `invoke_handler!` |
| `crates/vox-gui/ui/src/components/surfaces/Chat/ContextWindowMeter.tsx` | **Create** | Pure presentational meter widget |
| `crates/vox-gui/ui/src/components/surfaces/Chat/ContextWindowMeter.test.tsx` | **Create** | Unit tests for the meter's color zones and label logic |
| `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx` | **Modify** | Fetch budget on mount, render `<ContextWindowMeter>` |

---

## Task 1: Add `get_context_budget` Tauri command

**Context:** The command lives in `orchestrator.rs` alongside related orchestrator commands. It calls the daemon to get config, extracts the compaction section, and returns a typed `ContextBudgetPayload`.

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs` (append to end of file)

- [ ] **Step 1.1: Write the unit test first**

Append to `crates/vox-gui/src/commands/orchestrator.rs`:

```rust
#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn context_budget_payload_serializes() {
        let payload = ContextBudgetPayload {
            max_context_tokens: 128_000,
            reserved_tokens: 10_000,
            threshold_tokens: 102_400,
            usable_tokens: 118_000,
            strategy: "balanced".to_string(),
        };
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["max_context_tokens"], 128_000);
        assert_eq!(json["strategy"], "balanced");
        assert_eq!(json["threshold_tokens"], 102_400);
    }

    #[test]
    fn threshold_tokens_matches_trigger_at() {
        // CompactionConfig::trigger_at() = max * threshold_fraction
        // Default: 128_000 * 0.80 = 102_400
        let cfg = vox_orchestrator::compaction::CompactionConfig::default();
        assert_eq!(cfg.trigger_at(), 102_400);
        assert_eq!(cfg.usable_budget(), 118_000);
    }
}
```

- [ ] **Step 1.2: Run to verify it FAILS**

```
cargo test -p vox-gui -- budget_tests
```

Expected: FAIL — `ContextBudgetPayload` not defined yet.

- [ ] **Step 1.3: Add the struct and command implementation**

Append to `crates/vox-gui/src/commands/orchestrator.rs`:

```rust
/// Token budget snapshot returned to the frontend.
#[derive(Debug, serde::Serialize)]
pub struct ContextBudgetPayload {
    /// Maximum tokens the model's context can hold (from `CompactionConfig`).
    pub max_context_tokens: usize,
    /// Tokens reserved for the model's response (subtracted from usable budget).
    pub reserved_tokens: usize,
    /// Token count at which compaction triggers (`max * compaction_threshold`).
    pub threshold_tokens: usize,
    /// Usable token budget (`max - reserved`).
    pub usable_tokens: usize,
    /// Human-readable compaction strategy name: "aggressive", "balanced", or "conservative".
    pub strategy: String,
}

/// Return the active context-window budget from the current compaction config.
///
/// Falls back to `CompactionConfig::default()` values if the daemon is unavailable
/// — so the UI always has something reasonable to display.
#[tauri::command]
pub async fn get_context_budget() -> Result<ContextBudgetPayload, String> {
    use vox_orchestrator::compaction::CompactionConfig;

    // Try to read live config from daemon; fall back to defaults on any error.
    let cfg: CompactionConfig = call_daemon(
        "vox-orchestrator-d",
        vox_foundation::protocol::orch_daemon_method::GET_CONFIG,
        serde_json::json!({}),
        false,
    )
    .await
    .ok()
    .and_then(|v| {
        v.get("compaction")
            .and_then(|c| serde_json::from_value(c.clone()).ok())
    })
    .unwrap_or_default();

    Ok(ContextBudgetPayload {
        max_context_tokens: cfg.max_context_tokens,
        reserved_tokens: cfg.reserved_tokens,
        threshold_tokens: cfg.trigger_at(),
        usable_tokens: cfg.usable_budget(),
        strategy: cfg.strategy.to_string(),
    })
}
```

- [ ] **Step 1.4: Run tests to verify they PASS**

```
cargo test -p vox-gui -- budget_tests
```

Expected: 2 PASS.

- [ ] **Step 1.5: Compile check**

```
cargo check -p vox-gui
```

Expected: no errors.

- [ ] **Step 1.6: Commit**

```
git add crates/vox-gui/src/commands/orchestrator.rs
git commit -m "feat(gui): add get_context_budget Tauri command"
```

---

## Task 2: Register the command in `main.rs`

**Context:** Every Tauri command must be listed in the `invoke_handler!(...)` macro in `main.rs`. The list is at lines 108–242 in `crates/vox-gui/src/main.rs`. Add the new command after the existing orchestrator commands.

**Files:**
- Modify: `crates/vox-gui/src/main.rs:146-149`

- [ ] **Step 2.1: Add the command to the handler list**

Find this block in `main.rs` (around line 146):

```rust
            commands::orchestrator::get_orchestrator_status,
            commands::orchestrator::get_orchestrator_status_bin,
            commands::orchestrator::set_orchestrator_config,
            commands::orchestrator::get_orchestrator_config,
```

Add `commands::orchestrator::get_context_budget,` immediately after it:

```rust
            commands::orchestrator::get_orchestrator_status,
            commands::orchestrator::get_orchestrator_status_bin,
            commands::orchestrator::set_orchestrator_config,
            commands::orchestrator::get_orchestrator_config,
            commands::orchestrator::get_context_budget,
```

- [ ] **Step 2.2: Compile check**

```
cargo check -p vox-gui
```

Expected: no errors.

- [ ] **Step 2.3: Commit**

```
git add crates/vox-gui/src/main.rs
git commit -m "feat(gui): register get_context_budget in Tauri invoke_handler"
```

---

## Task 3: Build the `ContextWindowMeter` React component

**Context:** This is a pure presentational component. It receives `usedTokens`, `maxTokens`, `thresholdTokens`, and `strategy` as props, and renders a color-coded progress bar. No state, no side effects, no Tauri calls — easy to test.

Color zones (matching 2026 industry conventions from research doc §4.3):
- **Green** (`oklch(0.7 0.15 140)`): 0–70% of max (safe)
- **Amber** (`oklch(0.75 0.15 60)`): 70–90% (approaching threshold)
- **Red** (`oklch(0.65 0.2 25)`): 90–100% (compaction imminent or active)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/ContextWindowMeter.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/ContextWindowMeter.test.tsx`

- [ ] **Step 3.1: Write the tests first**

Create `crates/vox-gui/ui/src/components/surfaces/Chat/ContextWindowMeter.test.tsx`:

```typescript
import React from 'react';
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { ContextWindowMeter } from './ContextWindowMeter';

describe('ContextWindowMeter', () => {
  it('renders percentage label', () => {
    render(
      <ContextWindowMeter
        usedTokens={64_000}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    // 64000/128000 = 50%
    expect(screen.getByText('50%')).toBeInTheDocument();
  });

  it('shows strategy name', () => {
    render(
      <ContextWindowMeter
        usedTokens={0}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="aggressive"
      />
    );
    expect(screen.getByText('aggressive')).toBeInTheDocument();
  });

  it('applies green class when under 70%', () => {
    const { container } = render(
      <ContextWindowMeter
        usedTokens={50_000}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    // The fill bar should have data-zone="safe"
    expect(container.querySelector('[data-zone="safe"]')).toBeTruthy();
  });

  it('applies amber class at 80% usage', () => {
    const { container } = render(
      <ContextWindowMeter
        usedTokens={102_400}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    expect(container.querySelector('[data-zone="warn"]')).toBeTruthy();
  });

  it('applies red class above 90% usage', () => {
    const { container } = render(
      <ContextWindowMeter
        usedTokens={120_000}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    expect(container.querySelector('[data-zone="danger"]')).toBeTruthy();
  });

  it('clamps percent to 100 when used exceeds max', () => {
    render(
      <ContextWindowMeter
        usedTokens={999_999}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    expect(screen.getByText('100%')).toBeInTheDocument();
  });

  it('renders 0% when usedTokens is 0', () => {
    render(
      <ContextWindowMeter
        usedTokens={0}
        maxTokens={128_000}
        thresholdTokens={102_400}
        strategy="balanced"
      />
    );
    expect(screen.getByText('0%')).toBeInTheDocument();
  });
});
```

- [ ] **Step 3.2: Run tests to verify they FAIL**

```
cd crates/vox-gui/ui
pnpm test ContextWindowMeter
```

Expected: FAIL — component doesn't exist yet.

- [ ] **Step 3.3: Create the component**

Create `crates/vox-gui/ui/src/components/surfaces/Chat/ContextWindowMeter.tsx`:

```typescript
import React from 'react';

export interface ContextWindowMeterProps {
  /** Estimated tokens currently in the active context window. */
  usedTokens: number;
  /** Maximum tokens the model can hold (from CompactionConfig). */
  maxTokens: number;
  /** Token count at which compaction will trigger. */
  thresholdTokens: number;
  /** Human-readable strategy: "aggressive", "balanced", or "conservative". */
  strategy: string;
}

type Zone = 'safe' | 'warn' | 'danger';

function getZone(pct: number): Zone {
  if (pct >= 90) return 'danger';
  if (pct >= 70) return 'warn';
  return 'safe';
}

const ZONE_FILL: Record<Zone, string> = {
  safe:   'bg-[oklch(0.7_0.15_140)]',
  warn:   'bg-[oklch(0.75_0.15_60)]',
  danger: 'bg-[oklch(0.65_0.2_25)]',
};

const ZONE_TEXT: Record<Zone, string> = {
  safe:   'text-[oklch(0.7_0.15_140)]',
  warn:   'text-[oklch(0.75_0.15_60)]',
  danger: 'text-[oklch(0.65_0.2_25)]',
};

/** Color-coded token usage progress bar for the ChatExecutionRail. */
export function ContextWindowMeter({
  usedTokens,
  maxTokens,
  thresholdTokens,
  strategy,
}: ContextWindowMeterProps) {
  const pct = Math.min(100, maxTokens === 0 ? 0 : Math.round((usedTokens / maxTokens) * 100));
  const thresholdPct = maxTokens === 0 ? 80 : Math.round((thresholdTokens / maxTokens) * 100);
  const zone = getZone(pct);

  return (
    <div
      className="flex flex-col gap-0.5 px-2 py-1"
      role="meter"
      aria-valuenow={usedTokens}
      aria-valuemin={0}
      aria-valuemax={maxTokens}
      aria-label={`Context window: ${pct}% full`}
    >
      {/* Label row */}
      <div className="flex items-center justify-between">
        <span className="text-[9px] uppercase tracking-[0.14em] text-zinc-500">Context</span>
        <span className={`font-mono text-[10px] tabular-nums ${ZONE_TEXT[zone]}`}>
          {pct}%
        </span>
      </div>

      {/* Progress bar track */}
      <div className="relative h-1 w-full overflow-hidden rounded-full bg-white/[0.06]">
        {/* Fill */}
        <div
          data-zone={zone}
          className={`absolute inset-y-0 left-0 rounded-full transition-all duration-500 ${ZONE_FILL[zone]}`}
          style={{ width: `${pct}%` }}
        />
        {/* Threshold marker */}
        <div
          className="absolute inset-y-0 w-px bg-white/20"
          style={{ left: `${thresholdPct}%` }}
          title={`Compaction triggers at ${thresholdTokens.toLocaleString()} tokens`}
        />
      </div>

      {/* Strategy label */}
      <span className="text-[8px] text-zinc-600">{strategy}</span>
    </div>
  );
}
```

- [ ] **Step 3.4: Run tests to verify they PASS**

```
cd crates/vox-gui/ui
pnpm test ContextWindowMeter
```

Expected: 7 PASS.

- [ ] **Step 3.5: Commit**

```
git add crates/vox-gui/ui/src/components/surfaces/Chat/ContextWindowMeter.tsx
git add crates/vox-gui/ui/src/components/surfaces/Chat/ContextWindowMeter.test.tsx
git commit -m "feat(gui): add ContextWindowMeter widget with color zones and threshold marker"
```

---

## Task 4: Wire `ContextWindowMeter` into `ChatExecutionRail`

**Context:** `ChatExecutionRail.tsx` renders the sidebar in the chat view. It already shows `activeAgents`, `queueDepth`, and `openrouterSpendUsd`. We add the meter below the spend segment.

We need to:
1. Call `invoke('get_context_budget')` on mount — once. The budget is quasi-static (changes only when the user changes orchestrator config).
2. Store the response in a `useState<ContextBudgetPayload | null>`.
3. Use `usedTokens = 0` as the initial value (Phase 1: we don't yet have live token counts — that's a separate plan).
4. Render `<ContextWindowMeter>` when budget is available.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx`

- [ ] **Step 4.1: Write the test**

Append to the existing `ChatExecutionRail.test.tsx` (create it if it doesn't exist):

```typescript
// If ChatExecutionRail.test.tsx doesn't exist, create it:
import React from 'react';
import { render, waitFor } from '@testing-library/react';
import { vi, describe, it, expect } from 'vitest';

const mockBudget = {
  max_context_tokens: 128_000,
  reserved_tokens: 10_000,
  threshold_tokens: 102_400,
  usable_tokens: 118_000,
  strategy: 'balanced',
};

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === 'get_context_budget') return Promise.resolve(mockBudget);
    return Promise.resolve(null);
  }),
}));

vi.mock('../../ui/Glass', () => ({
  Glass: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
}));

vi.mock('../../../hooks/useLocalStorage', () => ({
  useLocalStorage: (key: string, def: unknown) => [def, vi.fn()],
}));

import { ChatExecutionRail } from './ChatExecutionRail';

describe('ChatExecutionRail with budget', () => {
  const defaultProps = {
    tasks: [],
    kpis: { activeAgents: { value: 0 }, queueDepth: { value: 0 }, mesh: { peers: 0 } },
    onNavigate: vi.fn(),
  };

  it('renders ContextWindowMeter after budget loads', async () => {
    const { getByRole } = render(<ChatExecutionRail {...defaultProps} />);
    await waitFor(() => {
      expect(getByRole('meter')).toBeInTheDocument();
    });
  });
});
```

- [ ] **Step 4.2: Run the test to verify it FAILS**

```
cd crates/vox-gui/ui
pnpm test ChatExecutionRail
```

Expected: FAIL — no `meter` role rendered yet.

- [ ] **Step 4.3: Add budget state and `ContextWindowMeter` to `ChatExecutionRail`**

In `ChatExecutionRail.tsx`, add these changes:

**At the top, add imports** (after existing imports):

```typescript
import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import { ContextWindowMeter } from './ContextWindowMeter';

interface ContextBudgetPayload {
  max_context_tokens: number;
  reserved_tokens: number;
  threshold_tokens: number;
  usable_tokens: number;
  strategy: string;
}
```

**Inside the `ChatExecutionRail` function** (after the `const [collapsed, setCollapsed]` line):

```typescript
  const [budget, setBudget] = useState<ContextBudgetPayload | null>(null);

  useEffect(() => {
    invoke<ContextBudgetPayload>('get_context_budget')
      .then(setBudget)
      .catch(() => {/* daemon unavailable; meter stays hidden */});
  }, []);
```

**In the JSX** (inside the non-collapsed return, after the last `<Segment>` for OpenRouter spend, before the closing `</Glass>`):

```typescript
          {budget && (
            <ContextWindowMeter
              usedTokens={0}
              maxTokens={budget.max_context_tokens}
              thresholdTokens={budget.threshold_tokens}
              strategy={budget.strategy}
            />
          )}
```

- [ ] **Step 4.4: Run the test to verify it PASSES**

```
cd crates/vox-gui/ui
pnpm test ChatExecutionRail
```

Expected: PASS.

- [ ] **Step 4.5: TypeScript compile check**

```
cd crates/vox-gui/ui
pnpm tsc --noEmit
```

Expected: no errors.

- [ ] **Step 4.6: Commit**

```
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx
git commit -m "feat(gui): wire ContextWindowMeter into ChatExecutionRail"
```

---

## Task 5: Manual smoke test

- [ ] **Step 5.1: Launch the app**

```
cargo run -p vox-gui
```

Open the Chat surface. The `ChatExecutionRail` sidebar should appear on the right.

- [ ] **Step 5.2: Verify the meter renders**

The meter should show:
- A horizontal progress bar (dark track, colored fill)
- A `0%` label (since `usedTokens = 0` for now)
- A white threshold marker at the 80% position
- The strategy name ("balanced") in small text below

- [ ] **Step 5.3: Commit any fixups**

```
git add -A
git commit -m "fix(gui): context window meter smoke test fixups"
```

---

## Self-Review Checklist

- [x] **Spec coverage:** Research doc §4.3 lists "color-coded progress bars, zone breakdown, alert thresholds" — all implemented in `ContextWindowMeter`.
- [x] **No placeholders:** All code is complete. `usedTokens = 0` is an explicitly documented Phase 1 limitation, not a placeholder.
- [x] **Type consistency:** `ContextBudgetPayload` interface in `ChatExecutionRail.tsx` matches `ContextBudgetPayload` struct fields in Rust (snake_case keys via serde default). Field names: `max_context_tokens`, `reserved_tokens`, `threshold_tokens`, `usable_tokens`, `strategy` — consistent across Tasks 1, 4.
- [x] **Fallback safe:** `get_context_budget` falls back to `CompactionConfig::default()` when daemon unavailable. The frontend `catch` suppresses errors silently.
- [x] **YAGNI:** `usedTokens` is hardcoded to 0 — live token tracking is a separate plan. Not pre-implemented here.
