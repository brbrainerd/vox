import React from 'react';
import { 
  Zap, BrainCircuit, FileText, Map, TestTube, 
  Bug, RotateCcw, Eye, Search, Layout, 
  ShieldCheck, BookOpen, Gauge, GitBranch, 
  Rocket, Sparkles
} from 'lucide-react';
import { voxTransport } from '../transport';

interface Superpower {
  id: string;
  name: string;
  icon: React.ElementType;
  description: string;
  category: 'Strategic' | 'Execution' | 'Verification';
  color: string;
}

const SUPERPOWERS: Superpower[] = [
  { 
    id: 'brainstorm', 
    name: 'Brainstorm', 
    icon: BrainCircuit, 
    description: 'Explore problem space and ideate high-level solutions.',
    category: 'Strategic',
    color: 'text-violet-400'
  },
  { 
    id: 'specify', 
    name: 'Specify', 
    icon: FileText, 
    description: 'Formalize requirements into machine-readable specifications.',
    category: 'Strategic',
    color: 'text-violet-400'
  },
  { 
    id: 'plan', 
    name: 'Plan', 
    icon: Map, 
    description: 'Decompose goals into an actionable execution graph.',
    category: 'Strategic',
    color: 'text-violet-400'
  },
  { 
    id: 'tdd', 
    name: 'TDD', 
    icon: TestTube, 
    description: 'Implement features via failing tests (Red-Green-Refactor).',
    category: 'Execution',
    color: 'text-primary'
  },
  { 
    id: 'debug', 
    name: 'Debug', 
    icon: Bug, 
    description: 'Systematic root-cause analysis and automated repair.',
    category: 'Execution',
    color: 'text-primary'
  },
  { 
    id: 'refactor', 
    name: 'Refactor', 
    icon: RotateCcw, 
    description: 'Improve code structure without changing behavior.',
    category: 'Execution',
    color: 'text-primary'
  },
  { 
    id: 'review', 
    name: 'Review', 
    icon: Eye, 
    description: 'Critically assess code against specifications and standards.',
    category: 'Verification',
    color: 'text-emerald-400'
  },
  { 
    id: 'research', 
    name: 'Research', 
    icon: Search, 
    description: 'Autonomous context gathering from web and local corpora.',
    category: 'Strategic',
    color: 'text-violet-400'
  },
  { 
    id: 'mockup', 
    name: 'Mockup', 
    icon: Layout, 
    description: 'Generate interactive UI prototypes and visual designs.',
    category: 'Execution',
    color: 'text-primary'
  },
  { 
    id: 'audit', 
    name: 'Audit', 
    icon: ShieldCheck, 
    description: 'Security, compliance, and architectural boundary checks.',
    category: 'Verification',
    color: 'text-emerald-400'
  },
  { 
    id: 'document', 
    name: 'Document', 
    icon: BookOpen, 
    description: 'Keep documentation synchronized with implementation.',
    category: 'Execution',
    color: 'text-primary'
  },
  { 
    id: 'optimize', 
    name: 'Optimize', 
    icon: Gauge, 
    description: 'Performance profiling and algorithmic optimization.',
    category: 'Execution',
    color: 'text-primary'
  },
  { 
    id: 'sync', 
    name: 'Sync', 
    icon: GitBranch, 
    description: 'Manage version control, conflicts, and branch state.',
    category: 'Strategic',
    color: 'text-violet-400'
  },
  { 
    id: 'deploy', 
    name: 'Deploy', 
    icon: Rocket, 
    description: 'Orchestrate build, test, and production release cycles.',
    category: 'Execution',
    color: 'text-primary'
  }
];

export const SuperpowersCatalog: React.FC = () => {
  const handleActivate = (power: Superpower) => {
    voxTransport.callTool('vox_submit_task', { 
      description: `Run ${power.name} skill: ${power.description}`,
      active_skill: `superpowers:${power.id}`,
      mode: power.category === 'Strategic' ? 'Verbose' : 'Precision'
    });
  };

  return (
    <div className="flex-1 flex flex-col p-8 overflow-hidden bg-void relative">
      <div className="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-violet-500 via-primary to-emerald-500 opacity-50" />
      
      <div className="flex items-center justify-between mb-12">
        <div>
          <h1 className="text-4xl font-rajdhani font-bold text-foreground tracking-tighter flex items-center gap-3">
            <Zap className="text-primary animate-pulse" size={32} />
            SUPERPOWERS
          </h1>
          <p className="text-steel font-mono text-sm mt-2 uppercase tracking-widest opacity-70">
            Procedural Skill Catalog • Agentic Workflows
          </p>
        </div>
        
        <div className="flex gap-4">
          <div className="px-4 py-2 rounded-lg bg-machine border border-border flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-violet-400" />
            <span className="text-[10px] font-mono font-bold text-steel uppercase">Strategic</span>
          </div>
          <div className="px-4 py-2 rounded-lg bg-machine border border-border flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-primary" />
            <span className="text-[10px] font-mono font-bold text-steel uppercase">Execution</span>
          </div>
          <div className="px-4 py-2 rounded-lg bg-machine border border-border flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-emerald-400" />
            <span className="text-[10px] font-mono font-bold text-steel uppercase">Verification</span>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6 overflow-y-auto pr-4 custom-scrollbar">
        {SUPERPOWERS.map((power) => (
          <div 
            key={power.id}
            className="group glass p-6 rounded-2xl border border-border hover:border-primary transition-all hover:translate-y-[-4px] flex flex-col relative overflow-hidden"
          >
            <div className={`absolute top-0 right-0 p-4 opacity-5 group-hover:opacity-10 transition-opacity`}>
              <power.icon size={64} />
            </div>
            
            <div className="flex items-center gap-4 mb-4">
              <div className={`p-3 rounded-xl bg-machine border border-border ${power.color}`}>
                <power.icon size={24} />
              </div>
              <h3 className="text-lg font-rajdhani font-bold text-foreground uppercase tracking-wider">{power.name}</h3>
            </div>
            
            <p className="text-steel text-sm leading-relaxed mb-8 flex-1">
              {power.description}
            </p>
            
            <button
              onClick={() => handleActivate(power)}
              className="w-full py-3 rounded-xl bg-machine border border-border text-[11px] font-bold uppercase tracking-widest text-steel hover:bg-primary hover:text-black hover:border-transparent transition-all flex items-center justify-center gap-2"
            >
              <Sparkles size={14} />
              Activate Skill
            </button>
          </div>
        ))}
      </div>
    </div>
  );
};
