import React, { useState } from 'react';
import { control } from './subAgentClient';
import type { SubAgentStatus } from './types';

export function SubAgentControls({ windowId, status }: { windowId: string; status: SubAgentStatus }) {
  const [note, setNote] = useState('');
  return (
    <div role="group" aria-label={`controls for ${windowId}`}>
      {status === 'paused'
        ? <button aria-label={`resume ${windowId}`} onClick={() => control(windowId, { kind: 'resume' })}>Resume</button>
        : <button aria-label={`pause ${windowId}`} onClick={() => control(windowId, { kind: 'pause' })}>Pause</button>}
      <button aria-label={`kill ${windowId}`} onClick={() => control(windowId, { kind: 'kill' })}>Kill</button>
      <input aria-label="overrule note" value={note} onChange={(e) => setNote(e.target.value)} placeholder="overrule…" />
      <button aria-label={`overrule ${windowId}`} onClick={() => control(windowId, { kind: 'overrule', note })}>Overrule</button>
    </div>
  );
}
