import React from 'react';

export interface AgentChip {
  id: string;
  name: string;
  state: string;
}

interface Props {
  agents: AgentChip[];
  onOpen: (agentId: string) => void;
}

/**
 * Persistent strip of live agents. Data comes from the same `vox://orch-status`
 * snapshot the Dashboard uses (mapped by the parent), so the two never disagree.
 * Clicking a chip asks the parent to open that agent as a tab.
 */
export function AgentStrip({ agents, onOpen }: Props) {
  if (agents.length === 0) {
    return (
      <div aria-label="agents" style={{ padding: '4px 10px', fontSize: 11, color: '#9ca3af' }}>
        no agents
      </div>
    );
  }
  return (
    <div aria-label="agents" style={{ display: 'flex', gap: 8, padding: '4px 10px', fontSize: 11 }}>
      {agents.map((a) => (
        <button
          key={a.id}
          onClick={() => onOpen(a.id)}
          style={{ borderRadius: 10, padding: '2px 8px', cursor: 'pointer' }}
          title={`${a.name} · ${a.state}`}
        >
          {a.name} · {a.state}
        </button>
      ))}
    </div>
  );
}
