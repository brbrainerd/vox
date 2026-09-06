import React, { useEffect, useState } from 'react';
import { Glass } from '../../ui/Glass';
import { Kpi } from '../../ui/Kpi';
import { ContextWindowMeter } from './ContextWindowMeter';
import { useLabel } from '../../../hooks/useLanguage';
import { getContextBudget, type ContextBudgetPayload } from '../../../transport';



export interface ChatExecutionTask {
  id: string;
  title: string;
  status?: string;
}

export interface ChatExecutionRailKpis {
  activeAgents: { value: number };
  queueDepth: { value: number };
  mesh: { peers: number };
}

export interface ChatExecutionRailProps {
  tasks: ChatExecutionTask[];
  kpis: ChatExecutionRailKpis;
  intents?: string[];
  activeModel?: string | null;
  openrouterSpendUsd?: number | null;
  onNavigate: (viewKey: string) => void;
  /** Active chat session id — passed to get_context_budget so the meter shows real token usage. */
  sessionId?: string | null;
  /** Opens the inline Routing panel (folded Matrix surface — gui-ia-blueprint: matrix → chat rail). */
  onOpenRouting?: () => void;
}

function formatOpenRouterSpend(usd: number): string {
  return `$${usd.toFixed(2)}`;
}

function Segment({
  testId,
  label,
  value,
  onClick,
}: {
  testId: string;
  label: string;
  value: string;
  onClick?: () => void;
}) {
  const className =
    'inline-flex w-full items-center justify-between gap-2 rounded-sm px-2 py-1 text-[10px] text-text-muted transition hover:bg-overlay-subtle hover:text-text-secondary';

  if (onClick) {
    return (
      <button type="button" data-testid={testId} onClick={onClick} className={className}>
        <span className="uppercase tracking-[0.14em] text-text-muted">{label}</span>
        <span className="font-mono tabular-nums text-text-secondary">{value}</span>
      </button>
    );
  }

  return (
    <div data-testid={testId} className={className}>
      <span className="uppercase tracking-[0.14em] text-text-muted">{label}</span>
      <span className="font-mono tabular-nums text-text-secondary">{value}</span>
    </div>
  );
}

export function ChatExecutionRail({
  tasks,
  kpis,
  intents,
  activeModel,
  openrouterSpendUsd,
  onNavigate,
  sessionId,
  onOpenRouting,
}: ChatExecutionRailProps) {
  const [budget, setBudget] = useState<ContextBudgetPayload | null>(null);

  useEffect(() => {
    getContextBudget(sessionId)
      .then(setBudget)
      .catch(() => {/* daemon unavailable; meter stays hidden */});
  }, [sessionId]);

  const peerLabel = kpis.mesh.peers === 1 ? '1 peer' : `${kpis.mesh.peers} peers`;

  return (
    <aside aria-label="Execution rail" className="w-full min-w-0">
      <Glass className="flex h-full flex-col gap-3 p-3">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-[10px] uppercase tracking-[0.18em] text-brass">{useLabel('chat-execution')}</h2>
        </div>

        <section
          role="region"
          aria-label="Active tasks"
          className="flex min-h-0 flex-1 flex-col gap-2"
        >
          {tasks.length === 0 ? (
            <p className="text-[11px] text-text-muted">No active tasks for this session.</p>
          ) : (
            <ul className="flex flex-col gap-1.5 overflow-y-auto custom-scrollbar">
              {tasks.map(task => (
                <li
                  key={task.id}
                  className="rounded-lg border border-border-subtle bg-overlay-subtle px-2.5 py-2"
                >
                  <p className="text-xs text-text-secondary leading-snug truncate" title={task.title}>{task.title}</p>
                  {task.status && (
                    <p className="mt-0.5 text-[10px] uppercase tracking-[0.12em] text-text-muted">
                      {task.status}
                    </p>
                  )}
                </li>
              ))}
            </ul>
          )}
        </section>

        {intents != null && intents.length > 0 && (
          <section
            role="region"
            aria-label="Intent map"
            className="flex flex-col gap-1 pt-3"
          >
            <div className="mb-1.5 border-b border-border-subtle pb-1 font-display text-[9px] uppercase tracking-[0.28em] text-text-muted">
              Intents
            </div>
            {intents.slice(0, 3).map(intent => (
              <button
                key={intent}
                type="button"
                aria-label={intent}
                onClick={() => onOpenRouting?.()}
                className="rounded-sm px-2 py-1 text-left text-[11px] text-text-secondary transition hover:bg-overlay-subtle hover:text-brass"
              >
                {intent}
              </button>
            ))}
          </section>
        )}

        <section aria-label="Resource strip" className="flex flex-col gap-1 pt-3">
          <div className="mb-1.5 border-b border-border-subtle pb-1 font-display text-[9px] uppercase tracking-[0.28em] text-text-muted">
            Resources
          </div>
          <Kpi
            data-testid="execution-rail-agents"
            label="Agents"
            value={kpis.activeAgents.value}
            accent="cyan"
            onClick={() => onNavigate('agents')}
            className="cursor-pointer mb-2"
          />
          <Kpi
            data-testid="execution-rail-queue"
            label="Queue"
            value={kpis.queueDepth.value}
            accent="amber"
            onClick={() => onNavigate('runs')}
            className="cursor-pointer mb-2"
          />
          <Segment
            testId="execution-rail-mesh"
            label="Mesh"
            value={peerLabel}
            onClick={() => onNavigate('mesh')}
          />
          {activeModel != null && activeModel !== '' && (
            <Segment
              testId="execution-rail-model"
              label="Model"
              value={activeModel}
              onClick={() => onNavigate('models')}
            />
          )}
          {openrouterSpendUsd != null && !Number.isNaN(openrouterSpendUsd) && (
            <Segment
              testId="execution-rail-openrouter"
              label="OpenRouter"
              value={formatOpenRouterSpend(openrouterSpendUsd)}
              onClick={() => onNavigate('settings')}
            />
          )}
        </section>

        {budget && (
          <ContextWindowMeter
            usedTokens={budget.used_tokens}
            maxTokens={budget.max_context_tokens}
            thresholdTokens={budget.threshold_tokens}
            strategy={budget.strategy}
          />
        )}
      </Glass>
    </aside>
  );
}

