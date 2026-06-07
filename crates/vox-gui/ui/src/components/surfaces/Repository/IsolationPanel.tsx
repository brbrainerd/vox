import React from 'react';
import {
  ISOLATION_STRATEGIES,
  conflictRows,
  defaultStrategy,
  perAgentRows,
  strategyLabel,
  type IsolationStatus,
  type IsolationStrategy,
} from './isolationHelpers';

export interface IsolationPanelProps {
  /** Raw `isolation_status_json` payload, or null while it is unavailable. */
  status: IsolationStatus | null;
  /** Set the default isolation strategy (POST /api/v2/vcs/isolation/strategy). */
  onSetDefault?: (strategy: IsolationStrategy) => void;
  /** True while a status fetch or write is in flight. */
  busy?: boolean;
  /**
   * Optional note rendered when no live status is wired yet (the GUI's MCP/Tauri
   * transport does not yet expose the isolation REST surface — see panel docs).
   */
  unavailableNote?: string;
}

/**
 * Multi-agent VCS isolation panel (spec §5.4): shows the default strategy with a
 * selector, the per-agent overrides, and live conflict rows. Presentational — the
 * parent owns transport so this stays unit-testable and transport-agnostic.
 */
export function IsolationPanel({ status, onSetDefault, busy, unavailableNote }: IsolationPanelProps) {
  const current = defaultStrategy(status);
  const agents = perAgentRows(status);
  const conflicts = conflictRows(status);

  return (
    <section className="space-y-3">
      <h3 className="font-display text-sm text-zinc-100 tracking-wider uppercase">
        Multi-Agent Isolation
      </h3>

      {status === null && (
        <p className="text-xs text-amber-300/80">
          {unavailableNote ?? 'Live isolation status unavailable.'}
        </p>
      )}

      <div className="flex items-center gap-2">
        <label className="text-xs text-zinc-400" htmlFor="isolation-default">
          Default strategy
        </label>
        <select
          id="isolation-default"
          className="rounded-lg border border-white/10 bg-white/[0.03] px-2 py-1 text-sm text-zinc-200"
          value={current}
          disabled={busy || !onSetDefault}
          onChange={(e) => onSetDefault?.(e.target.value as IsolationStrategy)}
        >
          {ISOLATION_STRATEGIES.map((s) => (
            <option key={s} value={s}>
              {strategyLabel(s)}
            </option>
          ))}
        </select>
        <span className="text-xs text-zinc-500">active: {strategyLabel(current)}</span>
      </div>

      <div>
        <h4 className="mb-1 text-xs text-zinc-400">Per-agent overrides</h4>
        {agents.length === 0 ? (
          <p className="text-xs text-zinc-500">No per-agent overrides.</p>
        ) : (
          <table className="w-full text-left text-xs text-zinc-300">
            <thead className="text-zinc-500">
              <tr>
                <th className="py-1 pr-4">Agent</th>
                <th className="py-1">Strategy</th>
              </tr>
            </thead>
            <tbody>
              {agents.map((row) => (
                <tr key={row.agentId} className="border-t border-white/5">
                  <td className="py-1 pr-4 font-mono">A-{row.agentId}</td>
                  <td className="py-1">{strategyLabel(row.strategy)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div>
        <h4 className="mb-1 text-xs text-zinc-400">Active conflicts</h4>
        {conflicts.length === 0 ? (
          <p className="text-xs text-emerald-300/80">No active conflicts</p>
        ) : (
          <ul className="space-y-1 text-xs text-rose-300">
            {conflicts.map((c) => (
              <li key={c.id} className="font-mono">
                {c.path} <span className="text-zinc-500">— agents {c.sides.join(', ')}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
