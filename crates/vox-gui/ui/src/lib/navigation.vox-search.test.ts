import { describe, it, expect } from 'vitest';
import { PARENT_CHILD_MAP, NAV_LABELS, resolveNavigation, labelForNavKey } from './navigation';

// NOTE: the plan's draft referenced `navMap`/`groupLabels`; the real exports on
// this branch are `PARENT_CHILD_MAP`/`NAV_LABELS`. Reconciled to current code.
describe('vox-search nav placement', () => {
  it('places vox-search under Knowledge', () => {
    expect(PARENT_CHILD_MAP['vox-search']).toEqual({ parent: 'knowledge', child: 'vox-search' });
  });

  it('resolves vox-search to the knowledge parent', () => {
    const nav = resolveNavigation('vox-search');
    expect(nav.parent).toBe('knowledge');
    expect(nav.child).toBe('vox-search');
  });

  it('keeps a Knowledge group label', () => {
    expect(NAV_LABELS['knowledge']).toBe('Knowledge');
  });

  it('labels the vox-search child', () => {
    expect(labelForNavKey('vox-search')).toBe('Search Index');
  });
});
