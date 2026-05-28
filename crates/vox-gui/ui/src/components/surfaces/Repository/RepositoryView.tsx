import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

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
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runAction('Workspace status', ['status'])}
        >
          Workspace status
        </button>
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runAction('Repository health', ['check', 'workspace'])}
        >
          Repo health check
        </button>
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runAction('Vox check', ['check'])}
        >
          `vox check`
        </button>
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runAction('List changed files', ['diff'])}
        >
          `vox diff`
        </button>
      </div>
      <pre className="max-h-[420px] overflow-auto rounded-lg border border-white/10 bg-black/40 p-3 text-xs text-zinc-300">
        {output}
      </pre>
    </section>
  );
}
