import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { SubAgentNode } from './types';

export interface SubagentTreeEdge {
  task_id: number;
  agent_id: number;
  parent_agent_id?: number | null;
  source_task_id?: number | null;
  reason: string;
  /** Chat session that originated this delegation (Phase D Task D1/D3), when
   *  the spawn happened inside a chat turn. */
  chat_session_id?: string | null;
  /** Provider tool-call id of the spawn request, for correlating this edge
   *  back to the exact turn (Phase D Task D1/D3). */
  origin_turn_id?: string | null;
}

/** Map the orchestrator's flat delegation edges into the SubAgentNode tree. */
export function buildSubAgentTree(edges: SubagentTreeEdge[]): SubAgentNode[] {
  const byId = new Map<string, SubAgentNode>();
  for (const e of edges) {
    byId.set(`agent-${e.agent_id}`, {
      windowId: `agent-${e.agent_id}`,
      parentWindowId: e.parent_agent_id != null ? `agent-${e.parent_agent_id}` : null,
      title: `task #${e.task_id} · ${e.reason}`,
      skill: null,
      // Honest unknowns: the daemon does not report model/token budgets on this
      // edge list. 0/0 renders as "budget unknown", never a fabricated number.
      model: { id: 'orchestrator', maxTokens: 0, toolCapable: false },
      status: 'running',
      usedTokens: 0,
      depth: 0,
      children: [],
    });
  }
  const roots: SubAgentNode[] = [];
  for (const n of byId.values()) {
    const parent = n.parentWindowId ? byId.get(n.parentWindowId) : undefined;
    if (parent) {
      parent.children.push(n);
    } else {
      if (n.parentWindowId) {
        // Dangling reference, not a true root: the edge list named a parent
        // agent that isn't itself present. Surfacing it as a root keeps the
        // agent visible rather than silently dropping it, but this usually
        // means a still-in-flight fetch or an id-space mismatch worth a look.
        console.warn(`SubAgents: agent ${n.windowId} references unknown parent ${n.parentWindowId}; showing as a root`);
      }
      roots.push(n);
    }
  }
  const stamp = (list: SubAgentNode[], depth: number) => {
    for (const n of list) { n.depth = depth; stamp(n.children, depth + 1); }
  };
  stamp(roots, 0);
  return roots;
}

/** Raw delegation edges, straight from the daemon — unlike `fetchTree()`,
 *  which discards `chat_session_id`/`origin_turn_id` while building the
 *  windowed `SubAgentNode[]` tree. Callers that need to correlate a spawn back
 *  to the chat turn/session that caused it (Phase D Task D3 — e.g.
 *  `agentsForSession`/`pausedAgentForSession`) should use this, not `fetchTree()`. */
export async function fetchEdges(): Promise<SubagentTreeEdge[]> {
  const edges = await invoke<SubagentTreeEdge[]>('list_subagent_tree');
  if (!Array.isArray(edges)) {
    console.warn('SubAgents: list_subagent_tree resolved a non-array payload', edges);
    return [];
  }
  return edges;
}

export async function fetchTree(): Promise<SubAgentNode[]> {
  const edges = await fetchEdges();
  return buildSubAgentTree(edges);
}

/** Subscribe to live agent-events; rejects outside Tauri (caller degrades). */
export const SUBAGENT_ACTIVITY_EVENT = 'vox://agent-events';
export function listenActivity(onEvent: (e: { id: number; timestamp_ms: number; kind: { type: string; [k: string]: unknown } }) => void): Promise<UnlistenFn> {
  return listen(SUBAGENT_ACTIVITY_EVENT, (e) => onEvent(e.payload as { id: number; timestamp_ms: number; kind: { type: string; [k: string]: unknown } }));
}
