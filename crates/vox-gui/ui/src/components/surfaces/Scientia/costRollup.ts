import { invoke } from '@tauri-apps/api/core';

/** One row of `by_provider` from `vox scientia cost`. */
export interface CostByProvider {
  provider: string;
  usd: number;
}

/** Per-phase + total quarterly summary from `vox scientia cost`. */
export interface QuarterlyCostSummary {
  extraction_usd: number;
  critic_usd: number;
  novelty_retrieval_usd: number;
  scholarly_submission_usd: number;
  total_usd: number;
}

/**
 * The JSON emitted by `vox scientia cost` (mirrors
 * `vox_scientia::dashboard::cost::CostRollup`). An empty Codex DB yields an
 * all-zeros rollup with `by_provider: []` — a valid, expected zero state.
 */
export interface CostRollup {
  this_quarter: QuarterlyCostSummary;
  per_finding_average_usd: number;
  by_provider: CostByProvider[];
}

/**
 * A6: fetch the Scientia cost rollup via the native `scientia_cost_rollup`
 * command, which reads through this app's own already-open DB pool instead
 * of shelling out to a `vox scientia cost` subprocess (which used to contend
 * with this app's own connection for the same DB file lock). Throws on
 * failure so the caller can surface a toast.
 */
export async function fetchCostRollup(): Promise<CostRollup> {
  return invoke<CostRollup>('scientia_cost_rollup');
}

function usd(n: number): string {
  return `$${n.toFixed(2)}`;
}

/** Per-provider rows, dollar-formatted, preserving producer order. */
export function providerRows(rollup: CostRollup): { provider: string; usd: string }[] {
  return rollup.by_provider.map((p) => ({ provider: p.provider, usd: usd(p.usd) }));
}

/** Quarterly phase lines + total, dollar-formatted and labelled. */
export function quarterlyRows(rollup: CostRollup): { label: string; usd: string }[] {
  const q = rollup.this_quarter;
  return [
    { label: 'Extraction', usd: usd(q.extraction_usd) },
    { label: 'Critic', usd: usd(q.critic_usd) },
    { label: 'Novelty retrieval', usd: usd(q.novelty_retrieval_usd) },
    { label: 'Scholarly submission', usd: usd(q.scholarly_submission_usd) },
    { label: 'Total', usd: usd(q.total_usd) },
  ];
}
