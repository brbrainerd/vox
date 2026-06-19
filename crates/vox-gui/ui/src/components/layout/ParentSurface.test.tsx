// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { render } from '@testing-library/react';
import { ParentSurface } from './ParentSurface';

// SubTabs is exercised elsewhere; stub it so this test focuses on which child
// view ParentSurface chooses to render.
vi.mock('./SubTabs', () => ({
  SubTabs: () => <div data-testid="subtabs" />,
}));

function renderParent(parentKey: string, activeChild: string) {
  const seen: string[] = [];
  render(
    <ParentSurface
      parentKey={parentKey}
      activeChild={activeChild}
      onChildChange={() => {}}
      renderChild={(vk) => {
        seen.push(vk);
        return <div>child:{vk}</div>;
      }}
    />,
  );
  return seen;
}

describe('ParentSurface child resolution', () => {
  it('renders the parent\'s own view for a self-defaulting top-level parent (settings)', () => {
    // `settings` has parentSurface:null in the registry, so without the
    // self-default fix ParentSurface fell through to the first child tab
    // (`coverage`) and the Settings panel was unreachable.
    const seen = renderParent('settings', 'settings');
    expect(seen).toContain('settings');
    expect(seen).not.toContain('coverage');
  });

  it('still resolves a normal parent to its registered child', () => {
    const seen = renderParent('workspace', 'console');
    expect(seen).toContain('console');
  });
});
