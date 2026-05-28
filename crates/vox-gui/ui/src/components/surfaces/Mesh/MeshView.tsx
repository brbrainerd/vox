import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface MeshViewProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

export function MeshView({ pushToast }: MeshViewProps) {
  const [output, setOutput] = useState('Mesh telemetry has not been queried yet.');
  const [busy, setBusy] = useState(false);

  const runMesh = async (profile: 'm1m4' | 'training') => {
    setBusy(true);
    setOutput(`Running mesh gate profile: ${profile}`);
    try {
      const result = await invoke<ExecuteOutput>('execute_command', {
        path: ['ci', 'mesh-gate'],
        args: { profile, __argv: [] },
      });
      const text = [result.stdout, result.stderr].filter(Boolean).join('\n').trim();
      setOutput(text || '(no output)');
      pushToast({
        tone: result.exit_code === 0 ? 'ok' : 'warn',
        title: `Mesh profile ${profile}`,
        body: result.exit_code === 0 ? 'Completed' : `Failed (exit ${result.exit_code})`,
      });
    } catch (err) {
      setOutput(String(err));
      pushToast({ tone: 'warn', title: 'Mesh refresh failed', body: String(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="space-y-4">
      <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Mesh Control</h2>
      <div className="flex flex-wrap gap-2">
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runMesh('m1m4')}
        >
          Refresh m1m4 profile
        </button>
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-sm hover:bg-white/[0.06]"
          disabled={busy}
          onClick={() => runMesh('training')}
        >
          Refresh training profile
        </button>
      </div>
      <pre className="max-h-[420px] overflow-auto rounded-lg border border-white/10 bg-black/40 p-3 text-xs text-zinc-300">
        {output}
      </pre>
    </section>
  );
}
