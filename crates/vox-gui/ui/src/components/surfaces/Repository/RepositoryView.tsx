import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { IsolationPanel } from './IsolationPanel';
import type { IsolationStatus, IsolationStrategy } from './isolationHelpers';

interface RepositoryViewProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

async function run(path: string[], argv: string[] = []): Promise<ExecuteOutput> {
  return invoke<ExecuteOutput>('execute_command', {
    path,
    args: { __argv: argv },
  });
}

export function RepositoryView({ pushToast }: RepositoryViewProps) {
  const [output, setOutput] = useState('No command run yet.');
  const [busy, setBusy] = useState(false);

  // Live VCS isolation state (default strategy + per-agent + active conflicts),
  // bridged from the orchestrator daemon via Tauri invoke.
  const [isolation, setIsolation] = useState<IsolationStatus | null>(null);
  const [isolationBusy, setIsolationBusy] = useState(false);
  const [isolationError, setIsolationError] = useState<string | null>(null);

  const refetchIsolation = useCallback(async () => {
    try {
      const status = await invoke<IsolationStatus>('get_vcs_isolation');
      setIsolation(status);
      setIsolationError(null);
    } catch (err) {
      setIsolation(null);
      setIsolationError(String(err));
    }
  }, []);

  // Fetch once on mount. Mutations refetch inline (the daemon also publishes a
  // `vcs.isolation.changed` topic for future push-driven refresh).
  useEffect(() => {
    void refetchIsolation();
  }, [refetchIsolation]);

  const handleSetDefault = useCallback(
    async (strategy: IsolationStrategy) => {
      setIsolationBusy(true);
      try {
        const status = await invoke<IsolationStatus>('set_vcs_isolation_strategy', {
          default: strategy,
          agentId: null,
          strategy: null,
        });
        setIsolation(status);
        setIsolationError(null);
        pushToast({ tone: 'ok', title: 'Isolation strategy', body: `Default → ${strategy}` });
      } catch (err) {
        setIsolationError(String(err));
        pushToast({ tone: 'warn', title: 'Isolation strategy', body: String(err) });
        // Reconcile against authoritative daemon state after a failed write.
        void refetchIsolation();
      } finally {
        setIsolationBusy(false);
      }
    },
    [pushToast, refetchIsolation],
  );

  const runAction = async (label: string, path: string[], argv: string[] = []) => {
    setBusy(true);
    setOutput(`Running: vox ${path.join(' ')} ${argv.join(' ')}`.trim());
    try {
      const result = await run(path, argv);
      const text = [result.stdout, result.stderr].filter(Boolean).join('\n').trim();
      setOutput(text || '(no output)');
      pushToast({
        tone: result.exit_code === 0 ? 'ok' : 'warn',
        title: label,
        body: result.exit_code === 0 ? 'Completed' : `Failed (exit ${result.exit_code})`,
      });
    } catch (err) {
      setOutput(String(err));
      pushToast({ tone: 'warn', title: label, body: String(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="space-y-4">
      <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Repository Harness</h2>
      <div className="grid gap-2 sm:grid-cols-2">
        <button
          type="button"
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runAction('Workspace status', ['status'])}
        >
          Workspace status
        </button>
        <button
          type="button"
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runAction('Repository health', ['check', 'workspace'])}
        >
          Repo health check
        </button>
        <button
          type="button"
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runAction('Vox check', ['check'])}
        >
          `vox check`
        </button>
        <button
          type="button"
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runAction('List changed files', ['diff'])}
        >
          `vox diff`
        </button>
      </div>
      <pre
        aria-label="Command output"
        aria-live="polite"
        aria-busy={busy}
        className="max-h-[420px] overflow-auto rounded-lg border border-white/10 bg-black/40 p-3 text-xs text-zinc-300"
      >
        {output}
      </pre>
      <div className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
        <IsolationPanel
          status={isolation}
          onSetDefault={handleSetDefault}
          busy={isolationBusy}
          unavailableNote={
            isolationError
              ? `Live isolation status unavailable: ${isolationError}`
              : 'Live isolation status is loading from the orchestrator daemon…'
          }
        />
      </div>
    </section>
  );
}
