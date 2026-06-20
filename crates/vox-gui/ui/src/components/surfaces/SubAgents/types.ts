export interface ModelProfileLite { id: string; maxTokens: number; toolCapable: boolean; }
export type SubAgentStatus = 'running' | 'idle' | 'paused' | 'blocked' | 'done' | 'failed';
export type ItemFate = 'included' | 'summarized' | 'dropped' | 'on_demand';

export interface SubAgentNode {
  windowId: string;
  parentWindowId: string | null;
  title: string;
  skill: string | null;
  model: ModelProfileLite;
  status: SubAgentStatus;
  usedTokens: number;
  depth: number;
  children: SubAgentNode[];
}

export interface ProjectionItem {
  itemId: string;
  role: string;
  itemKind: string;
  preview: string;
  byteLen: number;
  tokenEstimate: number;
  pinned: boolean;
  fate: ItemFate;
}

export type ControlAction =
  | { kind: 'pause' } | { kind: 'resume' } | { kind: 'kill' }
  | { kind: 'overrule'; note: string }
  | { kind: 'set_budget'; maxTokens: number }
  | { kind: 'set_model'; modelId: string };

export interface FlatRow { windowId: string; depth: number; node: SubAgentNode; hasChildren: boolean; }

export function flattenTree(nodes: SubAgentNode[], expanded: Set<string>): FlatRow[] {
  const out: FlatRow[] = [];
  const walk = (list: SubAgentNode[]) => {
    for (const n of list) {
      out.push({ windowId: n.windowId, depth: n.depth, node: n, hasChildren: n.children.length > 0 });
      if (n.children.length && expanded.has(n.windowId)) walk(n.children);
    }
  };
  walk(nodes);
  return out;
}

export function tokenFate(used: number, max: number): 'ok' | 'warn' | 'over' {
  if (used > max) return 'over';
  if (used >= max * 0.85) return 'warn';
  return 'ok';
}
