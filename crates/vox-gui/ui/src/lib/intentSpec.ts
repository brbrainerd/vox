/** Structured-intent fields for the composer. Effort maps 1:1 onto the
 *  orchestrator TaskPriority strings accepted by submit_orchestrator_task
 *  (crates/vox-gui/src/commands/control_plane.rs). */
export type Effort = '' | 'background' | 'normal' | 'urgent';

export interface IntentFields {
  goal: string;
  constraints: string;
  acceptance: string;
  effort: Effort;
}

export const EMPTY_INTENT: IntentFields = { goal: '', constraints: '', acceptance: '', effort: '' };

export function hasIntent(i: IntentFields): boolean {
  return Boolean(i.goal.trim() || i.constraints.trim() || i.acceptance.trim() || i.effort);
}

function section(heading: string, value: string): string {
  return value.trim() ? `\n\n## ${heading}\n${value.trim()}` : '';
}

/** Compose the task description: free text first; goal promotes to the head
 *  line when free text is empty (goal-only submits stay valid). */
export function composeDescription(text: string, i: IntentFields): string {
  const head = text.trim() || i.goal.trim();
  const goalSection = text.trim() && i.goal.trim() ? section('Goal', i.goal) : '';
  return `${head}${goalSection}${section('Constraints', i.constraints)}${section('Acceptance criteria', i.acceptance)}`;
}

export function effortToPriority(effort: Effort): string | null {
  return effort === '' ? null : effort;
}
