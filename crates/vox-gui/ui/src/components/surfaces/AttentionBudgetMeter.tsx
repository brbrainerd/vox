import type { AttentionBudgetSnapshot } from '../../types/tauri';
import { Pill } from '../ui/Pill';

interface Props {
  budget: AttentionBudgetSnapshot | null | undefined;
  waitingQuestions?: number;
  blockedTasks?: number;
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
 */
export function AttentionBudgetMeter({ budget, waitingQuestions, blockedTasks }: Props) {
  if (!budget) return null;
  const ratio = budget.max_attention_ms > 0 ? budget.spent_ms / budget.max_attention_ms : 1;
  const pct = Math.round(Math.min(Math.max(ratio, 0), 1) * 100);
  const min = (ms: number) => Math.round(ms / 60_000);
  return (
    <section className="attention-budget-meter" aria-label="Attention budget">
      <header>
        <span>Attention budget</span>
        <span>{focusLabel(budget.interrupt_freq_per_hour)}</span>
      </header>
      <div role="meter" aria-label="Attention spent" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
        <div className="attention-budget-meter__fill" style={{ width: `${pct}%` }} />
      </div>
      <div className="flex items-center gap-1.5 mt-2">
        {!!waitingQuestions && <Pill phase="Planning" label={`${waitingQuestions} waiting`} />}
        {!!blockedTasks && <Pill phase="Doubted" label={`${blockedTasks} blocked`} />}
      </div>
      <p>{min(budget.spent_ms)} / {min(budget.max_attention_ms)} min spent ({pct}%)</p>
      <p>Suppressed prompts (Deep focus): {budget.inbox_suppressed_count}</p>
    </section>
  );
}
