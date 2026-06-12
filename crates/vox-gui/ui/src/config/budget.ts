/**
 * Budget display state — orchestrator daemon is authoritative once status arrives.
 */

/** Used only until first daemon status snapshot; never shown as a fake cap in UI. */
export const DEFAULT_BUDGET_CAP_USD = 50.0;

export type BudgetSource = 'daemon' | 'fallback';

export interface BudgetState {
  spent: number;
  cap: number | null;
  source: BudgetSource;
}

export function budgetStateFromStatus(
  totalCost: number | undefined,
  budgetCap: number | undefined | null,
): BudgetState {
  if (budgetCap != null && Number.isFinite(budgetCap)) {
    return {
      spent: totalCost ?? 0,
      cap: budgetCap,
      source: 'daemon',
    };
  }
  return {
    spent: totalCost ?? 0,
    cap: null,
    source: 'fallback',
  };
}

/** Format cap for display; shows em-dash when unknown. */
export function formatBudgetCap(cap: number | null, source: BudgetSource): string {
  if (source === 'daemon' && cap != null) {
    return `$${cap.toFixed(2)}`;
  }
  return '—';
}
