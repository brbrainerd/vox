import { describe, it, expect } from 'vitest';
import { contextRefsFromPayload, attachItemsFromHits } from './loquelaContext';

describe('contextRefsFromPayload', () => {
  it('returns explicit files verbatim when present', () => {
    expect(contextRefsFromPayload({ files: ['a.rs', 'b.rs'] })).toEqual(['a.rs', 'b.rs']);
  });

  it('prefers explicit files over context chips', () => {
    const out = contextRefsFromPayload({
      files: ['explicit.rs'],
      context: [{ kind: 'file', ref: 'chip.rs' }],
    });
    expect(out).toEqual(['explicit.rs']);
  });

  it('maps file/image/url context chips to their refs', () => {
    const out = contextRefsFromPayload({
      context: [
        { kind: 'file', ref: 'src/main.rs' },
        { kind: 'image', ref: '/tmp/shot.png' },
        { kind: 'url', ref: 'https://example.com' },
      ],
    });
    expect(out).toEqual(['src/main.rs', '/tmp/shot.png', 'https://example.com']);
  });

  it('ignores non-attachable chip kinds (skill/agent/branch)', () => {
    const out = contextRefsFromPayload({
      context: [
        { kind: 'skill', ref: 'audit' },
        { kind: 'agent', ref: 'A-1' },
        { kind: 'branch', ref: 'main' },
        { kind: 'file', ref: 'keep.rs' },
      ],
    });
    expect(out).toEqual(['keep.rs']);
  });

  it('returns empty array for empty / missing input', () => {
    expect(contextRefsFromPayload({})).toEqual([]);
    expect(contextRefsFromPayload({ files: [], context: [] })).toEqual([]);
  });

  it('drops falsy refs', () => {
    const out = contextRefsFromPayload({
      context: [
        { kind: 'file', ref: '' },
        { kind: 'file', ref: 'real.rs' },
      ],
    });
    expect(out).toEqual(['real.rs']);
  });
});

describe('attachItemsFromHits', () => {
  it('maps file locators to file chips using the locator value', () => {
    const out = attachItemsFromHits([
      { locator: { kind: 'file', value: 'src/main.rs' }, path: 'ignored' },
    ]);
    expect(out).toEqual([{ kind: 'file', label: 'src/main.rs' }]);
  });

  it('maps web locators to url chips', () => {
    const out = attachItemsFromHits([{ locator: { kind: 'web', value: 'https://x.test' } }]);
    expect(out).toEqual([{ kind: 'url', label: 'https://x.test' }]);
  });

  it('drops memory / none locators (no attachable file)', () => {
    const out = attachItemsFromHits([
      { locator: { kind: 'memory', value: 'mem-42' } },
      { locator: { kind: 'none', value: 'x' } },
      { locator: { kind: 'file', value: 'keep.rs' } },
    ]);
    expect(out).toEqual([{ kind: 'file', label: 'keep.rs' }]);
  });

  it('falls back to path then source when locator value is empty', () => {
    expect(attachItemsFromHits([{ locator: { kind: 'file' }, path: 'p.rs' }]))
      .toEqual([{ kind: 'file', label: 'p.rs' }]);
    expect(attachItemsFromHits([{ locator: { kind: 'file' }, path: null, source: 's.rs' }]))
      .toEqual([{ kind: 'file', label: 's.rs' }]);
  });

  it('skips hits with no usable value', () => {
    expect(attachItemsFromHits([{ locator: { kind: 'file' } }])).toEqual([]);
  });
});
