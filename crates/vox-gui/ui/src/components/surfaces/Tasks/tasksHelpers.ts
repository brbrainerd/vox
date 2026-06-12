export interface TaskRow {
  id: number;
  description: string;
  priority: string; // 'urgent' | 'normal' | 'background' (normalized by the Tauri DTO)
  lifecycle: string; // 'queued' | 'in_progress' | 'blocked' | 'completed' | 'unknown'
  agent_id: number | null;
  session_id: string | null;
  estimated_complexity: number;
  depends_on: number[];
  write_files: string[];
}

export interface GroupedTasks {
  inProgress: TaskRow[];
  queued: TaskRow[];
}

const PRIORITY_ORDER: Record<string, number> = { urgent: 0, normal: 1, background: 2 };

export function groupTasks(rows: TaskRow[]): GroupedTasks {
  const inProgress = rows.filter(t => t.lifecycle === 'in_progress');
  const queued = rows
    .filter(t => t.lifecycle !== 'in_progress')
    .sort(
      (a, b) =>
        (PRIORITY_ORDER[a.priority] ?? 1) - (PRIORITY_ORDER[b.priority] ?? 1) || a.id - b.id,
    );
  return { inProgress, queued };
}

/**
 * Map each task id to the other queued task ids that write at least one of the
 * same files. The orchestrator serializes these via file locks (and may split
 * VCS changes); surfacing it tells the user why two tasks won't run in parallel.
 */
export function findWriteOverlaps(rows: TaskRow[]): Map<number, number[]> {
  const byFile = new Map<string, number[]>();
  for (const t of rows) {
    for (const f of t.write_files) {
      const list = byFile.get(f) ?? [];
      list.push(t.id);
      byFile.set(f, list);
    }
  }
  const out = new Map<number, number[]>();
  for (const ids of byFile.values()) {
    if (ids.length < 2) continue;
    for (const id of ids) {
      const others = ids.filter(o => o !== id);
      const cur = out.get(id) ?? [];
      out.set(id, [...new Set([...cur, ...others])].sort((a, b) => a - b));
    }
  }
  return out;
}

export function filterBySession(rows: TaskRow[], sessionId: string | null): TaskRow[] {
  if (!sessionId) return rows;
  return rows.filter(t => t.session_id === sessionId);
}

export function cyclePriority(p: string): string {
  if (p === 'background') return 'normal';
  if (p === 'normal') return 'urgent';
  if (p === 'urgent') return 'background';
  return 'normal';
}
