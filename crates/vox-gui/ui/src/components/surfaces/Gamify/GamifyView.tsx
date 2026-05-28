import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface GamifyViewProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

export function GamifyView({ pushToast }: GamifyViewProps) {
  const [output, setOutput] = useState('No Ludus operation executed yet.');
  const [busy, setBusy] = useState(false);

  const runLudus = async (path: string[]) => {
    setBusy(true);
    try {
      const result = await invoke<ExecuteOutput>('execute_command', {
        path,
        args: { __argv: [] },
      });
      const text = [result.stdout, result.stderr].filter(Boolean).join('\n').trim();
      setOutput(text || '(no output)');
      pushToast({
        tone: result.exit_code === 0 ? 'ok' : 'warn',
        title: `vox ${path.join(' ')}`,
        body: result.exit_code === 0 ? 'Completed' : `Failed (exit ${result.exit_code})`,
      });
    } catch (err) {
      setOutput(String(err));
      pushToast({ tone: 'warn', title: 'Ludus action failed', body: String(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="space-y-4">
      <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Gamification</h2>
      <div className="grid gap-2 sm:grid-cols-2">
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runLudus(['ludus', 'hud'])}
        >
          Open HUD feed
        </button>
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runLudus(['ludus', 'profile'])}
        >
          Show profile
        </button>
      </div>
      <pre className="max-h-[420px] overflow-auto rounded-lg border border-white/10 bg-black/40 p-3 text-xs text-zinc-300">
        {output}
      </pre>
    </section>
  );
}
