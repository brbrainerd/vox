export interface HopperTaskDto {
  item_id: string;
  intent: string;
  priority: number;
  state: string;
  task_id: number;
}

export interface TaskRow {
  id: number | string;
  description: string;
  priority: string; // 'urgent' | 'normal' | 'background' (normalized by the Tauri DTO)
  lifecycle: string; // 'queued' | 'in_progress' | 'blocked' | 'completed' | 'unknown'
  agent_id: number | null;
  session_id: string | null;
  estimated_complexity: number;
  depends_on: (number | string)[];
  write_files: string[];
  remote_node: string | null;
}

export interface GroupedTasks {
  inProgress: TaskRow[];
  queued: TaskRow[];
  blocked: TaskRow[];
}

const PRIORITY_ORDER: Record<string, number> = { urgent: 0, normal: 1, background: 2 };

export function groupTasks(rows: TaskRow[]): GroupedTasks {
  const inProgress = rows.filter(t => t.lifecycle === 'in_progress');
  const blocked = rows.filter(t => t.lifecycle === 'blocked');
  const queued = rows
    .filter(t => t.lifecycle !== 'in_progress' && t.lifecycle !== 'blocked')
    .sort(
      (a, b) =>
        (PRIORITY_ORDER[a.priority] ?? 1) - (PRIORITY_ORDER[b.priority] ?? 1) ||
        String(a.id).localeCompare(String(b.id)),
    );
  return { inProgress, queued, blocked };
}

/**
 * Map each task id to the other queued task ids that write at least one of the
 * same files. The orchestrator serializes these via file locks (and may split
 * VCS changes); surfacing it tells the user why two tasks won't run in parallel.
 */
export function findWriteOverlaps(rows: TaskRow[]): Map<string | number, (string | number)[]> {
  const byFile = new Map<string, (string | number)[]>();
  for (const t of rows) {
    for (const f of t.write_files) {
      const list = byFile.get(f) ?? [];
      list.push(t.id);
      byFile.set(f, list);
    }
  }
  const out = new Map<string | number, (string | number)[]>();
  for (const ids of byFile.values()) {
    if (ids.length < 2) continue;
    for (const id of ids) {
      const others = ids.filter(o => o !== id);
      const cur = out.get(id) ?? [];
      out.set(id, [...new Set([...cur, ...others])].sort((a, b) => String(a).localeCompare(String(b))));
    }
  }
  return out;
}

/**
 * Map raw hopper task DTOs into display rows, marking any task gated by a
 * pending needs-you feedback item as 'blocked'. Shared by TasksView's
 * self-fetch path and its shared-attention-inbox path so both derive rows
 * identically regardless of where the underlying data came from.
 */
export function mapHopperTasksToRows(tasks: HopperTaskDto[], gatedTaskIds: Set<number>): TaskRow[] {
  return tasks.map(dto => ({
    id: dto.item_id,
    description: dto.intent,
    priority: dto.priority === 2 ? 'urgent' : dto.priority === 0 ? 'background' : 'normal',
    lifecycle: gatedTaskIds.has(dto.task_id)
      ? 'blocked'
      : dto.state === 'assigned'
      ? 'in_progress'
      : dto.state === 'inbox'
      ? 'queued'
      : dto.state === 'done'
      ? 'completed'
      : 'unknown',
    agent_id: null,
    session_id: null,
    estimated_complexity: 1,
    depends_on: [],
    write_files: [],
    remote_node: null,
  }));
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
