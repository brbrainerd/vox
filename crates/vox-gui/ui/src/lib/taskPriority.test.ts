import { describe, it, expect } from 'vitest';
import { TASK_PRIORITY_WIRE, priorityLabel, priorityValue } from './taskPriority';

describe('task priority wire constants', () => {
  it('pins the Rust TaskPriority discriminants (crates/vox-orchestrator/src/types/tasks.rs:44-51)', () => {
    expect(TASK_PRIORITY_WIRE.background).toBe(0);
    expect(TASK_PRIORITY_WIRE.normal).toBe(1);
    expect(TASK_PRIORITY_WIRE.urgent).toBe(2);
  });
  it('round-trips label <-> value with normal as the fallback', () => {
    expect(priorityLabel(2)).toBe('urgent');
    expect(priorityLabel(0)).toBe('background');
    expect(priorityLabel(99)).toBe('normal');
    expect(priorityValue('urgent')).toBe(2);
    expect(priorityValue('garbage')).toBe(1);
  });
});
