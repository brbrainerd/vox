import React, { useState } from 'react';
import { Home, Layers, GitBranch, Terminal, BarChart3, Settings, FileJson } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { ReactFlowProvider } from '@xyflow/react';
import '@xyflow/react/dist/style.css';

// Components
import { Dashboard } from './components/Dashboard';
import { AgentFlow } from './components/AgentFlow';
import { PipelineView } from './components/PipelineView';
import { AstView } from './components/AstView';
import { RouteManifestIngest } from './components/RouteManifestIngest';

type View = 'dashboard' | 'dag' | 'pipeline' | 'ast' | 'telemetry' | 'webartifacts';

function App() {
  const [activeView, setActiveView] = useState<View>('dashboard');

  // Placeholder for real data
  const tasks = [
    { id: 1, description: 'Fix compiler pipeline', priority: 'Urgent', status: 'InProgress', depends_on: [] },
    { id: 2, description: 'Implement React visualizer', priority: 'Normal', status: 'Queued', depends_on: [1] },
    { id: 3, description: 'Sync LSP tokens', priority: 'Normal', status: 'Completed', depends_on: [] }
  ];

  return (
    <div className="flex h-screen bg-[#09090b] text-white overflow-hidden font-sans select-none">
      {/* Sidebar */}
      <nav className="w-20 flex flex-col items-center py-8 border-r border-[#27272a] bg-[#09090b]/80 backdrop-blur-3xl z-50">
        <div className="w-12 h-12 rounded-[1.25rem] bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center mb-12 shadow-[0_0_20px_rgba(59,130,246,0.3)]">
          <Layers className="w-6 h-6 text-white" />
        </div>
        
        <div className="flex-1 flex flex-col gap-8">
          <NavItem icon={<Home size={22} />} active={activeView === 'dashboard'} onClick={() => setActiveView('dashboard')} label="Dashboard" />
          <NavItem icon={<GitBranch size={22} />} active={activeView === 'dag'} onClick={() => setActiveView('dag')} label="Agent DAG" />
          <NavItem icon={<Terminal size={22} />} active={activeView === 'pipeline'} onClick={() => setActiveView('pipeline')} label="Pipeline" />
          <NavItem icon={<BarChart3 size={22} />} active={activeView === 'telemetry'} onClick={() => setActiveView('telemetry')} label="Telemetry" />
          <NavItem icon={<FileJson size={22} />} active={activeView === 'webartifacts'} onClick={() => setActiveView('webartifacts')} label="Web IR" />
        </div>

        <div className="mt-auto">
          <NavItem icon={<Settings size={22} />} active={activeView === 'ast'} onClick={() => setActiveView('ast')} label="AST Inspect" />
        </div>
      </nav>

      {/* Main Content */}
      <main className="flex-1 relative overflow-hidden bg-gradient-to-br from-transparent to-blue-500/[0.01]">
        <header className="h-16 border-b border-[#27272a]/50 flex items-center px-10 justify-between bg-[#09090b]/40 backdrop-blur-md sticky top-0 z-40">
          <div className="flex flex-col">
            <h1 className="text-xl font-bold tracking-tight text-white/95">Vox <span className="text-blue-500">Visualizer</span></h1>
            <span className="text-[10px] text-blue-500 font-mono uppercase tracking-widest leading-none mt-1 opacity-60">V0.2.0 Hardened Architecture</span>
          </div>
          
          <div className="flex items-center gap-4">
            <div className="px-3 py-1.5 rounded-full bg-emerald-500/10 border border-emerald-500/20 flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse shadow-[0_0_8px_rgba(16,185,129,0.5)]" />
              <span className="text-[9px] font-bold font-mono text-emerald-500 uppercase tracking-widest">LIVE WORKSPACE</span>
            </div>
            <div className="h-4 w-px bg-white/10 mx-2" />
            <div className="flex -space-x-2">
               <Avatar src="A1" color="blue" />
               <Avatar src="A3" color="emerald" />
               <Avatar src="A4" color="purple" />
            </div>
          </div>
        </header>

        <div className="h-[calc(100vh-64px)] w-full overflow-hidden">
          <AnimatePresence mode="wait">
            <motion.div
              key={activeView}
              initial={{ opacity: 0, scale: 0.99, y: 5 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.99, y: -5 }}
              transition={{ duration: 0.3, ease: [0.19, 1, 0.22, 1] }}
              className="h-full w-full"
            >
              {activeView === 'dashboard' && <Dashboard />}
              {activeView === 'dag' && <AgentFlow tasks={tasks} />}
              {activeView === 'pipeline' && <PipelineView />}
              {activeView === 'telemetry' && <div className="p-10 text-zinc-500">Telemetry view initialization...</div>}
              {activeView === 'webartifacts' && <RouteManifestIngest />}
              {activeView === 'ast' && <AstView />}
            </motion.div>
          </AnimatePresence>
        </div>
      </main>
    </div>
  );
}

const NavItem = ({ icon, active, onClick, label }: { icon: any, active: boolean, onClick: () => void, label: string }) => (
  <button 
    onClick={onClick}
    className={`group relative w-12 h-12 rounded-2xl flex items-center justify-center transition-all duration-500 ${
      active ? 'bg-blue-600/15 text-blue-500 shadow-[0_0_25px_rgba(59,130,246,0.15)] border border-blue-500/20' : 'text-zinc-500 hover:text-zinc-300 hover:bg-white/5'
    }`}
    title={label}
  >
    {icon}
    {active && <motion.div layoutId="nav-glow" className="absolute left-[-24px] w-1.5 h-8 bg-blue-500 rounded-full shadow-[0_0_15px_rgba(59,130,246,0.5)]" />}
  </button>
);

const Avatar = ({ src, color }: any) => (
  <div className={`w-8 h-8 rounded-full bg-${color}-500/10 border-2 border-[#09090b] flex items-center justify-center text-[10px] font-bold text-${color}-500 uppercase tracking-tighter`}>{src}</div>
)

export default () => (
  <ReactFlowProvider>
    <App />
  </ReactFlowProvider>
);
