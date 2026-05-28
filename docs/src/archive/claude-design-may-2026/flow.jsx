// AgentFlow — custom mind-map / shard graph

function AgentFlow({ graph, onSelect, selectedId }) {
  // SVG canvas 1200x720 normalized; nodes positioned by graph.x/y in 0..1
  const W = 1200, H = 720;
  const nodeAt = (id) => graph.nodes.find(n => n.id === id);
  const pos = (n) => ({ x: n.x * W, y: n.y * H });

  return (
    <Glass className="relative overflow-hidden p-0">
      <div className="flex items-center justify-between border-b border-white/5 px-5 py-3">
        <div>
          <h2 className="font-display text-[16px] font-semibold tracking-tight text-zinc-100">Mind-Map · Agent Shards</h2>
          <p className="text-[11px] text-zinc-500">Topology of the active agent graph · click a shard to inspect</p>
        </div>
        <div className="flex items-center gap-3 text-[10px]">
          <Legend color="bg-cyan-400" label="Planning" />
          <Legend color="bg-brass" label="Executing" />
          <Legend color="bg-violet-400" label="Verifying" />
          <Legend color="bg-zinc-500" label="Paused" />
        </div>
      </div>
      <div className="relative h-[640px] w-full">
        <div className="absolute inset-0 [background-image:radial-gradient(circle_at_center,rgba(255,255,255,0.06)_1px,transparent_1px)] [background-size:32px_32px] opacity-50" />
        <svg viewBox={`0 0 ${W} ${H}`} className="absolute inset-0 h-full w-full" preserveAspectRatio="xMidYMid meet">
          <defs>
            <linearGradient id="edge-grad" x1="0" x2="1">
              <stop offset="0" stopColor="#d4af37" stopOpacity="0.7"/>
              <stop offset="1" stopColor="#22d3ee" stopOpacity="0.7"/>
            </linearGradient>
            <radialGradient id="root-glow" cx="0.5" cy="0.5" r="0.5">
              <stop offset="0%" stopColor="#fff" stopOpacity="0.9"/>
              <stop offset="60%" stopColor="#d4af37" stopOpacity="0.3"/>
              <stop offset="100%" stopColor="#d4af37" stopOpacity="0"/>
            </radialGradient>
            <filter id="soft-glow">
              <feGaussianBlur stdDeviation="6"/>
            </filter>
          </defs>

          {/* Edges with animated dashes */}
          {graph.edges.map((e, i) => {
            const a = pos(nodeAt(e.from)), b = pos(nodeAt(e.to));
            const mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2 + 40;
            const d = `M ${a.x} ${a.y} Q ${mx} ${my} ${b.x} ${b.y}`;
            return (
              <g key={i}>
                <path d={d} stroke="rgba(255,255,255,0.06)" strokeWidth="1" fill="none" />
                <path d={d} stroke="url(#edge-grad)" strokeWidth={1.2 + e.flow * 1.6} strokeOpacity="0.55" fill="none" strokeDasharray="6 8" className="animate-vox-dash" />
              </g>
            );
          })}

          {/* Nodes */}
          {graph.nodes.map(n => {
            const { x, y } = pos(n);
            const isRoot = n.id === "ROOT";
            const t = PHASE_TONE[n.phase] || PHASE_TONE.Paused;
            const stroke = n.phase === "Verifying" ? "#a78bfa" : n.phase === "Executing" ? "#d4af37" : n.phase === "Planning" ? "#22d3ee" : n.phase === "Root" ? "#ffffff" : "#71717a";
            const r = isRoot ? 44 : 32;
            const isSel = selectedId === n.id;
            return (
              <g key={n.id} onClick={() => onSelect(n.id)} className="cursor-pointer">
                {isRoot && <circle cx={x} cy={y} r="100" fill="url(#root-glow)" />}
                <circle cx={x} cy={y} r={r + 14} fill={stroke} fillOpacity={isSel ? 0.18 : 0.08} filter="url(#soft-glow)" />
                <circle cx={x} cy={y} r={r} fill="#0b0b0d" stroke={stroke} strokeOpacity={isSel ? 1 : 0.6} strokeWidth={isSel ? 2 : 1.25} />
                <circle cx={x} cy={y} r={r - 6} fill="none" stroke={stroke} strokeOpacity="0.15" strokeDasharray="2 4" className="animate-vox-spin-slow" style={{transformOrigin: `${x}px ${y}px`}}/>
                <text x={x} y={y - 2} textAnchor="middle" className="font-display fill-zinc-100" fontSize={isRoot ? 13 : 11} fontWeight="600">{n.label}</text>
                <text x={x} y={y + 12} textAnchor="middle" fill={stroke} fontSize="9" className="font-mono uppercase tracking-widest">{n.phase}</text>
              </g>
            );
          })}
        </svg>

        {/* Inspector overlay */}
        {selectedId && (() => {
          const n = nodeAt(selectedId);
          const agent = window.VOX_FIXTURES.agents.find(a => a.id === selectedId);
          if (!n) return null;
          return (
            <div className="absolute right-5 top-5 w-72 rounded-xl border border-white/10 bg-zinc-950/80 p-4 backdrop-blur-xl">
              <div className="flex items-center justify-between">
                <div>
                  <div className="font-display text-[14px] text-zinc-100">{n.label}</div>
                  <div className="font-mono text-[10px] text-zinc-500">{n.id}</div>
                </div>
                <Pill phase={n.phase} />
              </div>
              {agent ? (
                <>
                  <div className="mt-3 text-[12px] text-zinc-300">{agent.task}</div>
                  <div className="mt-3 grid grid-cols-2 gap-2 text-[10px]">
                    <Stat label="Cost"   value={`$${agent.cost.toFixed(2)}`} />
                    <Stat label="Budget" value={`$${agent.budget.toFixed(2)}`} />
                    <Stat label="ETA"    value={agent.eta} />
                    <Stat label="Skill"  value={agent.skill} />
                  </div>
                </>
              ) : (
                <div className="mt-3 text-[11px] text-zinc-500">Root coordinator · routes all task fan-out.</div>
              )}
            </div>
          );
        })()}
      </div>
    </Glass>
  );
}

function Legend({ color, label }) {
  return <span className="flex items-center gap-1.5 text-zinc-400"><span className={`size-1.5 rounded-full ${color}`} />{label}</span>;
}
function Stat({ label, value }) {
  return (
    <div className="rounded-md border border-white/5 bg-white/[0.02] px-2 py-1.5">
      <div className="text-[9px] uppercase tracking-widest text-zinc-500">{label}</div>
      <div className="mt-0.5 font-mono text-[11px] text-zinc-200">{value}</div>
    </div>
  );
}

Object.assign(window, { AgentFlow });
