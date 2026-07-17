/** Wire values for task priority, shared with Rust `TaskPriority`
 *  (crates/vox-orchestrator/src/types/tasks.rs:44-51: Background=0, Normal=1,
 *  Urgent=2). Guarded on the Rust side by
 *  `task_priority_wire_values_match_frontend_constants`. */
export const TASK_PRIORITY_WIRE = { background: 0, normal: 1, urgent: 2 } as const;

export type PriorityLabel = keyof typeof TASK_PRIORITY_WIRE;

export function priorityLabel(value: number): PriorityLabel {
  if (value === TASK_PRIORITY_WIRE.urgent) return 'urgent';
  if (value === TASK_PRIORITY_WIRE.background) return 'background';
  return 'normal';
}

export function priorityValue(label: string): number {
  return TASK_PRIORITY_WIRE[label as PriorityLabel] ?? TASK_PRIORITY_WIRE.normal;
}
