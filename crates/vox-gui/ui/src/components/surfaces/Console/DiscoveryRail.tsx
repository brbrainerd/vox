import React, { useEffect, useRef, useState } from 'react';
import { discoveryHelp, discoveryRecord, type ActionHelp } from '../../../transport';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';
import { useLabel } from '../../../hooks/useLanguage';
import { Glass } from '../../ui/Glass';
import { useLocalStorage } from '../../../hooks/useLocalStorage';

interface Props {
  /** The action id currently under the cursor / top suggestion, or null. */
  actionId: string | null;
  /** Epoch ms (passed in so the component stays deterministic/testable). */
  nowMs: number;
  gamifyEnabled?: boolean;
  /** Apply the suggested example command to the console input. */
  onUseAction?: (example: string, actionId: string) => void;
}

const DISCOVERY_RAIL_COLLAPSED_KEY = 'gui.console.discovery_rail_collapsed.v1';

/**
 * Persistent right-hand rail (layout A). Resolves the active action to its help
 * and records a "seen" exposure (with dwell) so the spaced-repetition scheduler
 * learns what the user has been shown.
 */
export function DiscoveryRail({ actionId, nowMs, gamifyEnabled = false, onUseAction }: Props) {
  const [help, setHelp] = useState<ActionHelp | null>(null);
  const [collapsed, setCollapsed] = useLocalStorage<boolean>(DISCOVERY_RAIL_COLLAPSED_KEY, false);
  const shownAt = useRef<number>(nowMs);

  useEffect(() => {
    if (!actionId) {
      setHelp(null);
      return;
    }
    shownAt.current = nowMs;
    let live = true;
    discoveryHelp(actionId)
      .then((h) => live && setHelp(h))
      .catch(() => {});
    return () => {
      live = false;
    };
    // Keyed on actionId only: nowMs changes on unrelated parent re-renders and
    // must not re-fetch help.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [actionId]);

  // Record a "seen" exposure when the active action settles (debounced by 2s of
  // dwell — matches spec §discovery rail). Fire-and-forget.
  useEffect(() => {
    if (!actionId) return;
    const DWELL_MS = 2000;
    const t = setTimeout(() => {
      discoveryRecord(actionId, false, shownAt.current + DWELL_MS, DWELL_MS).catch(() => {});
    }, DWELL_MS);
    return () => clearTimeout(t);
    // Keyed on actionId only — see note above; restarting on nowMs would starve
    // the dwell timer.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [actionId]);

  if (collapsed) {
    return (
      <aside aria-label="discovery" className="shrink-0" data-testid="discovery-rail">
        <Glass className="flex flex-col items-center gap-2 p-2">
          <button
            type="button"
            aria-label="Expand discovery rail"
            aria-expanded={false}
            onClick={() => setCollapsed(false)}
            className="rounded-lg border border-border-subtle p-2 text-text-muted transition hover:border-brass/40 hover:text-brass"
          >
            <span className="font-mono text-sm" aria-hidden="true">
              »
            </span>
          </button>
        </Glass>
      </aside>
    );
  }

  const handleUse = () => {
    if (!actionId || !help) return;
    void recordGamifyGuiEvent(
      'discovery_action_used',
      { action_id: actionId },
      { enabled: gamifyEnabled },
    );
    onUseAction?.(help.example, actionId);
  };

  return (
    <aside
      aria-label="discovery"
      aria-live="polite"
      className="w-[280px] shrink-0"
      data-testid="discovery-rail"
    >
      <Glass className="flex h-full flex-col gap-2 p-3 text-xs">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-[10px] uppercase tracking-[0.18em] text-brass">{useLabel('con-discovery')}</h2>
          <button
            type="button"
            aria-label="Collapse discovery rail"
            aria-expanded={true}
            onClick={() => setCollapsed(true)}
            className="rounded-sm p-1 text-text-muted transition hover:bg-overlay-subtle hover:text-text-secondary"
          >
            <span className="font-mono text-xs" aria-hidden="true">
              «
            </span>
          </button>
        </div>

        {!help ? (
          <p className="text-text-muted">Start typing a vox command to see help and tips.</p>
        ) : (
          <>
            <h3 className="text-[13px] font-medium text-text-secondary">{help.example}</h3>
            <p className="text-text-muted">{help.about}</p>
            {help.args.length > 0 && (
              <ul className="list-disc pl-4 text-text-muted">
                {help.args.map((a) => (
                  <li key={a.name}>
                    <code className="text-text-secondary">{a.name}</code>
                    {a.required ? ' (required)' : ''} — {a.help}
                  </li>
                ))}
              </ul>
            )}
            <button
              type="button"
              onClick={handleUse}
              aria-label={`Use suggested action ${help.example}`}
              className="mt-2 self-start rounded-lg border border-border-subtle px-2.5 py-1.5 text-[11px] text-text-muted transition hover:border-brass/40 hover:text-brass"
            >
              Use
            </button>
          </>
        )}
      </Glass>
    </aside>
  );
}
