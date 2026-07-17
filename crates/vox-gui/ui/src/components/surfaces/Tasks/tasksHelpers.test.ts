import { describe, it, expect } from 'vitest';
import { groupTasks, cyclePriority, filterBySession, findWriteOverlaps, mapHopperTasksToRows, TaskRow } from './tasksHelpers';

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
  remote_node: null,
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

  it('separates blocked lifecycle into its own bucket', () => {
    const rows = [
      row({ id: 1, lifecycle: 'in_progress' }),
      row({ id: 2, lifecycle: 'queued' }),
      row({ id: 3, lifecycle: 'blocked' }),
    ];
    const g = groupTasks(rows);
    expect(g.blocked.map(t => t.id)).toEqual([3]);
    expect(g.inProgress.map(t => t.id)).toEqual([1]);
    expect(g.queued.map(t => t.id)).toEqual([2]);
  });
});

describe('filterBySession', () => {
  it('returns all rows for null filter and only matching session otherwise', () => {
    const rows = [
      row({ id: 1, session_id: 'gui-a' }),
      row({ id: 2, session_id: 'gui-b' }),
      row({ id: 3, session_id: null }),
    ];
    expect(filterBySession(rows, null)).toHaveLength(3);
    expect(filterBySession(rows, 'gui-a').map(t => t.id)).toEqual([1]);
  });
});

describe('findWriteOverlaps', () => {
  it('maps each task to the other task ids sharing a write file', () => {
    const rows = [
      row({ id: 1, write_files: ['a.rs', 'b.rs'] }),
      row({ id: 2, write_files: ['b.rs'] }),
      row({ id: 3, write_files: ['c.rs'] }),
    ];
    const m = findWriteOverlaps(rows);
    expect(m.get(1)).toEqual([2]);
    expect(m.get(2)).toEqual([1]);
    expect(m.get(3)).toBeUndefined();
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

describe('mapHopperTasksToRows', () => {
  it('maps DTO priority numbers and lifecycle states, with no gates', () => {
    const rows = mapHopperTasksToRows(
      [
        { item_id: 'a', intent: 'A', priority: 2, state: 'assigned', task_id: 1 },
        { item_id: 'b', intent: 'B', priority: 0, state: 'inbox', task_id: 2 },
        { item_id: 'c', intent: 'C', priority: 1, state: 'done', task_id: 3 },
        { item_id: 'd', intent: 'D', priority: 1, state: 'something-else', task_id: 4 },
      ],
      new Set(),
    );
    expect(rows.map(r => [r.priority, r.lifecycle])).toEqual([
      ['urgent', 'in_progress'],
      ['background', 'queued'],
      ['normal', 'completed'],
      ['normal', 'unknown'],
    ]);
  });

  it('marks a task blocked when its task_id is in the gated set, overriding its raw state', () => {
    const rows = mapHopperTasksToRows(
      [{ item_id: 'a', intent: 'A', priority: 1, state: 'assigned', task_id: 7 }],
      new Set([7]),
    );
    expect(rows[0].lifecycle).toBe('blocked');
  });

  it('an empty gated set never blocks anything', () => {
    const rows = mapHopperTasksToRows(
      [{ item_id: 'a', intent: 'A', priority: 1, state: 'inbox', task_id: 7 }],
      new Set(),
    );
    expect(rows[0].lifecycle).toBe('queued');
  });

  it('carries real session/agent/mesh fields from the DTO instead of hardcoded nulls', () => {
    const rows = mapHopperTasksToRows(
      [{
        item_id: 'a', intent: 'A', priority: 1, state: 'assigned', task_id: 1,
        session_id: 'gui-9', agent_id: 'agent-42', remote_node: 'did:vox:peer-1',
      }],
      new Set(),
    );
    expect(rows[0].session_id).toBe('gui-9');
    expect(rows[0].agent_id).toBe('agent-42');
    expect(rows[0].remote_node).toBe('did:vox:peer-1');
  });
});
