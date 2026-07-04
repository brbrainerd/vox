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
  // Only emit a separate "## Goal" section when both text and goal are set —
  // if text was empty, goal already became `head` above and must not repeat.
  const goalSection = text.trim() && i.goal.trim() ? section('Goal', i.goal) : '';
  const body = `${goalSection}${section('Constraints', i.constraints)}${section('Acceptance criteria', i.acceptance)}`;
  // Guard against a blank head (no text, no goal, only constraints/acceptance
  // set): callers are expected to gate submission on hasIntent()/non-empty
  // text, but this keeps the module correct standalone rather than relying
  // on that convention — a bare head would otherwise leave leading blank
  // lines before the first section heading.
  return head ? `${head}${body}` : body.replace(/^\n\n/, '');
}

export function effortToPriority(effort: Effort): 'urgent' | 'normal' | 'background' | null {
  return effort === '' ? null : effort;
}
