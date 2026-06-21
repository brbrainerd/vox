import React from 'react';
import { useLocalStorage } from '../../hooks/useLocalStorage';

export interface SubTabItem {
  viewKey: string;
  label: string;
}

interface SubTabsProps {
  parentKey: string;
  tabs: SubTabItem[];
  activeChild: string;
  onSelect: (viewKey: string) => void;
}

export function SubTabs({ parentKey, tabs, activeChild, onSelect }: SubTabsProps) {
  const [, setLast] = useLocalStorage<Record<string, string>>('vox_parent_tabs', {});

  const select = (viewKey: string) => {
    setLast(prev => ({ ...prev, [parentKey]: viewKey }));
    onSelect(viewKey);
  };

  if (tabs.length <= 1) return null;

  return (
    <div className="mb-4 flex flex-wrap gap-1 border-b border-border-subtle pb-2">
      {tabs.map(t => (
        <button
          key={t.viewKey}
          type="button"
          onClick={() => select(t.viewKey)}
          className={`rounded-md px-3 py-1.5 font-display text-[10px] uppercase tracking-[0.18em] transition ${
            activeChild === t.viewKey
              ? 'bg-overlay-subtle text-text-primary ring-1 ring-white/10'
              : 'text-text-muted hover:text-text-secondary hover:bg-overlay-subtle'
          }`}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}
