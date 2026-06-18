import { createStore } from 'zustand/vanilla';

export interface AgentState {
  x: number;
  y: number;
  energy: number;
  mood: string;
}

export interface LudusStoreState {
  agents: Record<string, AgentState>;
  updateAgent: (id: string, updates: Partial<AgentState>) => void;
}

export const useLudusStore = createStore<LudusStoreState>((set) => ({
  agents: {},
  updateAgent: (id, updates) =>
    set((state) => {
      const current = state.agents[id] || { x: 0, y: 0, energy: 100, mood: 'Happy' };
      return {
        agents: {
          ...state.agents,
          [id]: { ...current, ...updates },
        },
      };
    }),
}));
