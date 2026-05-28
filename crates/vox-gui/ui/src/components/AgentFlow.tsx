import React, { useMemo, useCallback, useState, useEffect } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  Panel,
  useNodesState,
  useEdgesState,
  addEdge,
  MarkerType,
  Handle,
  Position,
} from '@xyflow/react';
import { Activity, CheckCircle2, Clock, AlertCircle, Ban, ShieldAlert } from 'lucide-react';
import '@xyflow/react/dist/style.css';
import { voxTransport } from '../transport';

const TaskNode = ({ data }: { data: Record<string, unknown> }) => {
  let statusColor = 'zinc';
  let nodeClass = '';
  let Icon: React.ComponentType<{ size?: number; className?: string }> = Clock;
  let iconClass = 'text-zinc-500';

  if (data.status === 'Completed') {
    statusColor = 'emerald';
    nodeClass = 'node-completed';
    Icon = CheckCircle2;
    iconClass = 'text-emerald-500';
  } else if (data.status === 'InProgress') {
    if (data.mode === 'Fast') statusColor = 'rose';
    else if (data.mode === 'Verbose') statusColor = 'blue';
    else if (data.mode === 'Precision') statusColor = 'violet';
    else statusColor = 'emerald';
    Icon = Activity;
    iconClass = `text-${statusColor}-500 animate-pulse`;
  } else if (typeof data.status === 'string' && data.status.startsWith('Failed')) {
    statusColor = 'rose';
    nodeClass = 'node-failed';
    Icon = AlertCircle;
    iconClass = 'text-rose-500';
  } else if (data.status === 'Cancelled') {
    statusColor = 'zinc';
    nodeClass = 'node-cancelled';
    Icon = Ban;
  } else if (data.status === 'Blocked') {
    statusColor = 'amber';
    nodeClass = 'node-blocked';
    Icon = ShieldAlert;
    iconClass = 'text-amber-500';
  } else if (data.status === 'Doubted' || (typeof data.status === 'string' && data.status.includes('Doubted'))) {
    statusColor = 'rose';
    nodeClass = 'node-doubted border-double border-4';
    Icon = AlertCircle;
    iconClass = 'text-rose-500 animate-pulse';
  } else if (data.status === 'Validated') {
    statusColor = 'emerald';
    nodeClass = 'node-validated';
    Icon = CheckCircle2;
    iconClass = 'text-emerald-500';
  } else if (data.status === 'Overruled') {
    statusColor = 'rose';
    nodeClass = 'node-overruled shadow-[0_0_20px_rgba(239,68,68,0.4)]';
    Icon = ShieldAlert;
    iconClass = 'text-rose-600';
  }

  return (
    <div className={`w-64 glass rounded-xl border border-${statusColor}-500/30 overflow-hidden glow-${statusColor} ${nodeClass}`}>
      <div className={`h-1 bg-${statusColor}-500 ${data.status === 'InProgress' ? 'animate-pulse' : ''}`} />
      <div className="p-4">
        <div className="flex justify-between items-start mb-3">
          <span className={`text-[10px] font-bold uppercase tracking-widest text-${statusColor}-500`}>{String(data.id)}</span>
          <Icon size={12} className={iconClass} />
        </div>
        <p className="text-xs font-semibold text-white/90 leading-relaxed mb-4">{String(data.description ?? '')}</p>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            <span className="text-[10px] px-2 py-0.5 rounded bg-white/5 border border-white/10 text-zinc-400 font-mono italic">@{String(data.agent_id ?? '')}</span>
            {data.active_skill && (
              <span className="text-[9px] font-bold uppercase text-emerald-400/80 flex items-center gap-1 px-1.5 py-0.5 rounded bg-emerald-400/10 border border-emerald-400/20">
                <Activity size={10} /> {String(data.active_skill)}
              </span>
            )}
            {data.current_phase && (
              <span className="text-[9px] font-bold uppercase text-cyan-400/80 flex items-center gap-1 px-1.5 py-0.5 rounded bg-cyan-400/10 border border-cyan-400/20">
                {String(data.current_phase)}
              </span>
            )}
          </div>
          <div className="flex flex-col items-end gap-1">
            <span className="text-[10px] text-zinc-500 font-mono">{String(data.priority ?? '')}</span>
            {data.mode ? <span className={`text-[9px] font-bold uppercase text-${statusColor}-500 opacity-80`}>{String(data.mode)}</span> : null}
            {data.status === 'Completed' && (
                <button
                    className="text-[8px] font-bold uppercase tracking-widest px-1.5 py-0.5 rounded border border-rose-500/50 text-rose-500/80 hover:bg-rose-500 hover:text-white transition-all mt-1"
                    onClick={(e) => {
                        e.stopPropagation();
                        voxTransport.callTool('vox_doubt_task', { task_id: String(data.id) });
                    }}
                >
                    🚩 Doubt
                </button>
            )}
            {(String(data.status).includes('Failed') || String(data.status) === 'Doubted') && (
                <button
                    className="text-[8px] font-bold uppercase tracking-widest px-1.5 py-0.5 rounded border border-emerald-500/50 text-emerald-500/80 hover:bg-emerald-500 hover:text-white transition-all mt-1"
                    onClick={(e) => {
                        e.stopPropagation();
                        voxTransport.callTool('vox_overrule_task', { task_id: String(data.id), reason: 'Human overrule from GUI' });
                    }}
                >
                    ✅ Overrule
                </button>
            )}
          </div>
        </div>
      </div>
      <Handle type="target" position={Position.Top} className="opacity-0" />
      <Handle type="source" position={Position.Bottom} className="opacity-0" />
    </div>
  );
};

