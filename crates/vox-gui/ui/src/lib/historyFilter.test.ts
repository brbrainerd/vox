import { describe, it, expect } from 'vitest';
import { filterEntries } from './historyFilter';

describe('filterEntries', () => {
  it('ranks subsequence matches, drops non-matches', () => {
    const e = [
      { id: 1, text: 'cargo test' },
      { id: 2, text: 'git commit' },
      { id: 3, text: 'cargo build' },
    ];
    const out = filterEntries('crgo', e as any);
    expect(out.map(x => x.id)).toEqual([1, 3]);
  });

  it('empty query returns all in original order', () => {
    const e = [
      { id: 1, text: 'a' },
      { id: 2, text: 'b' },
    ];
    expect(filterEntries('', e as any).map(x => x.id)).toEqual([1, 2]);
  });

  it('ranks matches that start earlier higher', () => {
    const e = [
      { id: 1, text: 'barfoo' },
      { id: 2, text: 'foobar' },
    ];
    const out = filterEntries('foo', e as any);
    expect(out.map(x => x.id)).toEqual([2, 1]); // 'foobar' starts at 0, 'barfoo' at 3
  });
});
