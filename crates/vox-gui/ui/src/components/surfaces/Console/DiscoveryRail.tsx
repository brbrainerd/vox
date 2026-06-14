import React, { useEffect, useRef, useState } from 'react';
import { discoveryHelp, discoveryRecord, type ActionHelp } from '../../../transport';

interface Props {
  /** The action id currently under the cursor / top suggestion, or null. */
  actionId: string | null;
  /** Epoch ms (passed in so the component stays deterministic/testable). */
  nowMs: number;
}

/**
 * Persistent right-hand rail (layout A). Resolves the active action to its help
 * and records a "seen" exposure (with dwell) so the spaced-repetition scheduler
 * learns what the user has been shown.
 */
export function DiscoveryRail({ actionId, nowMs }: Props) {
  const [help, setHelp] = useState<ActionHelp | null>(null);
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

  if (!help) {
    return (
      <aside
        aria-label="discovery"
        aria-live="polite"
        style={{ width: 280, padding: 12, fontSize: 12 }}
      >
        <p className="text-text-muted">Start typing a vox command to see help and tips.</p>
      </aside>
    );
  }

  return (
    <aside
      aria-label="discovery"
      aria-live="polite"
      style={{ width: 280, padding: 12, fontSize: 12 }}
    >
      <h3 style={{ fontSize: 13, margin: '0 0 6px' }}>{help.example}</h3>
      <p style={{ margin: '0 0 8px' }}>{help.about}</p>
      {help.args.length > 0 && (
        <ul style={{ margin: 0, paddingLeft: 16 }}>
          {help.args.map((a) => (
            <li key={a.name}>
              <code>{a.name}</code>
              {a.required ? ' (required)' : ''} — {a.help}
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}
