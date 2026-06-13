import { invoke } from '@tauri-apps/api/core';

/** The daemon's fire-and-forget research.run envelope. */
export interface ResearchRunHandle {
  session_id: number;
  task_id: string;
  status: string;
}

/**
 * A2: start a research run asynchronously via the persistent orchestrator
 * daemon's `research.run` executor (Tauri command `start_research_async`).
 *
 * Returns the daemon's `{session_id, task_id, status: "running"}` envelope
 * immediately — it does NOT await the pipeline. The caller observes terminal
 * status via the Scientia-queue watcher + session-detail polling. This replaces
 * the old inline `execute_command(['research','run'], …)` path, which blocked
 * the UI for the whole pipeline.
 */
export async function startResearchAsync(args: {
  query: string;
  scope?: string;
  maxSources?: number;
  verifyClaims?: boolean;
}): Promise<ResearchRunHandle> {
  return invoke<ResearchRunHandle>('start_research_async', {
    query: args.query,
    scope: args.scope,
    maxSources: args.maxSources,
    verifyClaims: args.verifyClaims,
  });
}
