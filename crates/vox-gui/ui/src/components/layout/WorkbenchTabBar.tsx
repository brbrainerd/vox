import React from 'react';
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

export function WorkbenchTabBar({ tabs, activeTab, onSelect, onClose }: WorkbenchTabBarProps) {
  if (tabs.length === 0) return null;

  return (
    <div
      role="tablist"
      aria-label="Open surfaces"
      className="mb-3 flex flex-wrap items-center gap-1 border-b border-border-subtle pb-2"
      data-testid="workbench-tab-bar"
    >
      {tabs.map((tab) => {
        const selected = activeTab === tab.id;
        return (
          <div
            key={tab.id}
            className={`group flex items-center gap-0.5 rounded-md pl-2 pr-1 py-1 transition ${
              selected
                ? 'bg-overlay-subtle text-text-primary ring-1 ring-white/10'
                : 'text-text-muted hover:bg-overlay-subtle hover:text-text-secondary'
            }`}
          >
            <button
              type="button"
              role="tab"
              aria-selected={selected}
              data-testid={`workbench-tab-${tab.id}`}
              onClick={() => onSelect(tab.id)}
              className="font-display text-[10px] uppercase tracking-[0.18em]"
            >
              {tab.label}
              {tab.badge != null && tab.badge > 0 ? (
                <span className="ml-1.5 rounded-full bg-brass/20 px-1.5 text-[9px] text-brass">
                  {tab.badge}
                </span>
              ) : null}
            </button>
            {!tab.pinned ? (
              <button
                type="button"
                aria-label={`Close ${tab.label}`}
                onClick={(e) => {
                  e.stopPropagation();
                  onClose(tab.id);
                }}
                className="flex size-5 items-center justify-center rounded opacity-60 transition hover:bg-white/10 hover:opacity-100"
              >
                <Icon.x className="size-3" aria-hidden="true" />
              </button>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
