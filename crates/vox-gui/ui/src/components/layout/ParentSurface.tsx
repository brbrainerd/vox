import React, { useMemo } from 'react';
import { SURFACE_REGISTRY } from '../../generated/surfaceRegistry.generated';
import { SubTabs } from './SubTabs';
import { useLocalStorage } from '../../hooks/useLocalStorage';

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
  const [lastTabs] = useLocalStorage<Record<string, string>>('vox_parent_tabs', {});

  const tabs = useMemo(
    () =>
      SURFACE_REGISTRY
        .filter(e => e.parentSurface === parentKey && e.viewKey && e.navLabel)
        .map(e => ({ viewKey: e.viewKey as string, label: e.navLabel as string })),
    [parentKey],
  );

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
