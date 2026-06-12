import React, { useEffect, useState } from 'react';
import { listenAgentEvents, type AgentEventFrame } from '../../../transport';

interface Props {
  agentId: string;
}

/**
 * A console tab showing one agent's live event stream. Reuses the existing
 * `vox://agent-events` Tauri stream (the same source as the Dashboard), filtering
 * to this agent. Read-only view; spawning/controlling agents is done via commands.
 */
export function AgentTab({ agentId }: Props) {
  const [lines, setLines] = useState<string[]>([]);

  useEffect(() => {
    setLines([]);
    let un: (() => void) | undefined;
    listenAgentEvents((e: AgentEventFrame) => {
      const id = (e.kind as { agent_id?: string | number }).agent_id;
      if (id != null && String(id) !== agentId) return;
      setLines((prev) => [...prev.slice(-499), `${e.timestamp_ms} ${e.kind.type}`]);
    })
      .then((u) => (un = u))
      .catch(() => {});
    return () => un?.();
  }, [agentId]);

  return (
    <div
      aria-label="agent events"
      style={{ fontFamily: 'monospace', fontSize: 12, padding: 8, overflowY: 'auto', height: '100%' }}
    >
      {lines.length === 0 ? (
        <p style={{ color: '#9ca3af' }}>waiting for events…</p>
      ) : (
        lines.map((l, i) => <div key={i}>{l}</div>)
      )}
    </div>
  );
}
