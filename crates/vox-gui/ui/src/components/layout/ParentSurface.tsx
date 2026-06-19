import React, { useMemo } from 'react';
import { SURFACE_REGISTRY } from '../../generated/surfaceRegistry.generated';
import { SubTabs } from './SubTabs';
import { useLocalStorage } from '../../hooks/useLocalStorage';
import { DEFAULT_CHILD_BY_PARENT, labelForNavKey } from '../../lib/navigation';

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
    () => {
      const childTabs = SURFACE_REGISTRY
        .filter(e => e.parentSurface === parentKey && e.viewKey && e.navLabel)
        .map(e => ({ viewKey: e.viewKey as string, label: e.navLabel as string }));
      // A top-level parent can also BE a content surface (e.g. `settings`, whose
      // own view is its default child). Such parents have `parentSurface: null`
      // in the registry, so their own view never appears among the child tabs and
      // ParentSurface would fall through to the first registered child (e.g.
      // `coverage`), leaving the parent's panel unreachable. Prepend the parent's
      // own surface as the primary tab so it renders by default.
      const selfDefaults =
        DEFAULT_CHILD_BY_PARENT[parentKey] === parentKey &&
        !childTabs.some(t => t.viewKey === parentKey);
      return selfDefaults
        ? [{ viewKey: parentKey, label: labelForNavKey(parentKey) }, ...childTabs]
        : childTabs;
    },
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
