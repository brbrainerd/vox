import React from 'react';
import { Layers, Terminal, Activity, CheckCircle2, AlertCircle, Clock, Zap, Target, Cpu, MessageSquare } from 'lucide-react';
import { motion } from 'framer-motion';

export const Dashboard = ({ ops = [], stats = {} }: any) => {
  return (
    <div className="p-10 grid grid-cols-12 gap-8 overflow-y-auto h-full bg-[#09090b]">
      <div className="col-span-12 mb-2">
        <h2 className="text-3xl font-extrabold text-white tracking-tight mb-2 flex items-center gap-3">
          Fleet <span className="text-blue-500">Dashboard</span>
          <div className="px-2 py-0.5 rounded bg-blue-500/10 border border-blue-500/20 text-[10px] font-bold uppercase text-blue-500 tracking-widest">Live</div>
        </h2>
        <p className="text-zinc-500 text-sm max-w-2xl font-medium tracking-wide">Orchestrator monitoring and real-time compiler telemetry for the Vox workspace.</p>
      </div>

      {/* Primary Stats row */}
      <div className="col-span-3">
        <StatCard title="Active Agents" value={stats.activeAgents || "4"} delta="+1" color="blue" icon={<Cpu size={16} />} />
      </div>
      <div className="col-span-3">
        <StatCard title="Queue Depth" value={stats.queueDepth || "12"} delta="-2" color="emerald" icon={<Layers size={16} />} />
      </div>
      <div className="col-span-3">
        <StatCard title="Avg Latency" value={stats.latency || "0.8s"} delta="-120ms" color="purple" icon={<Zap size={16} />} />
      </div>
      <div className="col-span-3">
        <StatCard title="Fleet Budget" value={stats.budget || "98%"} delta="+0.4%" color="amber" icon={<Target size={16} />} />
      </div>

      {/* Main interaction row */}
      <div className="col-span-8">
        <div className="glass rounded-[2rem] border border-white/5 p-8 h-full">
           <div className="flex items-center justify-between mb-8">
             <div className="flex items-center gap-3">
               <div className="w-10 h-10 rounded-2xl bg-blue-500/10 flex items-center justify-center text-blue-500">
                  <Activity size={20} />
               </div>
               <div>
                 <h3 className="text-sm font-bold text-white/90 uppercase tracking-widest leading-tight">Operation Stream</h3>
                 <span className="text-[11px] text-zinc-500 font-medium">Real-time task dispatching across @vox-mcp</span>
               </div>
             </div>
             <button className="px-4 py-2 rounded-xl bg-white/5 border border-white/10 text-[11px] font-bold text-zinc-400 hover:text-white hover:bg-white/10 transition-all uppercase tracking-widest">View All Logs</button>
           </div>

           <div className="space-y-1">
             <OpRow label="vox_validate_file" agent="A-01" status="Success" time="42ms" />
             <OpRow label="vox_run_tests" agent="A-03" status="Running" time="2.4s" active />
             <OpRow label="vox_a2a_tasks" agent="A-01" status="Success" time="12ms" />
             <OpRow label="vox_submit_task" agent="A-04" status="Queued" time="-" />
             <OpRow label="vox_git_status" agent="A-02" status="Success" time="4ms" />
           </div>
        </div>
      </div>

      <div className="col-span-4 flex flex-col gap-8">
        {/* Alerts / Error stack */}
        <div className="glass rounded-[2rem] border border-white/5 p-8 flex-1">
           <div className="flex items-center gap-3 mb-8">
              <div className="w-10 h-10 rounded-2xl bg-emerald-500/10 flex items-center justify-center text-emerald-500">
                 <MessageSquare size={20} />
              </div>
              <h3 className="text-sm font-bold text-white/90 uppercase tracking-widest leading-tight">Pipeline Health</h3>
           </div>
           
           <div className="flex flex-col items-center justify-center h-48 py-10">
              <div className="w-16 h-16 rounded-full bg-emerald-500/5 flex items-center justify-center text-emerald-500/30 mb-4 animate-ping" />
              <div className="absolute w-16 h-16 rounded-full bg-emerald-500/5 border border-emerald-500/20 flex items-center justify-center text-emerald-500">
                 <CheckCircle2 size={32} />
              </div>
              <p className="text-[11px] font-bold text-emerald-500 uppercase tracking-widest mt-4">All Stages Green</p>
              <span className="text-[10px] text-zinc-500 mt-1">Last audit: Just now</span>
           </div>
        </div>

        {/* Action Quicklinks */}
        <div className="grid grid-cols-2 gap-4">
           <ActionBtn icon={<Terminal size={16} />} label="Fmt Build" />
           <ActionBtn icon={<Layers size={16} />} label="Rebalance" />
        </div>
      </div>
    </div>
  );
};

