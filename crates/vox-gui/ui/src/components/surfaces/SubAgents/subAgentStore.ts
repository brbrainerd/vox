import { create } from 'zustand';
import type { SubAgentNode } from './types';

export interface ActivityEvent { id: number; timestamp_ms: number; kind: { type: string; [k: string]: unknown }; }

interface SubAgentState {
  tree: SubAgentNode[];
  expanded: Set<string>;
  selectedWindowId: string | null;
  eventsByWindow: Record<string, ActivityEvent[]>;
  setTree: (t: SubAgentNode[]) => void;
  toggleExpand: (id: string) => void;
  select: (id: string) => void;
  pushEvent: (windowId: string, e: ActivityEvent) => void;
  reset: () => void;
}

export const useSubAgentStore = create<SubAgentState>((set) => ({
  tree: [], expanded: new Set(), selectedWindowId: null, eventsByWindow: {},
  setTree: (t) => set({ tree: t }),
  toggleExpand: (id) => set((s) => {
    const next = new Set(s.expanded);
    next.has(id) ? next.delete(id) : next.add(id);
    return { expanded: next };
  }),
  select: (id) => set({ selectedWindowId: id }),
  pushEvent: (windowId, e) => set((s) => {
    const prev = s.eventsByWindow[windowId] ?? [];
    const next = [...prev, e].slice(-200);
    return { eventsByWindow: { ...s.eventsByWindow, [windowId]: next } };
  }),
  reset: () => set({ tree: [], expanded: new Set(), selectedWindowId: null, eventsByWindow: {} }),
}));
