import React, { useState } from 'react';
import { Glass } from '../../ui/Glass';
import { Pill, PHASE_TONE, PhaseKind } from '../../ui/Pill';
import { Agent } from '../../../types/dashboard';
import { phaseStroke, viz } from '../../../lib/visualTokens';

interface GraphNode {
  id: string;
  label: string;
  phase: string;
  x: number;
  y: number;
}

interface GraphEdge {
  from: string;
  to: string;
  flow: number;
}

interface AgentGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

interface AgentFlowProps {
  agents: Agent[];
  graph?: AgentGraph;
  onSelect?: (id: string) => void;
  selectedId?: string;
}

function Legend({ color, label }: { color: string; label: string }) {
  return (
    <span className="flex items-center gap-1.5 text-text-muted">
      <span className={`size-1.5 rounded-full ${color}`} />
      <span className="text-[10px]">{label}</span>
    </span>
  );
}

/** Build a synthetic graph from live agent data when no pre-computed graph is provided. */
function buildGraphFromAgents(agents: Agent[]): AgentGraph {
  const W = 1;
  const H = 1;
  const root: GraphNode = { id: 'ROOT', label: 'Orchestrator', phase: 'Root', x: 0.5, y: 0.42 };

  // Distribute agents in a circle around the root.
  const nodes: GraphNode[] = [root];
  const edges: GraphEdge[] = [];
  const radius = 0.32;

  agents.forEach((a, i) => {
    const angle = (i / agents.length) * Math.PI * 2 - Math.PI / 2;
    nodes.push({
      id: a.id,
      label: a.codename,
      phase: a.phase,
      x: Math.min(0.9, Math.max(0.1, 0.5 + Math.cos(angle) * radius)),
      y: Math.min(0.85, Math.max(0.1, 0.42 + Math.sin(angle) * radius)),
    });
    edges.push({ from: 'ROOT', to: a.id, flow: a.progress ?? 0.5 });
  });

  // Wire co-dependent agents (e.g. verifying → verified).
  agents.forEach((a) => {
    if (a.phase === 'Verifying') {
      const compiler = agents.find(x => x.skill === 'compiler' && x.id !== a.id);
      if (compiler) {
        edges.push({ from: a.id, to: compiler.id, flow: 0.4 });
      }
    }
  });

  return { nodes, edges };
}

function AgentInspector({ node, agent }: { node: GraphNode; agent?: Agent }) {
  return (
    <div className="absolute right-5 top-5 w-72 rounded-xl border border-border-subtle bg-bg-base/85 p-4 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]">
      <div className="flex items-center justify-between">
        <div>
          <div className="font-display text-[14px] text-text-primary">{node.label}</div>
          <div className="font-mono text-[10px] text-text-muted">{node.id}</div>
        </div>
        <Pill phase={node.phase as PhaseKind} />
      </div>
      {agent ? (
        <>
          <div className="mt-3 text-[12px] text-text-secondary leading-relaxed">{agent.task}</div>
          <div className="mt-3 grid grid-cols-2 gap-2 text-[10px]">
            {[
              ['Cost',   `$${(agent.cost ?? 0).toFixed(2)}`],
              ['Budget', agent.budget != null ? `$${agent.budget.toFixed(2)}` : '—'],
              ['ETA',    agent.eta],
              ['Skill',  agent.skill ?? '—'],
            ].map(([label, value]) => (
              <div key={label} className="rounded-md border border-border-subtle bg-overlay-subtle px-2 py-1.5">
                <div className="text-[9px] uppercase tracking-widest text-text-muted">{label}</div>
                <div className="mt-0.5 font-mono text-[11px] text-text-secondary">{value}</div>
              </div>
            ))}
          </div>
          {/* Progress bar */}
          <div className="mt-3">
            <div className="flex items-center justify-between text-[9px] uppercase tracking-widest text-text-muted">
              <span>Progress</span>
              <span className="font-mono text-text-secondary">{Math.round(agent.progress * 100)}%</span>
            </div>
            <div
              className="mt-1 h-1 overflow-hidden rounded-full bg-overlay-subtle"
              role="progressbar"
              aria-label={`${node.label} progress`}
              aria-valuenow={Math.round(agent.progress * 100)}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <div
                className="h-full bg-linear-to-r from-violet-400 to-emerald-400 transition-all duration-700"
                style={{ width: `${agent.progress * 100}%` }}
              />
            </div>
          </div>
        </>
      ) : (
        <div className="mt-3 text-[11px] text-text-muted">
          Root coordinator · routes all task fan-out via file-affinity policy.
        </div>
      )}
    </div>
  );
}

