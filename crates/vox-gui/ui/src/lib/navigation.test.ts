import { describe, expect, it } from 'vitest';
import { resolveNavigation } from './navigation';

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
});
