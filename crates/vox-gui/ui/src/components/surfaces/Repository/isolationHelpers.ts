// Pure view-model helpers for the VCS isolation panel.
//
// The backend serves `isolation_status_json` (see
// `crates/vox-orchestrator/src/json_vcs_facade.rs`) and the
// `/api/v2/vcs/isolation` REST surface. These helpers normalize that raw JSON
// shape into render-ready rows so the component stays declarative and the
// parsing logic is unit-testable without a DOM (the project's vitest harness
// collects `*.test.ts`, not component renders).

/** The three orchestrator isolation strategies (serde `snake_case`). */
export type IsolationStrategy =
  | 'shared_branch'
  | 'split_changes'
  | 'separate_branches';

export const ISOLATION_STRATEGIES: IsolationStrategy[] = [
  'shared_branch',
  'split_changes',
  'separate_branches',
];

/** Raw `isolation_status_json` payload (the `data` of the REST envelope). */
export interface IsolationStatus {
  strategy_default?: string;
  per_agent?: Record<string, string>;
  active_conflicts?: Array<{
    id?: string;
    path?: string;
    sides?: string[];
    created_ms?: number;
  }>;
}

/** A per-agent override row for the table. */
export interface PerAgentRow {
  agentId: string;
  strategy: IsolationStrategy;
}

/** A normalized active-conflict row. */
export interface ConflictRow {
  id: string;
  path: string;
  sides: string[];
}

/** Human label for a strategy id (falls back to the raw id). */
export function strategyLabel(strategy: string): string {
  switch (strategy) {
    case 'shared_branch':
      return 'Shared Branch';
    case 'split_changes':
      return 'Split Changes';
    case 'separate_branches':
      return 'Separate Branches';
    default:
      return strategy;
  }
}

/** Narrow an arbitrary string to a known strategy, defaulting to shared_branch. */
export function asStrategy(value: string | undefined): IsolationStrategy {
  return (ISOLATION_STRATEGIES as string[]).includes(value ?? '')
    ? (value as IsolationStrategy)
    : 'shared_branch';
}

/** The effective default strategy, defaulting to `shared_branch`. */
export function defaultStrategy(status: IsolationStatus | null | undefined): IsolationStrategy {
  return asStrategy(status?.strategy_default);
}

/** Per-agent override rows, sorted by numeric agent id for stable rendering. */
export function perAgentRows(status: IsolationStatus | null | undefined): PerAgentRow[] {
  const map = status?.per_agent ?? {};
  return Object.entries(map)
    .map(([agentId, strategy]) => ({ agentId, strategy: asStrategy(strategy) }))
    .sort((a, b) => {
      const na = Number(a.agentId);
      const nb = Number(b.agentId);
      if (Number.isFinite(na) && Number.isFinite(nb)) return na - nb;
      return a.agentId.localeCompare(b.agentId);
    });
}

/** Normalized active-conflict rows (missing fields tolerated). */
export function conflictRows(status: IsolationStatus | null | undefined): ConflictRow[] {
  const list = status?.active_conflicts ?? [];
  return list.map((c, i) => ({
    id: c.id ?? `conflict-${i}`,
    path: c.path ?? '(unknown path)',
    sides: Array.isArray(c.sides) ? c.sides : [],
  }));
}
