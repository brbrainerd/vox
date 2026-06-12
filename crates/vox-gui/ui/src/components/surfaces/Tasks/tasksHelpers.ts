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

export function cyclePriority(p: string): string {
  if (p === 'background') return 'normal';
  if (p === 'normal') return 'urgent';
  if (p === 'urgent') return 'background';
  return 'normal';
}
