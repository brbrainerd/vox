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
  /** Numeric for ratio bars (budget); a preformatted string for display-only KPIs (e.g. mesh "3.2 GB/s"). */
  value: number | string;
  cap: number;
  spark: number[];
}

/** Orchestrator-fed metric for dashboard chart widgets (queue depth, etc.). */
export interface DashboardMetricKpi {
  value: number;
  spark: number[];
}

export interface Agent {
  id: string;
  codename: string;
  phase: string;
  /** 0–1 when known; null → indeterminate progress UI. */
  progress: number | null;
  task: string;
  cost: number;
  /** Per-agent cap when daemon reports it; null → show em-dash, no fake bar. */
  budget: number | null;
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
  /** Numeric orchestrator task id — present only for task-related events; used to gate doubt/overrule controls. */
  taskId?: number;
  metadata?: Record<string, any>;
}

/** A chat-turn-visible event derived from a tool call's RESULT (never its
 *  arguments alone) — see Rust `turn_event_for_result`
 *  (`crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs`). Shape
 *  is intentionally open (`kind` is a free string, not a union) so an
 *  unrecognized future `kind` renders as a no-op chip instead of a crash —
 *  see `ChatTurnEventRow`. */
export interface TurnEventDto {
  kind: string;
  /** Present when `kind === 'skill_activated'`. Always a validated slug or
   *  the literal `"unknown"` — never raw, untrusted tool-call content. */
  skill_id?: string;
  [key: string]: unknown;
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
    queueDepth: DashboardMetricKpi;
  };
  agents: Agent[];
  stream: StreamItem[];
  alerts: LudusAlert[];
  contextChips: string[];
  skills: CommandCatalogEntry[];
  /** Optional pre-computed agent topology graph. AgentFlow will generate one from
   *  live agent data if this is absent. */
  graph?: AgentGraph;
}
