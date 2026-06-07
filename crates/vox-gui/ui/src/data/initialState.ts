import { DashboardData } from '../types/dashboard';

export const INITIAL_DATA: DashboardData = {
  // Honest empty seeds — populated from the live orchestrator status (App.tsx
  // applyStatus / vox://orch-status). No fabricated peers or KPI values render
  // before real data arrives; the Sidebar/TopHud show empty states until then.
  peers: [],
  kpis: {
    budgetBurn: { label: "Budget Burn", value: 0, cap: 20.0, spark: [0, 0, 0, 0, 0, 0, 0] },
    mesh: { label: "Mesh", value: "0 GB/s", cap: 10, spark: [0, 0, 0, 0, 0, 0, 0] },
  },
  agents: [],
  stream: [],
  alerts: [],
  contextChips: [],
  skills: [],
  graph: {
    nodes: [
      { id: "ROOT", label: "Orchestrator", phase: "Root", x: 0.50, y: 0.42 },
    ],
    edges: [],
  },
};

export const INITIAL_KPIS = {
    activeAgents: { value: 0, delta: 0, spark: [0, 0, 0, 0, 0] },
    queueDepth: { value: 0, delta: 0, spark: [0, 0, 0, 0, 0] },
    budgetBurn: { value: 0, cap: 50.0, delta: 0, spark: [0, 0, 0, 0, 0] },
    mesh: { value: "0 MB/s", unit: "MB/s", delta: 0, spark: [0, 0, 0, 0, 0], peers: 0, vramGb: 0 },
};
