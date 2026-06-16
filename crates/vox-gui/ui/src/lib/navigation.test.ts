import { describe, expect, it } from 'vitest';
import { resolveNavigation, parseViewFromLocation, breadcrumbsForView } from './navigation';

describe('resolveNavigation', () => {
  it('deep-links approvals to runs parent', () => {
    const nav = resolveNavigation('approvals');
    expect(nav.parent).toBe('runs');
    expect(nav.child).toBe('approvals');
  });

  it('maps agents parent to dashboard child', () => {
    const nav = resolveNavigation('agents');
    expect(nav.parent).toBe('agents');
    expect(nav.child).toBe('dashboard');
  });

  it('parseViewFromLocation reads hash', () => {
    expect(parseViewFromLocation({ hash: '#view=console', search: '' })).toBe('console');
    expect(parseViewFromLocation({ hash: '', search: '?view=memory' })).toBe('memory');
    expect(parseViewFromLocation({ hash: '', search: '' })).toBeNull();
  });

  it('workspace parent resolves to console default child', () => {
    const nav = resolveNavigation('workspace');
    expect(nav.parent).toBe('workspace');
    expect(nav.child).toBe('console');
  });

  it('breadcrumbsForView includes parent and child', () => {
    const crumbs = breadcrumbsForView('console');
    expect(crumbs.map(c => c.key)).toEqual(['workspace', 'console']);
  });
});