const OpRow = ({ label, agent, status, time, active }: any) => (
  <motion.div 
    initial={false}
    animate={{ opacity: 1 }}
    className="group flex items-center justify-between py-4 border-b border-white/[0.03] last:border-0 hover:bg-white/[0.02] -mx-4 px-4 rounded-2xl transition-all duration-300"
  >
    <div className="flex items-center gap-4">
      <div className={`w-10 h-10 rounded-xl flex items-center justify-center transition-all ${
        status === 'Success' ? 'bg-emerald-500/5 text-emerald-500 group-hover:bg-emerald-500/10' : 
        status === 'Running' ? 'bg-blue-500/10 text-blue-500 animate-pulse border border-blue-500/20 shadow-[0_0_15px_rgba(59,130,246,0.1)]' : 
        'bg-zinc-500/10 text-zinc-500'
      }`}>
         <Terminal size={16} />
      </div>
      <div>
        <p className="text-sm font-bold text-white/80 group-hover:text-white transition-colors tracking-tight">{label}</p>
        <span className="text-[10px] text-zinc-500 font-bold uppercase tracking-widest">@ {agent}</span>
      </div>
    </div>
    
    <div className="flex items-center gap-8">
       <span className="text-[11px] font-mono text-zinc-600 font-medium">{time}</span>
       <div className={`px-3 py-1 rounded-lg text-[9px] font-extrabold uppercase tracking-widest border transition-all ${
         status === 'Success' ? 'bg-emerald-500/10 text-emerald-500 border-emerald-500/20' : 
         status === 'Running' ? 'bg-blue-500/10 text-blue-400 border-blue-500/20 shadow-[0_0_10px_rgba(59,130,246,0.1)]' : 
         'bg-zinc-500/5 text-zinc-600 border-transparent'
       }`}>
         {status}
       </div>
    </div>
  </motion.div>
)

const StatCard = ({ title, value, delta, color, icon }: any) => (
  <motion.div 
    whileHover={{ y: -4, scale: 1.02 }}
    className="bg-[#101012] border border-[#27272a] rounded-[2rem] p-8 hover:border-blue-500/30 transition-all cursor-default group relative overflow-hidden"
  >
     {/* Background glow decorator */}
     <div className="absolute -top-10 -right-10 w-32 h-32 bg-blue-500/5 blur-[40px] rounded-full group-hover:bg-blue-500/10 transition-all duration-700" />
     
     <div className="flex justify-between items-center mb-6">
       <div className="w-8 h-8 rounded-lg bg-white/5 border border-white/10 flex items-center justify-center text-zinc-500 group-hover:text-blue-500 group-hover:border-blue-500/30 transition-all">
          {icon}
       </div>
       {delta && <span className={`text-[10px] font-extrabold tracking-widest uppercase ${delta.startsWith('-') ? 'text-rose-500' : 'text-emerald-500'} bg-white/[0.02] px-2 py-0.5 rounded-full border border-white/5`}>{delta}</span>}
     </div>
     
     <div className="flex flex-col">
       <span className="text-[10px] font-bold text-zinc-500 mb-1 uppercase tracking-widest peer">{title}</span>
       <span className="text-4xl font-black text-white group-hover:text-blue-400 transition-all duration-300 tracking-tighter">{value}</span>
     </div>
  </motion.div>
);

const ActionBtn = ({ icon, label }: any) => (
  <button className="flex-1 glass border border-white/5 rounded-2xl py-5 px-4 flex flex-col items-center gap-3 group hover:border-blue-500/40 hover:bg-blue-500/[0.02] transition-all">
     <div className="text-zinc-500 group-hover:text-blue-500 transition-colors">{icon}</div>
     <span className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest group-hover:text-zinc-300">{label}</span>
  </button>
)
