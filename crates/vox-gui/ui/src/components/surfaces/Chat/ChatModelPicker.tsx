import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

/** Chat-surface model pick. The pick is lifted to App state and threaded into
 *  the chat submit payload as the `model_override` enqueue hint — the one
 *  channel the daemon consumes (TaskEnqueueHints.model_override →
 *  AgentTask.model_override → StreamRoute::UserModelOverride). Deliberately
 *  NOT `set_active_model`, which only touches the GUI process (Resolved
 *  decision "Item 4"). `null` pick = auto-route (clear the override). */
export function ChatModelPicker({
  activeModel,
  onApplied,
}: {
  activeModel?: string | null;
  onApplied?: (modelId: string | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const [models, setModels] = useState<Array<{ id: string }>>([]);
  const [error, setError] = useState<string | null>(null);

  const toggle = async () => {
    const next = !open;
    setOpen(next);
    if (next && models.length === 0) {
      try {
        const cards = await invoke<Array<{ id: string }>>('list_model_cards', { limit: 120 });
        setModels(Array.isArray(cards) ? cards : []);
      } catch (e) {
        setError(String(e));
      }
    }
  };

  const apply = (id: string | null) => {
    onApplied?.(id);
    setOpen(false);
  };

  return (
    <div className="relative">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => void toggle()}
        className="rounded-lg border border-border-subtle px-2 py-1 font-mono text-[10px] text-text-muted hover:text-brass"
      >
        model: {activeModel ?? 'auto-route'}
      </button>
      {open && (
        <ul
          role="listbox"
          aria-label="Pick model for this chat"
          className="absolute z-50 mt-1 max-h-64 w-72 overflow-y-auto rounded-lg border border-border-subtle bg-bg-base p-1 custom-scrollbar"
        >
          <li key="auto-route">
            <button
              type="button"
              role="option"
              aria-selected={activeModel == null}
              onClick={() => apply(null)}
              className="w-full truncate rounded px-2 py-1 text-left font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle"
            >
              auto-route (clear override)
            </button>
          </li>
          {models.map(m => (
            <li key={m.id}>
              <button
                type="button"
                role="option"
                aria-selected={m.id === activeModel}
                onClick={() => apply(m.id)}
                className="w-full truncate rounded px-2 py-1 text-left font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle"
              >
                {m.id}
              </button>
            </li>
          ))}
        </ul>
      )}
      {error && <div role="alert" className="mt-1 text-[10px] text-rose-400">{error}</div>}
    </div>
  );
}
