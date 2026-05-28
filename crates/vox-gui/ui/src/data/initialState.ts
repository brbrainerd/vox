import { DashboardData } from '../types/dashboard';

export const INITIAL_DATA: DashboardData = {
  peers: [
    { id: "P-01", name: "Hecate", backend: "candle-metal", online: true },
    { id: "P-02", name: "Moros", backend: "burn-cuda", online: true },
    { id: "P-03", name: "Nyx", backend: "candle-cpu", online: false },
  ],
  kpis: {
    budgetBurn: { label: "Budget Burn", value: 1.42, cap: 20.0, spark: [1, 2, 1.5, 3, 2.5, 4, 3.8] },
    mesh: { label: "Mesh", value: "3.2 GB/s", cap: 10, spark: [2, 3, 2.8, 3.5, 3.2, 3.8, 3.2] },
  },
  agents: [],
  stream: [],
  alerts: [],
  contextChips: [],
  skills: [],
  intentions: [
    { id: "B-01", parent: "R-COORD", branch: "Optimistic",   phase: "Active",      conf: 0.82, note: "Prefers local inference for low-latency tasks." },
    { id: "B-02", parent: "R-COORD", branch: "Conservative", phase: "Validated",   conf: 0.95, note: "Requires multi-agent consensus for file mutations." },
    { id: "B-03", parent: "R-COORD", branch: "Speculative",  phase: "Speculative", conf: 0.42, note: "Testing experimental branch: crypto-harden." },
  ],
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
