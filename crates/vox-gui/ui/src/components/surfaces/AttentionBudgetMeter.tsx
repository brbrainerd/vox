import { useEffect, useRef, useState } from 'react';
import type { AttentionBudgetSnapshot } from '../../types/tauri';
import { Pill } from '../ui/Pill';

interface Props {
  budget: AttentionBudgetSnapshot | null | undefined;
  waitingQuestions?: number;
  blockedTasks?: number;
  /**
   * Desired collapsed/expanded state, computed by the caller (ChatSurface)
   * from "is there an active, non-trivial conversation and nothing urgent
   * that needs the full card visible". Tracked reactively — NOT just a
   * `useState` initializer — because the caller's inputs (message count,
   * budget spend) typically settle asynchronously after this component
   * first mounts (e.g. `attention_budget` arrives from the orchestrator
   * status stream before the session's messages finish hydrating), so a
   * mount-only initializer would permanently miss the "now there's a real
   * conversation, collapse" transition. Once the user manually toggles,
   * their choice wins over any further prop changes for this mount.
   */
  defaultCollapsed?: boolean;
}

// Mirrors Rust AttentionBudget::focus_depth() thresholds.
function focusLabel(freqPerHour: number): string {
  if (freqPerHour >= 8) return 'Deep focus';
  if (freqPerHour >= 3) return 'Focused';
  return 'Ambient focus';
}

/**
 * Read-only attention-budget surface (Track D, audit #1). Shows session attention spent,
 * the current focus depth (derived from interrupt frequency), and how many A2A prompts
 * were suppressed under Deep focus. Rides the existing orchestrator status stream.
 *
 * Collapsible (panels-density Task 5): a realistic-content audit of the Chat
 * transcript/composer panel found this card's ~110-120px fixed footprint
 * (header + progress bar + two caption lines + padding) unconditionally
 * competing with the transcript for vertical room, regardless of how long
 * the conversation above it is. Collapsed, it renders a single ~28px summary
 * row (focus label + pct/minutes) with the same `role="meter"` semantics
 * preserved so screen-reader users see no behavior change across states.
 */
export function AttentionBudgetMeter({ budget, waitingQuestions, blockedTasks, defaultCollapsed = false }: Props) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);
  const userToggledRef = useRef(false);
  useEffect(() => {
    if (userToggledRef.current) return;
    setCollapsed(defaultCollapsed);
  }, [defaultCollapsed]);
  if (!budget) return null;
  const ratio = budget.max_attention_ms > 0 ? budget.spent_ms / budget.max_attention_ms : 1;
  const pct = Math.round(Math.min(Math.max(ratio, 0), 1) * 100);
  const min = (ms: number) => Math.round(ms / 60_000);
  return (
    <section className="attention-budget-meter" aria-label="Attention budget" data-collapsed={collapsed}>
      <header>
        <span>Attention budget</span>
        <div className="flex items-center gap-1.5">
          {collapsed && (
            <span className="attention-budget-meter__summary">
              {pct}% · {min(budget.spent_ms)}/{min(budget.max_attention_ms)}m
            </span>
          )}
          <span>{focusLabel(budget.interrupt_freq_per_hour)}</span>
          <button
            type="button"
            aria-expanded={!collapsed}
            aria-label={collapsed ? 'Expand attention budget details' : 'Collapse attention budget details'}
            onClick={() => {
              userToggledRef.current = true;
              setCollapsed(c => !c);
            }}
            className="attention-budget-meter__toggle"
          >
            {collapsed ? '▸' : '▾'}
          </button>
        </div>
      </header>
      <div
        role="meter"
        aria-label="Attention spent"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct}
        className={collapsed ? 'attention-budget-meter__bar--compact' : undefined}
      >
        <div className="attention-budget-meter__fill" style={{ width: `${pct}%` }} />
      </div>
      {!collapsed && (
        <>
          <div className="flex items-center gap-1.5 mt-2">
            {!!waitingQuestions && <Pill phase="Planning" label={`${waitingQuestions} waiting`} />}
            {!!blockedTasks && <Pill phase="Doubted" label={`${blockedTasks} blocked`} />}
          </div>
          <p>{min(budget.spent_ms)} / {min(budget.max_attention_ms)} min spent ({pct}%)</p>
          <p>Suppressed prompts (Deep focus): {budget.inbox_suppressed_count}</p>
        </>
      )}
    </section>
  );
}
