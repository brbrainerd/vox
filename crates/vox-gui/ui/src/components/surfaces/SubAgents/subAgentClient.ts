import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { SubAgentNode, ProjectionItem, ControlAction } from './types';

interface Envelope<T> { is_error: boolean; result: T; }
async function call<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  const env = (await invoke(cmd, args)) as Envelope<T>;
  if (env.is_error) throw new Error(`${cmd} failed`);
  return env.result;
}

export async function fetchTree(): Promise<SubAgentNode[]> {
  return (await call<{ nodes: SubAgentNode[] }>('subagent_tree', {})).nodes;
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
  return listen(SUBAGENT_ACTIVITY_EVENT, (e) => onEvent(e.payload as never));
}