export function AgentFlow({ agents, graph, onSelect, selectedId }: AgentFlowProps) {
  const [sel, setSel] = useState<string>(selectedId ?? 'ROOT');

  const g = graph ?? buildGraphFromAgents(agents);
  const W = 1200;
  const H = 640;

  const nodeAt = (id: string) => g.nodes.find(n => n.id === id);
  const pos = (n: GraphNode) => ({ x: n.x * W, y: n.y * H });

  const selectedNode = nodeAt(sel);
  const selectedAgent = agents.find(a => a.id === sel);

  return (
    <Glass className="relative overflow-hidden p-0">
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 border-b border-border-subtle px-5 py-3">
        <div className="min-w-0">
          <h2 className="font-display text-[16px] font-semibold tracking-tight text-text-primary">
            Mind-Map · Agent Shards
          </h2>
          <p className="text-[11px] text-text-muted">
            Topology of the active agent graph · click a shard to inspect
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          {/* Was bg-cyan-400 — the one blue swatch in a legend otherwise
              built from the app's brass/violet/emerald accent palette.
              Matches Pill.tsx PHASE_TONE.Planning, which now shares
              Executing's brass tone. */}
          <Legend color="bg-brass"      label="Planning" />
          <Legend color="bg-brass"      label="Executing" />
          <Legend color="bg-violet-400" label="Verifying" />
          <Legend color="bg-text-muted"   label="Paused" />
          <Legend color="bg-emerald-400" label="Validated" />
        </div>
      </div>

      <div className="relative h-[600px] w-full">
        {/* Grid background */}
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(255,255,255,0.06)_1px,transparent_1px)] bg-size-[32px_32px] opacity-50" />

        <svg
          viewBox={`0 0 ${W} ${H}`}
          className="absolute inset-0 h-full w-full"
          preserveAspectRatio="xMidYMid meet"
        >
          <defs>
            <linearGradient id="ag-edge-grad" x1="0" x2="1">
              {/* Was a brass→cyan400 gradient — the edge lines carried the
                  same stray blue as the Planning/Active phase tones. Now a
                  brass→brass (soft fade) gradient, consistent with the rest
                  of the mind-map's accent color. */}
              <stop offset="0" stopColor="rgb(var(--brass) / 0.7)" />
              <stop offset="1" stopColor="rgb(var(--brass) / 0.35)" />
            </linearGradient>
            <radialGradient id="ag-root-glow" cx="0.5" cy="0.5" r="0.5">
              <stop offset="0%"   stopColor={viz.white}    stopOpacity="0.9" />
              <stop offset="60%"  stopColor="rgb(var(--brass) / 0.3)" />
              <stop offset="100%" stopColor="rgb(var(--brass) / 0)" />
            </radialGradient>
            <filter id="ag-soft-glow">
              <feGaussianBlur stdDeviation="6" />
            </filter>
          </defs>

          {/* Edges */}
          {g.edges.map((e, i) => {
            const a = nodeAt(e.from);
            const b = nodeAt(e.to);
            if (!a || !b) return null;
            const pa = pos(a);
            const pb = pos(b);
            const mx = (pa.x + pb.x) / 2;
            const my = (pa.y + pb.y) / 2 + 40;
            const d = `M ${pa.x} ${pa.y} Q ${mx} ${my} ${pb.x} ${pb.y}`;
            return (
              <g key={i}>
                <path d={d} stroke="rgba(255,255,255,0.06)" strokeWidth="1" fill="none" />
                <path
                  d={d}
                  stroke="url(#ag-edge-grad)"
                  strokeWidth={1.2 + e.flow * 1.6}
                  strokeOpacity="0.55"
                  fill="none"
                  strokeDasharray="6 8"
                  className="animate-vox-dash"
                />
              </g>
            );
          })}

          {/* Nodes */}
          {g.nodes.map(n => {
            const { x, y } = pos(n);
            const isRoot = n.id === 'ROOT';
            const stroke = phaseStroke(n.phase);
            const r = isRoot ? 44 : 32;
            const isSel = sel === n.id;
            return (
              <g
                key={n.id}
                role="button"
                tabIndex={0}
                aria-label={`${n.label} — ${n.phase}`}
                aria-pressed={isSel}
                onClick={() => { setSel(n.id); onSelect?.(n.id); }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    setSel(n.id);
                    onSelect?.(n.id);
                  }
                }}
                className="cursor-pointer focus:outline-hidden focus-visible:[outline:2px_solid_rgb(var(--brass))]"
              >
                {isRoot && <circle cx={x} cy={y} r={100} fill="url(#ag-root-glow)" />}
                <circle
                  cx={x} cy={y} r={r + 14}
                  fill={stroke} fillOpacity={isSel ? 0.18 : 0.08}
                  filter="url(#ag-soft-glow)"
                />
                <circle
                  cx={x} cy={y} r={r}
                  fill={viz.void}
                  stroke={stroke}
                  strokeOpacity={isSel ? 1 : 0.6}
                  strokeWidth={isSel ? 2 : 1.25}
                />
                <circle
                  cx={x} cy={y} r={r - 6}
                  fill="none"
                  stroke={stroke}
                  strokeOpacity="0.15"
                  strokeDasharray="2 4"
                  className="animate-vox-spin-slow"
                  style={{ transformOrigin: `${x}px ${y}px` }}
                />
                <text
                  x={x} y={y - 2}
                  textAnchor="middle"
                  fill={viz.zinc100}
                  fontSize={isRoot ? 13 : 11}
                  fontWeight="600"
                  fontFamily="'Space Grotesk', sans-serif"
                >
                  {n.label}
                </text>
                <text
                  x={x} y={y + 12}
                  textAnchor="middle"
                  fill={stroke}
                  fontSize="9"
                  fontFamily="'JetBrains Mono', monospace"
                  style={{ textTransform: 'uppercase', letterSpacing: '0.12em' }}
                >
                  {n.phase}
                </text>
              </g>
            );
          })}
        </svg>

        {/* Inspector overlay */}
        {selectedNode && (
          <AgentInspector node={selectedNode} agent={selectedAgent} />
        )}
      </div>
    </Glass>
  );
}
