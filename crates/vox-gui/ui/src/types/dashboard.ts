import { CommandCatalogEntry } from './catalog';

export interface Peer {
  id: string;
  name: string;
  backend: string;
  online: boolean;
  vram_gb?: number;
  tok_per_sec?: number;
}

export interface KPI {
  label: string;
  value: number;
  cap: number;
  spark: number[];
}

export interface Agent {
  id: string;
  codename: string;
  phase: string;
  progress: number;
  task: string;
  cost: number;
  budget: number;
  eta: string;
  skill?: string;
}

export interface StreamItem {
  id: string;
  kind: 'validated' | 'in-progress' | 'doubted' | 'speculative' | 'system' | 'agent';
  tag: string;
  title: string;
  body: string;
  ts: string;
  metadata?: Record<string, any>;
}

export interface LudusAlert {
  id: string;
  level: 'ok' | 'warn' | 'info' | 'error';
  title: string;
  body: string;
}

export interface GraphNode {
  id: string;
  label: string;
  phase: string;
  x: number;
  y: number;
}

export interface GraphEdge {
  from: string;
  to: string;
  flow: number;
}

export interface AgentGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface DashboardData {
  peers: Peer[];
  kpis: {
    budgetBurn: KPI;
    mesh: KPI;
  };
  agents: Agent[];
  stream: StreamItem[];
  alerts: LudusAlert[];
  contextChips: string[];
  skills: CommandCatalogEntry[];
  intentions: any[];
  /** Optional pre-computed agent topology graph. AgentFlow will generate one from
   *  live agent data if this is absent. */
  graph?: AgentGraph;
}
