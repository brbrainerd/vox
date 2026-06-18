import { createStore } from 'zustand/vanilla';

export type MoodType = 'Happy' | 'Tired' | 'Sad' | 'Excited' | 'Exhausted';

export interface AgentState {
  x: number;
  y: number;
  energy: number;
  mood: MoodType;
}

export interface BuildingState {
  x: number;
  y: number;
  warnings: number;
  errors: number;
}

export interface AgentTask {
  taskId: string;
  filePath: string;
  status: string;
}

export interface LudusStoreState {
  agents: Record<string, AgentState>;
  buildings: Record<string, BuildingState>;
  focusedFile: string | null;
  agentTasks: Record<string, AgentTask>;
  updateAgent: (id: string, updates: Partial<AgentState>) => void;
  updateBuilding: (filePath: string, updates: Partial<BuildingState>) => void;
  setFocusedFile: (filePath: string | null) => void;
  updateAgentTask: (agentId: string, task: AgentTask | null) => void;
  reset: () => void;
}

export const useLudusStore = createStore<LudusStoreState>((set) => ({
  agents: {},
  buildings: {},
  focusedFile: null,
  agentTasks: {},
  updateAgent: (id, updates) =>
    set((state) => {
      const current = state.agents[id] || { x: 0, y: 0, energy: 100, mood: 'Happy' as MoodType };
      return {
        agents: {
          ...state.agents,
          [id]: { ...current, ...updates },
        },
      };
    }),
  updateBuilding: (filePath, updates) =>
    set((state) => {
      const current = state.buildings[filePath] || { x: 0, y: 0, warnings: 0, errors: 0 };
      return {
        buildings: {
          ...state.buildings,
          [filePath]: { ...current, ...updates },
        },
      };
    }),
  setFocusedFile: (filePath) => set({ focusedFile: filePath }),
  updateAgentTask: (agentId, task) =>
    set((state) => {
      const next = { ...state.agentTasks };
      if (task === null) {
        delete next[agentId];
      } else {
        next[agentId] = task;
      }
      return { agentTasks: next };
    }),
  reset: () => set({ agents: {}, buildings: {}, focusedFile: null, agentTasks: {} }),
}));
