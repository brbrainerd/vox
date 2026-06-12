import React, { useEffect, useState } from 'react';
import { InputEditor } from './InputEditor';
import { DiscoveryRail } from './DiscoveryRail';
import { TerminalTab, type PendingLine } from './TerminalTab';
import { AgentStrip, type AgentChip } from './AgentStrip';
import { AgentTab } from './AgentTab';
import { listenOrchStatus } from '../../../transport';

interface Props {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

/**
 * Vox Console surface (layout A): agent strip on top, terminal on the left,
 * persistent discovery rail on the right, owned input editor along the bottom.
 * A single PTY tab ("console-1") in v1; multi-tab is additive.
 */
export function Console({ pushToast }: Props) {
  const [pending, setPending] = useState<PendingLine | null>(null);
  const [activeAction, setActiveAction] = useState<string | null>(null);
  const [agents, setAgents] = useState<AgentChip[]>([]);
  const [openAgentId, setOpenAgentId] = useState<string | null>(null);
  const seq = React.useRef(0);
  const tabId = 'console-1';
  const nowMs = Date.now();

  useEffect(() => {
    let un: (() => void) | undefined;
    listenOrchStatus((status: any) => {
      const list: AgentChip[] = (status?.agents ?? []).map((a: any) => ({
        id: String(a.id ?? a.agent_id ?? ''),
        name: String(a.name ?? a.id ?? 'agent'),
        state: a.paused ? 'paused' : a.in_progress > 0 ? 'running' : 'queued',
      }));
      setAgents(list);
    })
      .then((u) => (un = u))
      .catch(() => {
        /* not in tauri / daemon down — strip shows "no agents" */
      });
    return () => un?.();
  }, []);

  const submit = (line: string) => {
    seq.current += 1;
    setPending({ text: line, seq: seq.current });
  };

  const openAgentTab = (agentId: string) => {
    setOpenAgentId(agentId);
    pushToast({ tone: 'info', title: 'Agent', body: `streaming events for ${agentId}` });
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <AgentStrip agents={agents} onOpen={openAgentTab} />
      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minWidth: 0 }}>
          {openAgentId && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 8,
                padding: '4px 10px',
                fontSize: 11,
                borderBottom: '1px solid rgba(255,255,255,0.08)',
              }}
            >
              <span>agent {openAgentId}</span>
              <button onClick={() => setOpenAgentId(null)}>back to terminal</button>
            </div>
          )}
          <div style={{ flex: 1, minHeight: 0, display: openAgentId ? 'block' : 'none' }}>
            {openAgentId && <AgentTab agentId={openAgentId} />}
          </div>
          <div style={{ flex: 1, minHeight: 0, display: openAgentId ? 'none' : 'block' }}>
            <TerminalTab tabId={tabId} pendingLine={pending} />
          </div>
          <div style={{ borderTop: '1px solid rgba(255,255,255,0.08)', padding: '6px 10px' }}>
            <InputEditor onSubmit={submit} onActiveSuggestion={setActiveAction} />
          </div>
        </div>
        <DiscoveryRail actionId={activeAction} nowMs={nowMs} />
      </div>
    </div>
  );
}
