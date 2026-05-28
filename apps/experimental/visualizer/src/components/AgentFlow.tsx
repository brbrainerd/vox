import React, { useMemo, useCallback } from 'react';
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
  Position
} from '@xyflow/react';
import { Activity, CheckCircle2, Clock, AlertCircle } from 'lucide-react';
import '@xyflow/react/dist/style.css';

// Custom Node for Agent Tasks
const TaskNode = ({ data }: any) => {
  const statusColor = data.status === 'Completed' ? 'emerald' : 
                     data.status === 'InProgress' ? 'blue' : 
                     data.status.startsWith('Failed') ? 'rose' : 
                     'zinc';

  return (
    <div className={`w-64 glass rounded-xl border border-${statusColor}-500/30 overflow-hidden glow-${statusColor}`}>
      <div className={`h-1 bg-${statusColor}-500 ${data.status === 'InProgress' ? 'animate-pulse' : ''}`} />
      <div className="p-4">
        <div className="flex justify-between items-start mb-3">
          <span className={`text-[10px] font-bold uppercase tracking-widest text-${statusColor}-500`}>{data.id}</span>
          {data.status === 'Completed' ? <CheckCircle2 size={12} className="text-emerald-500" /> : 
           data.status === 'InProgress' ? <Activity size={12} className="text-blue-500 animate-pulse" /> : 
           data.status.startsWith('Failed') ? <AlertCircle size={12} className="text-rose-500" /> :
           <Clock size={12} className="text-zinc-500" />}
        </div>
        <p className="text-xs font-semibold text-white/90 leading-relaxed mb-4">{data.description}</p>
        <div className="flex items-center justify-between">
           <span className="text-[10px] px-2 py-0.5 rounded bg-white/5 border border-white/10 text-zinc-400 font-mono italic">@{data.agent_id}</span>
           <span className="text-[10px] text-zinc-500 font-mono">{data.priority}</span>
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

export const AgentFlow = ({ tasks = [] }: { tasks: any[] }) => {
  const initialNodes = useMemo(() => tasks.map((t, idx) => ({
    id: t.id.toString(),
    type: 'task',
    position: { x: (idx % 3) * 300, y: Math.floor(idx / 3) * 200 },
    data: { ...t, agent_id: 'A-01' }, // Dummy agent id for now
  })), [tasks]);

  const initialEdges = useMemo(() => {
    const edges: any[] = [];
    tasks.forEach(t => {
      if (t.depends_on) {
        t.depends_on.forEach((dep: any) => {
          edges.push({
            id: `e-${dep}-${t.id}`,
            source: dep.toString(),
            target: t.id.toString(),
            type: 'smoothstep',
            animated: t.status === 'InProgress',
            style: { stroke: '#3b82f6', strokeWidth: 2, opacity: 0.4 },
            markerEnd: { type: MarkerType.ArrowClosed, color: '#3b82f6' },
          });
        });
      }
    });
    return edges;
  }, [tasks]);

  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  const onConnect = useCallback((params: any) => setEdges((eds) => addEdge(params, eds)), []);

  return (
    <div className="h-full w-full bg-[#09090b] relative">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        nodeTypes={nodeTypes}
        fitView
      >
        <Background color="#27272a" gap={20} />
        <Controls />
        <Panel position="top-right" className="glass p-4 rounded-xl border border-white/5 flex flex-col gap-2">
           <div className="flex items-center gap-2">
             <div className="w-2 h-2 rounded-full bg-blue-500" />
             <span className="text-[10px] text-zinc-400 font-bold uppercase">In Progress</span>
           </div>
           <div className="flex items-center gap-2">
             <div className="w-2 h-2 rounded-full bg-emerald-500" />
             <span className="text-[10px] text-zinc-400 font-bold uppercase">Completed</span>
           </div>
           <div className="flex items-center gap-2">
             <div className="w-2 h-2 rounded-full bg-zinc-500" />
             <span className="text-[10px] text-zinc-400 font-bold uppercase">Queued</span>
           </div>
        </Panel>
      </ReactFlow>
    </div>
  );
};
