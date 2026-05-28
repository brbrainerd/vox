import { useState } from 'react';
import { SpeakPanel } from './SpeakPanel';
import { CommandTab } from '../../../app/src/generated/CommandTab';
import { NetworkTab } from '../../../app/src/generated/NetworkTab';
import { ForgeTab } from '../../../app/src/generated/ForgeTab';

type Tab = 'speak' | 'command' | 'network' | 'forge';

const NAV: { id: Tab; label: string }[] = [
  { id: 'speak',   label: 'LOQUELA'  },
  { id: 'command', label: 'IMPERIUM' },
  { id: 'network', label: 'RETE'     },
  { id: 'forge',   label: 'FABRICA'  },
];

export function AppShellLive() {
  const [tab, setTab] = useState<Tab>('speak');

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100 font-mono flex flex-col">
      <div className="h-10 border-b border-zinc-800 px-4 flex items-center justify-between shrink-0">
        <span className="text-xs tracking-widest text-zinc-500">VOX ORCHESTRATOR</span>
        <div className="flex gap-1">
          {NAV.map(({ id, label }) => (
            <button
              key={id}
              className={`tab-btn${tab === id ? ' tab-btn-active' : ''}`}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      <div className="flex-1 overflow-hidden flex flex-col">
        <div className={tab === 'speak'   ? 'contents' : 'hidden'}><SpeakPanel /></div>
        <div className={tab === 'command' ? 'contents' : 'hidden'}><CommandTab /></div>
        <div className={tab === 'network' ? 'contents' : 'hidden'}><NetworkTab /></div>
        <div className={tab === 'forge'   ? 'contents' : 'hidden'}><ForgeTab /></div>
      </div>
    </div>
  );
}
