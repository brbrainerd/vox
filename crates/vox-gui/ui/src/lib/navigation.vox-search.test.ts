import { describe, it, expect } from 'vitest';
import { PARENT_CHILD_MAP, resolveNavigation, labelForNavKey } from './navigation';

// NOTE: the plan's draft referenced `navMap`/`groupLabels`; the real export on
// this branch is `PARENT_CHILD_MAP`. `NAV_LABELS` was a parallel label map that
// drifted from the lexicon and has been deleted — group labels are asserted
// through `labelForNavKey`, the surviving public API, instead.
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
    expect(labelForNavKey('knowledge')).toBe('Knowledge');
  });

  it('labels the vox-search child', () => {
    expect(labelForNavKey('vox-search')).toBe('Search Index');
  });
});
