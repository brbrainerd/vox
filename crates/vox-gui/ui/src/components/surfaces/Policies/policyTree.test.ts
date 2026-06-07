import { describe, it, expect } from 'vitest';
import { buildGroupTree, worstStatus, statusForRow, needsAttention, overallWorst, worstCount } from './policyTree';
import type { PolicyRow, PolicyStatus } from './types';

const rows: PolicyRow[] = [
  { id: 'code-audit/stub/todo', domain: 'code-audit-rule', title: 'TODO stub', group: 'Language rules / Stubs', severity: 'error', blocking: true, protected: true },
  { id: 'code-audit/stub/unimpl', domain: 'code-audit-rule', title: 'unimplemented', group: 'Language rules / Stubs', severity: 'error', blocking: true, protected: true },
  { id: 'ci/parity', domain: 'ci-gate', title: 'parity', group: 'CI Gates', severity: null, blocking: true, protected: false },
];

describe('buildGroupTree', () => {
  it('groups rows by group label and counts status colors per group', () => {
    const status: PolicyStatus[] = [
      { branch: 'main', id: 'code-audit/stub/todo', status: 'fail', hits: 2 },
      { branch: 'main', id: 'code-audit/stub/unimpl', status: 'pass', hits: 0 },
      { branch: 'main', id: 'ci/parity', status: 'not_run', hits: 0 },
    ];
    const tree = buildGroupTree(rows, status, ['main']);
    const stubs = tree.find(g => g.group === 'Language rules / Stubs')!;
    expect(stubs.rows.length).toBe(2);
    expect(stubs.counts.fail).toBe(1);
    expect(stubs.counts.pass).toBe(1);
    expect(stubs.worst).toBe('fail'); // group badge color
    const ci = tree.find(g => g.group === 'CI Gates')!;
    expect(ci.counts.not_run).toBe(1);
    expect(ci.worst).toBe('not_run');
  });

  it('grey not_run is the default when status is missing for a branch', () => {
    const tree = buildGroupTree(rows, [], ['main']);
    expect(tree.every(g => g.worst === 'not_run')).toBe(true);
  });

  it('multi-branch: a row is worst-of across selected branches', () => {
    const status: PolicyStatus[] = [
      { branch: 'main', id: 'ci/parity', status: 'pass', hits: 0 },
      { branch: 'feat', id: 'ci/parity', status: 'fail', hits: 1 },
    ];
    expect(statusForRow('ci/parity', status, ['main', 'feat'])).toBe('fail');
    expect(statusForRow('ci/parity', status, ['main'])).toBe('pass');
  });
});

describe('needsAttention', () => {
  it('is empty (all-clear) when nothing fails/warns', () => {
    const status: PolicyStatus[] = rows.map(r => ({ branch: 'main', id: r.id, status: 'pass' as const, hits: 0 }));
    expect(needsAttention(rows, status, ['main'])).toEqual([]);
  });
  it('collects only failing/warning rows', () => {
    const status: PolicyStatus[] = [
      { branch: 'main', id: 'code-audit/stub/todo', status: 'fail', hits: 1 },
      { branch: 'main', id: 'ci/parity', status: 'warn', hits: 1 },
    ];
    const na = needsAttention(rows, status, ['main']);
    expect(na.map(r => r.id).sort()).toEqual(['ci/parity', 'code-audit/stub/todo']);
  });
});

describe('worstStatus', () => {
  it('ranks fail > warn > pass > not_run', () => {
    expect(worstStatus(['pass', 'not_run', 'warn'])).toBe('warn');
    expect(worstStatus(['not_run'])).toBe('not_run');
    expect(worstStatus([])).toBe('not_run');
  });
});

describe('master badge (overallWorst + worstCount)', () => {
  it('overallWorst is the worst status across the whole catalog', () => {
    const status: PolicyStatus[] = [
      { branch: 'main', id: 'code-audit/stub/todo', status: 'fail', hits: 2 },
      { branch: 'main', id: 'code-audit/stub/unimpl', status: 'warn', hits: 1 },
      { branch: 'main', id: 'ci/parity', status: 'pass', hits: 0 },
    ];
    expect(overallWorst(rows, status, ['main'])).toBe('fail');
    expect(worstCount(rows, status, ['main'])).toBe(1); // one rule at the worst (fail)
  });
  it('worstCount is 0 when the worst tier is not_run (empty store → grey dot, no noisy count)', () => {
    expect(overallWorst(rows, [], ['main'])).toBe('not_run');
    expect(worstCount(rows, [], ['main'])).toBe(0);
  });
  it('worstCount still counts non-not_run worst tiers', () => {
    const status: PolicyStatus[] = [
      { branch: 'main', id: 'code-audit/stub/todo', status: 'warn', hits: 1 },
      { branch: 'main', id: 'code-audit/stub/unimpl', status: 'warn', hits: 1 },
      { branch: 'main', id: 'ci/parity', status: 'not_run', hits: 0 },
    ];
    expect(overallWorst(rows, status, ['main'])).toBe('warn');
    expect(worstCount(rows, status, ['main'])).toBe(2); // two warns; not_run not counted
  });
});
