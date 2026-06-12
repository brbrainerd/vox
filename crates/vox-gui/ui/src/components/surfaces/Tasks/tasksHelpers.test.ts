import { describe, it, expect } from 'vitest';
import { groupTasks, cyclePriority, TaskRow } from './tasksHelpers';

const row = (over: Partial<TaskRow>): TaskRow => ({
  id: 1,
  description: 'd',
  priority: 'normal',
  lifecycle: 'queued',
  agent_id: null,
  session_id: null,
  estimated_complexity: 1,
  depends_on: [],
  write_files: [],
  ...over,
});

describe('groupTasks', () => {
  it('splits in-progress from queued and orders queued urgent>normal>background', () => {
    const rows = [
      row({ id: 1, lifecycle: 'in_progress' }),
      row({ id: 2, priority: 'background' }),
      row({ id: 3, priority: 'urgent' }),
      row({ id: 4, priority: 'normal' }),
    ];
    const g = groupTasks(rows);
    expect(g.inProgress.map(t => t.id)).toEqual([1]);
    expect(g.queued.map(t => t.id)).toEqual([3, 4, 2]);
  });

  it('treats unknown lifecycle labels as queued', () => {
    const g = groupTasks([row({ id: 9, lifecycle: 'weird' })]);
    expect(g.queued).toHaveLength(1);
  });
});

describe('cyclePriority', () => {
  it('cycles background→normal→urgent→background', () => {
    expect(cyclePriority('background')).toBe('normal');
    expect(cyclePriority('normal')).toBe('urgent');
    expect(cyclePriority('urgent')).toBe('background');
    expect(cyclePriority('garbage')).toBe('normal');
  });
});
