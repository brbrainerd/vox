import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';

/** Chat-surface model pick. The pick is lifted to App state and threaded into
 *  the chat submit payload as the `model_override` enqueue hint — the one
 *  channel the daemon consumes (TaskEnqueueHints.model_override →
 *  AgentTask.model_override → StreamRoute::UserModelOverride). Deliberately
 *  NOT `set_active_model`, which only touches the GUI process (Resolved
 *  decision "Item 4"). `null` pick = auto-route (clear the override). */
interface ProviderStatus {
  provider: string;
  key_present: boolean;
  is_local: boolean;
  local_reachable: boolean | null;
}

/** True when the picker should refuse this provider — no key for a cloud
 *  provider, or the cached local-server probe reports it unreachable. This
 *  is what keeps the picker's list in sync with the BackendAvailability
 *  strip; without it a user could pick a model the strip is simultaneously
 *  showing as unavailable, and the request would fail 100% of the time. */
function isProviderUnavailable(provider: string | undefined, statuses: ProviderStatus[]): boolean {
  if (!provider) return false;
  const s = statuses.find(x => x.provider.toLowerCase() === provider.toLowerCase());
  if (!s) return false;
  if (s.is_local) return s.local_reachable === false;
  return !s.key_present;
}

export function ChatModelPicker({
  activeModel,
  onApplied,
}: {
  activeModel?: string | null;
  onApplied?: (modelId: string | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const [models, setModels] = useState<Array<{ id: string; provider?: string }>>([]);
  const [statuses, setStatuses] = useState<ProviderStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  // Escape + outside-click close (Escape pattern mirrors ChatSurface's routing
  // drawer; outside-click mirrors ChatSessionRail's menu dismiss). Without
  // these, only re-toggling or a selection closes the listbox.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    const onPointerDown = (e: PointerEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', onPointerDown);
    return () => {
      window.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', onPointerDown);
    };
  }, [open]);

  const toggle = async () => {
    const next = !open;
    setOpen(next);
    if (next && models.length === 0) {
      try {
        const [cards, providerStatuses] = await Promise.all([
          invoke<Array<{ id: string; provider?: string }>>('list_model_cards', { limit: 120 }),
          invoke<ProviderStatus[]>('inference_provider_status'),
        ]);
        setModels(Array.isArray(cards) ? cards : []);
        setStatuses(Array.isArray(providerStatuses) ? providerStatuses : []);
      } catch (e) {
        setError(sanitizeErrorForToast(e));
      }
    }
  };

  const apply = (id: string | null, unavailable: boolean) => {
    if (unavailable) return;
    onApplied?.(id);
    setOpen(false);
  };

  return (
    <div ref={rootRef} className="relative">
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
          className="absolute bottom-full left-0 z-50 mb-1 max-h-64 w-72 overflow-y-auto rounded-lg border border-border-subtle bg-bg-base p-1 custom-scrollbar"
        >
          <li key="auto-route">
            <button
              type="button"
              role="option"
              aria-selected={activeModel == null}
              onClick={() => apply(null, false)}
              className="w-full truncate rounded-sm px-2 py-1 text-left font-mono text-[10px] text-text-secondary hover:bg-overlay-subtle"
            >
              auto-route (clear override)
            </button>
          </li>
          {models.map(m => {
            const unavailable = isProviderUnavailable(m.provider, statuses);
            return (
              <li key={m.id}>
                <button
                  type="button"
                  role="option"
                  aria-selected={m.id === activeModel}
                  aria-disabled={unavailable}
                  disabled={unavailable}
                  title={unavailable ? `${m.provider} is currently unavailable (no key or unreachable)` : undefined}
                  onClick={() => apply(m.id, unavailable)}
                  className={`w-full truncate rounded px-2 py-1 text-left font-mono text-[10px] ${
                    unavailable
                      ? 'cursor-not-allowed text-text-muted/50'
                      : 'text-text-secondary hover:bg-overlay-subtle'
                  }`}
                >
                  {m.id}
                  {unavailable ? ' (unavailable)' : ''}
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {error && <div role="alert" className="mt-1 text-[10px] text-rose-400">{error}</div>}
    </div>
  );
}
