import type { PolicyRow, PolicyStatus, RunStatus } from './types';
import { STATUS_RANK } from './types';

export interface GroupNode {
  group: string;
  rows: PolicyRow[];
  counts: Record<RunStatus, number>;
  worst: RunStatus;
}

/** Worst (highest-rank) of a list; empty → not_run (grey). */
export function worstStatus(statuses: RunStatus[]): RunStatus {
  return statuses.reduce<RunStatus>(
    (acc, s) => (STATUS_RANK[s] > STATUS_RANK[acc] ? s : acc),
    'not_run',
  );
}

/** A rule's effective status = worst across the selected branch set. */
export function statusForRow(id: string, status: PolicyStatus[], branches: string[]): RunStatus {
  const sel = new Set(branches);
  const hits = status.filter(s => s.id === id && sel.has(s.branch)).map(s => s.status);
  return worstStatus(hits);
}

function emptyCounts(): Record<RunStatus, number> {
  return { fail: 0, warn: 0, pass: 0, not_run: 0 };
}

/** Group rows by their `group` label; roll up per-group status counts + worst. */
export function buildGroupTree(rows: PolicyRow[], status: PolicyStatus[], branches: string[]): GroupNode[] {
  const byGroup = new Map<string, PolicyRow[]>();
  for (const r of rows) {
    const arr = byGroup.get(r.group) ?? [];
    arr.push(r);
    byGroup.set(r.group, arr);
  }
  const nodes: GroupNode[] = [];
  for (const [group, grpRows] of byGroup) {
    const counts = emptyCounts();
    const perRow: RunStatus[] = [];
    for (const r of grpRows) {
      const s = statusForRow(r.id, status, branches);
      counts[s] += 1;
      perRow.push(s);
    }
    nodes.push({ group, rows: grpRows, counts, worst: worstStatus(perRow) });
  }
  // Stable display order: worst groups first, then alphabetical.
  nodes.sort((a, b) => STATUS_RANK[b.worst] - STATUS_RANK[a.worst] || a.group.localeCompare(b.group));
  return nodes;
}

/** Rows that are failing or warning on any selected branch (the shrinking group). */
export function needsAttention(rows: PolicyRow[], status: PolicyStatus[], branches: string[]): PolicyRow[] {
  return rows
    .filter(r => {
      const s = statusForRow(r.id, status, branches);
      return s === 'fail' || s === 'warn';
    })
    .sort((a, b) => a.id.localeCompare(b.id));
}

/** Master-sidebar badge: worst status across the whole catalog for the selection. */
export function overallWorst(rows: PolicyRow[], status: PolicyStatus[], branches: string[]): RunStatus {
  return worstStatus(rows.map(r => statusForRow(r.id, status, branches)));
}

/**
 * Count of rules at the worst status (for the master-sidebar badge number).
 *
 * When the worst status is `not_run` (e.g. an empty store, where every rule is
 * grey), the badge would otherwise show the full catalog count (e.g. 176) — a
 * noisy "everything's not run" number. Return 0 in that case so only a grey dot
 * renders. All other (actionable) statuses keep their count.
 */
export function worstCount(rows: PolicyRow[], status: PolicyStatus[], branches: string[]): number {
  const worst = overallWorst(rows, status, branches);
  if (worst === 'not_run') return 0;
  return rows.filter(r => statusForRow(r.id, status, branches) === worst).length;
}
