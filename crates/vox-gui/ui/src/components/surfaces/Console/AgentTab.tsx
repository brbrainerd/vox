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
    let disposed = false;
    let un: (() => void) | undefined;
    listenAgentEvents((e: AgentEventFrame) => {
      const id = (e.kind as { agent_id?: string | number }).agent_id;
      // Only append frames that carry a matching agent id; frames without one
      // belong to no specific tab and must not bleed in here.
      if (id == null || String(id) !== agentId) return;
      setLines((prev) => [...prev.slice(-499), `${e.timestamp_ms} ${e.kind.type}`]);
    })
      .then((u) => {
        // Unmount may win the race against the async subscription resolving.
        if (disposed) u();
        else un = u;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      un?.();
    };
  }, [agentId]);

  return (
    <div
      aria-label="agent events"
      role="log"
      aria-live="polite"
      style={{ fontFamily: 'monospace', fontSize: 12, padding: 8, overflowY: 'auto', height: '100%' }}
    >
      {lines.length === 0 ? (
        <p className="text-text-muted">waiting for events…</p>
      ) : (
        lines.map((l, i) => <div key={i}>{l}</div>)
      )}
    </div>
  );
}
