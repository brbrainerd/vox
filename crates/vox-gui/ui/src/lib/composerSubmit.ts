/**
 * Payload builder for persisting a composer-submitted user message via the
 * `chat_append_message` Tauri command.
 *
 * `already_submitted: true` tells the backend secretary (chat.rs
 * `chat_append_message`) that this message was ALREADY dispatched as a task
 * by the composer (`submit_orchestrator_task`). Without it, every actionable
 * composer message was submitted twice (audit finding C2), producing a
 * spurious "Secretary proposed a task" toast or a wrong near-duplicate
 * confirm dialog depending on which submit lost the race.
 */
export interface ComposerUserAppendInput {
  session_id: string;
  role: 'user';
  content: string;
  task_id: null;
  already_submitted: true;
}

export function userAppendInput(
  sessionId: string,
  description: unknown,
): ComposerUserAppendInput {
  return {
    session_id: sessionId,
    role: 'user',
    content: String(description ?? ''),
    task_id: null,
    already_submitted: true,
  };
}
