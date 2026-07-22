import React, { useState } from 'react';
import { ActivitySurface } from '../Activity/ActivitySurface';
import { DiscoveryInbox } from '../Scientia/DiscoveryInbox';
import { DiscoveryReview } from '../Scientia/DiscoveryReview';
import { ArchivePanel } from '../Scientia/ArchivePanel';
import { DISCOVERY_PRESET_SEED_KEY } from '../../../lib/navigation';
import type { Toast } from '../../../types/tauri';

export type DiscoveryPreset = 'timeline' | 'inbox' | 'review' | 'archive';

const PRESETS: Array<{ id: DiscoveryPreset; label: string }> = [
  { id: 'timeline', label: 'Timeline' },
  { id: 'inbox', label: 'Inbox' },
  { id: 'review', label: 'Review' },
  { id: 'archive', label: 'Archive' },
];

function consumeSeed(): DiscoveryPreset {
  try {
    const seed = window.localStorage.getItem(DISCOVERY_PRESET_SEED_KEY);
    if (seed === 'inbox' || seed === 'review' || seed === 'archive') {
      window.localStorage.removeItem(DISCOVERY_PRESET_SEED_KEY);
      return seed;
    }
  } catch {
    /* localStorage unavailable */
  }
  return 'timeline';
}

export interface DiscoverySurfaceProps {
  pushToast: (t: Toast) => void;
  gamifyEnabled?: boolean;
  condensed?: boolean;
}

/**
 * One Discovery surface (view key `activity`) absorbing the four former
 * activity clones: Timeline (activity_query), Inbox, Review, Archive
 * (gui-ia-blueprint §4 MERGE: archive-panel/discovery-inbox/discovery-review → activity).
 */
export function DiscoverySurface({ pushToast, gamifyEnabled, condensed }: DiscoverySurfaceProps) {
  const [preset, setPreset] = useState<DiscoveryPreset>(consumeSeed);

  if (condensed) {
    // Same `preset` state the tab strip already tracks — the four nested
    // surfaces (Timeline/Inbox/Review/Archive) are each substantial widgets
    // that don't fit narrow, so condensed just names which one is active.
    const activeLabel = PRESETS.find(p => p.id === preset)?.label ?? preset;
    return (
      <div className="p-2 text-[11px] text-text-muted">
        <div className="mb-1 text-xs font-semibold uppercase tracking-wider text-text-primary">Discovery</div>
        <div>{activeLabel}</div>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-col">
      <div role="tablist" aria-label="Discovery presets" className="flex gap-1 px-4 pt-3">
        {PRESETS.map(p => (
          <button
            key={p.id}
            type="button"
            role="tab"
            aria-selected={preset === p.id}
            onClick={() => setPreset(p.id)}
            className={`rounded-md px-3 py-1.5 font-display text-[11px] uppercase tracking-[0.16em] transition ${
              preset === p.id
                ? 'bg-overlay-subtle text-brass'
                : 'text-text-muted hover:bg-overlay-hover hover:text-text-secondary'
            }`}
          >
            {p.label}
          </button>
        ))}
      </div>
      {preset === 'timeline' && <ActivitySurface pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
      {preset === 'inbox' && <DiscoveryInbox pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
      {preset === 'review' && <DiscoveryReview pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
      {preset === 'archive' && <ArchivePanel pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
    </div>
  );
}
