import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listenContextArchived } from '../../../transport';
import { Button } from '../../ui/Button';

export interface ContextWindowArchiveControlProps {
  /** The context_windows.id for the active window. May be undefined if no window is active. */
  activeWindowId?: string | null;
}

type Tier = 'hot' | 'warm' | 'cold' | null;

const VALID_TIERS = new Set<string>(['hot', 'warm', 'cold']);
function toTier(s: string): Tier {
  return VALID_TIERS.has(s) ? (s as Tier) : null;
}

const TIER_LABEL: Record<NonNullable<Tier>, string> = {
  hot: 'Hot',
  warm: 'Warm',
  cold: 'Cold',
};

const TIER_COLOR: Record<NonNullable<Tier>, string> = {
  hot: 'text-[oklch(0.7_0.15_25)]',   // red-orange
  warm: 'text-[oklch(0.75_0.15_60)]', // amber
  cold: 'text-[oklch(0.7_0.12_220)]', // blue
};

interface ContextWindowInfoResult {
  tier: string;
}

export function ContextWindowArchiveControl({ activeWindowId }: ContextWindowArchiveControlProps) {
  const [tier, setTier] = useState<Tier>(null);
  const [archiving, setArchiving] = useState(false);

  // Fetch tier info when activeWindowId changes
  useEffect(() => {
    if (!activeWindowId) {
      setTier(null);
      return;
    }
    let cancelled = false;
    invoke<ContextWindowInfoResult>('get_context_window_info', { windowId: activeWindowId })
      .then((info) => {
        if (!cancelled) setTier(toTier(info.tier));
      })
      .catch((e: unknown) => {
        console.error('ContextWindowArchiveControl: get_context_window_info failed', e);
        if (!cancelled) setTier(null);
      });
    return () => { cancelled = true; };
  }, [activeWindowId]);

  // Subscribe to archive events
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    listenContextArchived((payload) => {
      if (payload.window_id === activeWindowId) {
        setTier(toTier(payload.tier));
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    }).catch((e: unknown) => {
      console.warn('ContextWindowArchiveControl: listenContextArchived unavailable', e);
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [activeWindowId]);

  const handleArchive = useCallback(async () => {
    if (!activeWindowId) return;
    setArchiving(true);
    try {
      await invoke('archive_context_window', { windowId: activeWindowId });
    } catch (err) {
      console.error('[ContextWindowArchiveControl] archive failed:', err);
    } finally {
      setArchiving(false);
    }
  }, [activeWindowId]);

  return (
    <div className="flex flex-col gap-1 px-2 py-1" data-testid="archive-control">
      <div className="flex items-center justify-between">
        <span className="text-[9px] uppercase tracking-[0.14em] text-zinc-500">Storage</span>
        {tier != null && (
          <span className={`font-mono text-[10px] tabular-nums ${TIER_COLOR[tier]}`}>
            {TIER_LABEL[tier]}
          </span>
        )}
      </div>
      {tier !== 'cold' && activeWindowId != null && (
        <Button
          size="xs"
          variant="ghost"
          disabled={archiving}
          onClick={handleArchive}
          aria-label={archiving ? 'Archiving context window' : 'Archive context window'}
          className="justify-center text-zinc-500 hover:text-zinc-300"
        >
          {archiving ? 'Archiving…' : 'Archive'}
        </Button>
      )}
    </div>
  );
}
