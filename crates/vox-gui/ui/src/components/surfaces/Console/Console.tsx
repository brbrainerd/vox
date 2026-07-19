import React, { useCallback, useMemo, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { InputEditor } from './InputEditor';
import { DiscoveryRail } from './DiscoveryRail';
import { TerminalTab, type PendingLine } from './TerminalTab';
import { AgentStrip, type AgentChip } from './AgentStrip';
import { AgentTab } from './AgentTab';
import { SendToAgent } from './SendToAgent';
import { renderBlockForAgent, type Block } from './osc633';
import { sendToAgent } from '../../../transport';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';
import { Button } from '../../ui/Button';
import {
  orchestratorStatusErrorMessage,
  useOrchestratorStatus,
} from '../../../hooks/useOrchestratorStatus';
import type { OrchestratorStatus, RawAgentSummary, Toast } from '../../../types/tauri';

function agentsFromStatus(status: OrchestratorStatus | undefined): AgentChip[] {
  return (status?.agents ?? []).map((a: RawAgentSummary) => ({
    id: String(a.id ?? ''),
    name: String(a.name ?? a.codename ?? a.id ?? 'agent'),
    state: a.paused ? 'paused' : a.in_progress ? 'running' : 'queued',
  }));
}

interface Props {
  pushToast: (item: Toast) => void;
  gamifyEnabled?: boolean;
  /** When set (e.g. via the Dashboard "Open in Console" deep link), open this
   *  agent's live event tab on mount. */
  initialAgentId?: string | null;
}

/**
 * Vox Console surface (layout A): agent strip on top, terminal on the left,
 * persistent discovery rail on the right, owned input editor along the bottom.
 * A single PTY tab ("console-1") in v1; multi-tab is additive.
 */
export function Console({ pushToast, gamifyEnabled = false, initialAgentId = null }: Props) {
  const orchQuery = useOrchestratorStatus();
  const agents = useMemo(() => agentsFromStatus(orchQuery.data), [orchQuery.data]);
  const orchError = orchestratorStatusErrorMessage(orchQuery);
  const [pending, setPending] = useState<PendingLine | null>(null);
  const [activeAction, setActiveAction] = useState<string | null>(null);
  const [openAgentId, setOpenAgentId] = useState<string | null>(initialAgentId);
  const [composing, setComposing] = useState(false);
  const [lastLine, setLastLine] = useState('');
  const [latestBlock, setLatestBlock] = useState<Block | null>(null);
  const [applyLine, setApplyLine] = useState<string | null>(null);
  const seq = React.useRef(0);
  const tabId = 'console-1';
  const nowMs = Date.now();

  const submit = (line: string) => {
    seq.current += 1;
    setLastLine(line);
    setPending({ text: line, seq: seq.current });
  };

  const handleBlock = useCallback(
    (block: Block) => {
      setLatestBlock(block);
      if (block.exitCode === 0) {
        recordGamifyGuiEvent(
          'console_command_success',
          { command: block.command },
          { enabled: gamifyEnabled },
        );
      }
    },
    [gamifyEnabled],
  );

  const openAgentTab = (agentId: string) => {
    setOpenAgentId(agentId);
  };

  const blockBody = renderBlockForAgent(latestBlock, lastLine);

  const copyLastBlock = async () => {
    const writeText = navigator.clipboard?.writeText?.bind(navigator.clipboard);
    if (!writeText) {
      pushToast({ tone: 'warn', title: 'Copy failed', body: 'clipboard unavailable', cause: 'backend-error' });
      return;
    }
    try {
      await writeText(blockBody);
      pushToast({ tone: 'ok', title: 'Copied', body: 'last block to clipboard', cause: 'clipboard' });
    } catch {
      pushToast({ tone: 'warn', title: 'Copy failed', cause: 'backend-error' });
    }
  };

  const handleSend = (agentId: string, body: string) => {
    setComposing(false);
    sendToAgent(agentId, body)
      .then(() => pushToast({ tone: 'ok', title: 'Sent', body: `to agent ${agentId}`, cause: 'backend-ok' }))
      .catch((e) => pushToast({ tone: 'warn', title: 'Send failed', body: sanitizeErrorForToast(e), cause: 'backend-error' }));
  };

  return (
    <div className="flex h-full min-h-0 flex-col" data-testid="console-root">
      <div className="flex shrink-0 items-center justify-between">
        <div className="min-w-0 flex-1">
          <AgentStrip agents={agents} onOpen={openAgentTab} />
          {orchError && (
            <div role="alert" aria-live="polite" className="px-2.5 pb-1 text-[11px] text-amber-400">
              {orchError}
            </div>
          )}
        </div>
        <div className="mx-2.5 flex gap-1.5">
          <Button onClick={copyLastBlock} className="text-[11px]">
            copy last block
          </Button>
          <Button
            disabled={agents.length === 0}
            onClick={() => setComposing(true)}
            className="text-[11px]"
          >
            send to agent
          </Button>
        </div>
      </div>
      {composing && (
        <SendToAgent
          initialBody={blockBody}
          agents={agents}
          onSend={handleSend}
          onClose={() => setComposing(false)}
        />
      )}
      <div className="flex min-h-0 flex-1">
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          {openAgentId && (
            <div className="flex shrink-0 items-center gap-2 border-b border-white/10 px-2.5 py-1 text-[11px]">
              <span>agent {openAgentId}</span>
              <Button
                onClick={() => setOpenAgentId(null)}
                aria-label="Close agent view and return to terminal"
              >
                back to terminal
              </Button>
            </div>
          )}
          <div className={`min-h-0 flex-1 ${openAgentId ? 'block' : 'hidden'}`}>
            {openAgentId ? <AgentTab agentId={openAgentId} /> : null}
          </div>
          <div className={`min-h-0 flex-1 ${openAgentId ? 'hidden' : 'block'}`}>
            <TerminalTab tabId={tabId} pendingLine={pending} onBlock={handleBlock} />
          </div>
          <div className="shrink-0 border-t border-white/10 px-2.5 py-1.5">
            <InputEditor
              onSubmit={submit}
              onActiveSuggestion={setActiveAction}
              applyLine={applyLine}
              onApplyLineConsumed={() => setApplyLine(null)}
            />
          </div>
        </div>
        <DiscoveryRail
          actionId={activeAction}
          nowMs={nowMs}
          gamifyEnabled={gamifyEnabled}
          onUseAction={(example) => setApplyLine(example)}
        />
      </div>
    </div>
  );
}
