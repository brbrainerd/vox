import React, { useState } from 'react';
import type { AgentChip } from './AgentStrip';

interface Props {
  /** Prefilled message body (e.g. the last command line). */
  initialBody?: string;
  agents: AgentChip[];
  onSend: (agentId: string, body: string) => void;
  onClose: () => void;
}

/** Small composer to send a free-form note to an agent's A2A inbox. */
export function SendToAgent({ initialBody = '', agents, onSend, onClose }: Props) {
  const [target, setTarget] = useState(agents[0]?.id ?? '');
  const [body, setBody] = useState(initialBody);
  const canSend = target !== '' && body.trim() !== '';
  return (
    <div role="dialog" aria-label="send to agent" style={{ padding: 12, fontSize: 12 }}>
      <select
        aria-label="agent"
        value={target}
        onChange={(e) => setTarget(e.target.value)}
        style={{ width: '100%', marginBottom: 8 }}
      >
        {agents.map((a) => (
          <option key={a.id} value={a.id}>
            {a.name} · {a.state}
          </option>
        ))}
      </select>
      <textarea
        aria-label="message"
        value={body}
        onChange={(e) => setBody(e.target.value)}
        placeholder="message to send to the agent's inbox"
        rows={3}
        style={{ width: '100%', fontFamily: 'monospace', marginBottom: 8 }}
      />
      <div style={{ display: 'flex', gap: 8 }}>
        <button disabled={!canSend} onClick={() => onSend(target, body)}>
          Send
        </button>
        <button onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}
