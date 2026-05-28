import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { voxTransport } from '../../../transport';

interface HarnessViewProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

interface ModelCard {
  model_id: string;
  display_name: string;
}

interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

export function HarnessView({ pushToast }: HarnessViewProps) {
  const [models, setModels] = useState<ModelCard[]>([]);
  const [selectedModel, setSelectedModel] = useState('');
  const [task, setTask] = useState('Implement the requested Vox code change in this repository');
  const [output, setOutput] = useState('Ready.');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    voxTransport
      .listModels(80)
      .then((cards: any) => {
        const list = Array.isArray(cards) ? cards : [];
        setModels(list);
        if (list.length > 0) {
          setSelectedModel(list[0].model_id);
        }
      })
      .catch(() => {
        setModels([]);
      });
  }, []);

  const runHarness = async () => {
    if (!task.trim()) {
      pushToast({ tone: 'warn', title: 'Task required', body: 'Please provide a coding task.' });
      return;
    }
    setBusy(true);
    try {
      if (selectedModel) {
        await voxTransport.setActiveModel(selectedModel);
      }
      await invoke('submit_orchestrator_task', {
        input: {
          description: task.trim(),
          files: ['.'],
          priority: 'normal',
          session_id: 'gui-harness',
        },
      });
      const verify = await invoke<ExecuteOutput>('execute_command', {
        path: ['check'],
        args: { __argv: [] },
      });
      const text = [verify.stdout, verify.stderr].filter(Boolean).join('\n').trim();
      setOutput(text || '(no output)');
      pushToast({
        tone: verify.exit_code === 0 ? 'ok' : 'warn',
        title: 'Harness run',
        body: verify.exit_code === 0 ? 'Submitted and verified with `vox check`' : 'Submitted, but `vox check` failed',
      });
    } catch (err) {
      setOutput(String(err));
      pushToast({ tone: 'warn', title: 'Harness failed', body: String(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="space-y-4">
      <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Coding Harness</h2>
      <div className="grid gap-3">
        <label className="text-xs uppercase tracking-wider text-zinc-400">
          Model
          <select
            className="mt-1 w-full rounded-lg border border-white/10 bg-zinc-950 px-3 py-2 text-sm text-zinc-200"
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
            disabled={busy}
          >
            {models.map((m) => (
              <option key={m.model_id} value={m.model_id}>
                {m.display_name ?? m.model_id}
              </option>
            ))}
          </select>
        </label>
        <label className="text-xs uppercase tracking-wider text-zinc-400">
          Task
          <textarea
            className="mt-1 min-h-[88px] w-full rounded-lg border border-white/10 bg-zinc-950 px-3 py-2 text-sm text-zinc-200"
            value={task}
            onChange={(e) => setTask(e.target.value)}
            disabled={busy}
          />
        </label>
        <button
          className="rounded-lg border border-brass/40 bg-brass/15 px-3 py-2 text-sm text-brass hover:bg-brass/25 disabled:opacity-50"
          onClick={runHarness}
          disabled={busy}
        >
          Run harness path
        </button>
      </div>
      <pre className="max-h-[380px] overflow-auto rounded-lg border border-white/10 bg-black/40 p-3 text-xs text-zinc-300">
        {output}
      </pre>
    </section>
  );
}
