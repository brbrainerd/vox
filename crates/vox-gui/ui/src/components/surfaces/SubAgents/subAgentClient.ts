import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { SubAgentNode, ProjectionItem, ControlAction } from './types';

interface Envelope<T> { is_error: boolean; result: T; }
async function call<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  const env = (await invoke(cmd, args)) as Envelope<T>;
  if (env.is_error) throw new Error(`${cmd} failed`);
  return env.result;
}

export interface SubagentTreeEdge {
  task_id: number;
  agent_id: number;
  parent_agent_id?: number | null;
  source_task_id?: number | null;
  reason: string;
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

export async function fetchTree(): Promise<SubAgentNode[]> {
  const edges = await invoke<SubagentTreeEdge[]>('list_subagent_tree');
  if (!Array.isArray(edges)) {
    // A resolved non-array payload likely means a frontend/backend version
    // skew rather than "no data yet" — worth a signal, not a silent [].
    console.warn('SubAgents: list_subagent_tree resolved a non-array payload', edges);
    return [];
  }
  return buildSubAgentTree(edges);
}
export async function getContext(windowId: string): Promise<ProjectionItem[]> {
  return (await call<{ items: ProjectionItem[] }>('context_get', { windowId })).items;
}
export async function setContext(windowId: string, orderedItemIds: string[]): Promise<void> {
  await call('context_set', { windowId, orderedItemIds });
}
export async function control(windowId: string, action: ControlAction): Promise<void> {
  await call('subagent_control', { windowId, action });
}

/** Subscribe to live agent-events; rejects outside Tauri (caller degrades). */
export const SUBAGENT_ACTIVITY_EVENT = 'vox://agent-events';
export function listenActivity(onEvent: (e: { id: number; timestamp_ms: number; kind: { type: string; [k: string]: unknown } }) => void): Promise<UnlistenFn> {
  return listen(SUBAGENT_ACTIVITY_EVENT, (e) => onEvent(e.payload as { id: number; timestamp_ms: number; kind: { type: string; [k: string]: unknown } }));
}
