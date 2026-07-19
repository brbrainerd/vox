import React, { useRef } from 'react';
import { Icon } from '../ui/Icons';

export interface WorkbenchTabItem {
  id: string;
  label: string;
  badge?: number;
  /** Pinned tabs (e.g. Chat) cannot be closed from the tab bar. */
  pinned?: boolean;
}

interface WorkbenchTabBarProps {
  tabs: WorkbenchTabItem[];
  activeTab: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
}

// APG tablist keyboard pattern, "automatic activation" variant: arrow keys move
// DOM focus AND select the newly-focused tab immediately (Enter/Space are still
// accepted for parity with "manual activation" callers/tests, but are redundant
// here since focus already triggers selection). This matches the existing code's
// prior behavior of tying `selected` directly to activeTab/tabIndex, so no extra
// "focused but not yet selected" state is introduced.
export function WorkbenchTabBar({ tabs, activeTab, onSelect, onClose }: WorkbenchTabBarProps) {
  const tabRefs = useRef<Record<string, HTMLDivElement | null>>({});

  if (tabs.length === 0) return null;

  const focusAndSelect = (id: string) => {
    onSelect(id);
    tabRefs.current[id]?.focus();
  };

  return (
    <div
      role="tablist"
      aria-label="Open surfaces"
      className="mb-3 flex flex-wrap items-center gap-1 border-b border-border-subtle pb-2"
      data-testid="workbench-tab-bar"
    >
      {tabs.map((tab, index) => {
        const selected = activeTab === tab.id;
        return (
          <div
            key={tab.id}
            ref={(el) => {
              tabRefs.current[tab.id] = el;
            }}
            role="tab"
            aria-selected={selected}
            aria-keyshortcuts="Delete"
            tabIndex={selected ? 0 : -1}
            data-testid={`workbench-tab-${tab.id}`}
            onClick={() => onSelect(tab.id)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') onSelect(tab.id);
              if (e.key === 'Delete' && !tab.pinned) onClose(tab.id);
              if (e.key === 'ArrowRight') {
                e.preventDefault();
                const next = tabs[(index + 1) % tabs.length];
                focusAndSelect(next.id);
              }
              if (e.key === 'ArrowLeft') {
                e.preventDefault();
                const prev = tabs[(index - 1 + tabs.length) % tabs.length];
                focusAndSelect(prev.id);
              }
              if (e.key === 'Home') {
                e.preventDefault();
                focusAndSelect(tabs[0].id);
              }
              if (e.key === 'End') {
                e.preventDefault();
                focusAndSelect(tabs[tabs.length - 1].id);
              }
            }}
            className={`group flex cursor-pointer items-center gap-0.5 rounded-md pl-2 pr-1 py-1 transition ${
              selected
                ? 'bg-overlay-subtle text-text-primary ring-1 ring-white/10'
                : 'text-text-muted hover:bg-overlay-subtle hover:text-text-secondary'
            }`}
          >
            <span className="font-display text-[10px] uppercase tracking-[0.18em]">
              {tab.label}
              {tab.badge != null && tab.badge > 0 ? (
                <span className="ml-1.5 rounded-full bg-brass/20 px-1.5 text-[9px] text-brass">
                  {tab.badge}
                </span>
              ) : null}
            </span>
            {!tab.pinned ? (
              <span
                aria-hidden="true"
                data-testid={`workbench-tab-close-${tab.id}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onClose(tab.id);
                }}
                className="flex size-5 cursor-pointer items-center justify-center rounded opacity-60 transition hover:bg-white/10 hover:opacity-100"
              >
                <Icon.x className="size-3" />
              </span>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
