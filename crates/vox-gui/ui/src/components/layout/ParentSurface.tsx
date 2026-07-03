import React, { useMemo } from 'react';
import { SURFACE_REGISTRY } from '../../generated/surfaceRegistry.generated';
import { SubTabs } from './SubTabs';
import { useLocalStorage } from '../../hooks/useLocalStorage';
import { useLang } from '../../hooks/useLanguage';
import { labelFor } from '../../lib/lexicon';
import { orderedChildren } from '../../lib/navigation';

interface ParentSurfaceProps {
  parentKey: string;
  activeChild: string;
  onChildChange: (viewKey: string) => void;
  renderChild: (viewKey: string) => React.ReactNode;
}

export function ParentSurface({
  parentKey,
  activeChild,
  onChildChange,
  renderChild,
}: ParentSurfaceProps) {
  const { lang } = useLang();
  const [lastTabs] = useLocalStorage<Record<string, string>>('vox_parent_tabs', {});

  const tabs = useMemo(() => {
    const raw = SURFACE_REGISTRY
      .filter(e => e.parentSurface === parentKey && e.viewKey && e.navLabel)
      .map(e => ({ viewKey: e.viewKey as string, label: labelFor(e.viewKey as string, lang) }));
    const order = orderedChildren(parentKey, raw.map(t => t.viewKey));
    const rank = new Map(order.map((k, i) => [k, i]));
    return [...raw].sort((a, b) => (rank.get(a.viewKey) ?? 0) - (rank.get(b.viewKey) ?? 0));
  }, [parentKey, lang]);

  const child =
    tabs.some(t => t.viewKey === activeChild)
      ? activeChild
      : lastTabs[parentKey] ?? tabs[0]?.viewKey ?? activeChild;

  return (
    <div className="flex flex-col min-h-0">
      <SubTabs parentKey={parentKey} tabs={tabs} activeChild={child} onSelect={onChildChange} />
      {renderChild(child)}
    </div>
  );
}