const nodeTypes = {
  task: TaskNode,
};

function collectNumericAgentIds(tasks: Array<Record<string, unknown>>): number[] {
  const s = new Set<number>();
  for (const t of tasks) {
    const raw = t.agent_id;
    if (typeof raw === 'number' && !Number.isNaN(raw)) {
      s.add(raw);
      continue;
    }
    const m = String(raw ?? '').match(/(\d+)/);
    if (m) s.add(Number(m[1]));
  }
  return [...s].sort((a, b) => a - b);
}

export const AgentFlow = ({ tasks = [], capabilities = null }: { tasks: unknown[]; capabilities?: unknown | null }) => {
  const taskRows = tasks as Array<Record<string, unknown>>;
  const numericIds = useMemo(() => collectNumericAgentIds(taskRows), [taskRows]);

  const [agentControlId, setAgentControlId] = useState(0);
  const [manualId, setManualId] = useState('');
  const [budgetUsd, setBudgetUsd] = useState(0);

  useEffect(() => {
    if (numericIds.length > 0 && !numericIds.includes(agentControlId)) {
      setAgentControlId(numericIds[0]);
    }
  }, [numericIds, agentControlId]);

  const resolvedAgentId = manualId.trim() !== '' ? Number.parseInt(manualId, 10) : agentControlId;
  const canSend = Number.isFinite(resolvedAgentId) && resolvedAgentId >= 0;
  const mcpOk = Boolean((capabilities as { mcpConnected?: boolean } | null)?.mcpConnected);

  const initialNodes = useMemo(
    () =>
      taskRows.map((t, idx) => ({
        id: String(t.id ?? idx),
        type: 'task' as const,
        position: { x: (idx % 3) * 300, y: Math.floor(idx / 3) * 200 },
        data: { ...t, agent_id: t.agent_id ?? '—', mode: t.mode ?? 'Efficient' },
      })),
    [taskRows],
  );

  const initialEdges = useMemo(() => {
    const edges: Array<Record<string, unknown>> = [];
    taskRows.forEach((t) => {
      const deps = t.depends_on as unknown[] | undefined;
      if (!deps) return;
      deps.forEach((dep: unknown) => {
        let edgeClass = '';
        let edgeColor = '#3b82f6';

        if (t.status === 'InProgress') {
          if (t.mode === 'Efficient') {
            edgeClass = 'mode-efficient';
            edgeColor = '#4ADE80';
          } else if (t.mode === 'Fast') {
            edgeClass = 'mode-fast';
            edgeColor = '#EF4444';
          } else if (t.mode === 'Verbose') {
            edgeClass = 'mode-verbose';
            edgeColor = '#60A5FA';
          } else if (t.mode === 'Precision') {
            edgeClass = 'mode-precision';
            edgeColor = '#A78BFA';
          }
        } else if (t.status === 'Completed') {
          edgeColor = '#10b981';
        } else if (t.status === 'Blocked') {
          edgeColor = '#f59e0b';
        } else if (t.status === 'Cancelled') {
          edgeColor = '#71717a';
        }

        edges.push({
          id: `e-${String(dep)}-${String(t.id)}`,
          source: String(dep),
          target: String(t.id),
          type: t.mode === 'Precision' ? 'straight' : 'smoothstep',
          animated: t.status === 'InProgress',
          className: edgeClass,
          style: {
            stroke: edgeColor,
            strokeWidth: t.mode === 'Verbose' ? 4 : 2,
            opacity: t.status === 'Cancelled' ? 0.2 : 0.6,
          },
          markerEnd: t.mode === 'Precision' ? undefined : { type: MarkerType.ArrowClosed, color: edgeColor },
        });
      });
    });
    return edges;
  }, [taskRows]);

  const [nodes, , onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  useEffect(() => {
    setEdges(initialEdges);
  }, [initialEdges, setEdges]);

  const onConnect = useCallback((params: Parameters<typeof addEdge>[0]) => setEdges((eds) => addEdge(params, eds)), [setEdges]);

  const lifecycleBtn =
    'text-[10px] font-bold uppercase tracking-widest px-3 py-2 rounded-lg border transition-opacity disabled:opacity-40';
  const btnStyle: React.CSSProperties = {
    borderColor: 'var(--vscode-panel-border)',
    background: 'var(--vscode-button-secondaryBackground)',
    color: 'var(--vscode-button-secondaryForeground)',
  };

  return (
    <div
      className="h-full w-full relative"
      style={{ background: 'var(--vscode-sideBar-background, #09090b)' }}
    >
      <ReactFlow nodes={nodes} edges={edges} onNodesChange={onNodesChange} onEdgesChange={onEdgesChange} onConnect={onConnect} nodeTypes={nodeTypes} fitView>
        <Background color="#27272a" gap={20} />
        <Controls className="!bg-[#09090b] !border-white/10 [&>button]:!bg-transparent [&>button]:!border-white/10 [&>button>svg]:!fill-white/50" />
        <Panel position="top-right" className="glass p-4 rounded-xl border border-white/5 flex flex-col gap-3 max-w-[220px]">
          <div className="text-[10px] font-bold uppercase tracking-widest text-zinc-500 border-b border-white/5 pb-2">Orchestrator agent</div>
          <p className="text-[9px] text-zinc-500 leading-snug">Uses numeric `agent_id` from tasks or manual entry. Requires MCP tools on the server.</p>
          {!mcpOk ? <p className="text-[9px] text-amber-600">MCP disconnected — actions may fail.</p> : null}
          <div className="flex flex-col gap-1">
            <label className="text-[9px] text-zinc-500">From queue</label>
            <select
              className="text-[11px] bg-zinc-900 border border-white/10 rounded px-2 py-1 text-zinc-200"
              value={numericIds.includes(agentControlId) ? agentControlId : numericIds[0] ?? 0}
              onChange={(e) => setAgentControlId(Number(e.target.value))}
              disabled={numericIds.length === 0}
            >
              {numericIds.length === 0 ? <option value={0}>— no ids in tasks —</option> : null}
              {numericIds.map((id) => (
                <option key={id} value={id}>
                  {id}
                </option>
              ))}
            </select>
          </div>
          <div className="flex flex-col gap-1">
            <label className="text-[9px] text-zinc-500">Manual id</label>
            <input
              className="text-[11px] bg-zinc-900 border border-white/10 rounded px-2 py-1 text-zinc-200"
              placeholder="override"
              value={manualId}
              onChange={(e) => setManualId(e.target.value)}
            />
          </div>
          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              disabled={!canSend}
              className={lifecycleBtn}
              style={btnStyle}
              onClick={() => canSend && voxTransport.callTool('vox_pause_agent', { agent_id: String(resolvedAgentId) })}
            >
              Pause
            </button>
            <button
              type="button"
              disabled={!canSend}
              className={lifecycleBtn}
              style={btnStyle}
              onClick={() => canSend && voxTransport.callTool('vox_resume_agent', { agent_id: String(resolvedAgentId) })}
            >
              Resume
            </button>
            <button
              type="button"
              disabled={!canSend}
              className={lifecycleBtn}
              style={btnStyle}
              onClick={() => canSend && voxTransport.callTool('vox_drain_agent', { agent_id: String(resolvedAgentId) })}
            >
              Drain
            </button>
            <button
              type="button"
              disabled={!canSend}
              className={lifecycleBtn}
              style={btnStyle}
              onClick={() => canSend && voxTransport.callTool('vox_retire_agent', { agent_id: String(resolvedAgentId) })}
            >
              Retire
            </button>
          </div>
          <div className="flex flex-col gap-1 mt-2 border-t border-white/5 pt-2">
            <label className="text-[9px] text-zinc-500">Override Agent Dollar Cap ($)</label>
            <div className="flex items-center gap-2">
              <input
                className="text-[11px] bg-zinc-900 border border-white/10 rounded px-2 py-1 text-zinc-200 w-full"
                placeholder="e.g. 5.00"
                type="number"
                step="0.01"
                onChange={(e) => setBudgetUsd(parseFloat(e.target.value))}
              />
              <button
                type="button"
                disabled={!canSend || isNaN(budgetUsd)}
                className={lifecycleBtn}
                style={btnStyle}
                onClick={() => canSend && !isNaN(budgetUsd) && voxTransport.callTool('vox_set_budget', { max_cost_usd: budgetUsd })}
              >
                Set
              </button>
            </div>
          </div>
        </Panel>
        <Panel position="top-left" className="glass p-4 rounded-xl border border-white/5 flex flex-col gap-3">
          <div className="text-[10px] font-bold uppercase tracking-widest text-zinc-500 border-b border-white/5 pb-2">Execution modes</div>
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full shadow-[0_0_10px_rgba(74,222,128,0.5)] bg-emerald-400" />
            <span className="text-[10px] text-zinc-300 font-bold uppercase">Efficient</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full shadow-[0_0_10px_rgba(239,68,68,0.5)] bg-red-500" />
            <span className="text-[10px] text-zinc-300 font-bold uppercase">Fast</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full shadow-[0_0_10px_rgba(96,165,250,0.5)] bg-blue-400" />
            <span className="text-[10px] text-zinc-300 font-bold uppercase">Verbose</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full shadow-[0_0_10px_rgba(167,139,250,0.5)] bg-violet-400" />
            <span className="text-[10px] text-zinc-300 font-bold uppercase">Precision</span>
          </div>
        </Panel>
      </ReactFlow>
    </div>
  );
};
