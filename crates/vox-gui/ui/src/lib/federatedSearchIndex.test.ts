import { describe, it, expect } from 'vitest';
import {
  buildFederatedIndex,
  searchFederatedIndex,
  type FederatedIndexSources,
} from './federatedSearchIndex';
import type { SettingEntry } from '../components/surfaces/Settings/settingsIndex';

const surfaces: FederatedIndexSources['surfaces'] = [
  {
    viewKey: 'tasks',
    cliGroup: null,
    tier: 'live_backend',
    navLabel: 'Tasks',
    navIcon: 'list',
    navGroup: 'operate',
  },
  { viewKey: null, cliGroup: 'mens', tier: 'none', navLabel: null, navIcon: null, navGroup: null },
];

const settings: SettingEntry[] = [
  {
    id: 'llm-max-concurrency',
    section: 'llm',
    label: 'Max parallel LLM requests',
    hint: 'Global ceiling',
    keywords: ['openrouter'],
  },
];

function baseSources(overrides: Partial<FederatedIndexSources> = {}): FederatedIndexSources {
  return {
    surfaces,
    settings,
    policies: [],
    docs: [],
    skills: [],
    commands: [],
    actions: [],
    ...overrides,
  };
}

describe('buildFederatedIndex', () => {
  it('includes surface entries from SURFACE_REGISTRY-like input', () => {
    const entries = buildFederatedIndex(baseSources());
    const surfaceEntries = entries.filter(e => e.kind === 'surface');
    expect(surfaceEntries).toHaveLength(1);
    expect(surfaceEntries[0]).toMatchObject({
      kind: 'surface',
      id: 'surface:tasks',
      label: 'Tasks',
      detail: 'operate',
      payload: { type: 'surface', viewKey: 'tasks' },
    });
  });

  it('includes command entries from catalog rows using command as label', () => {
    const entries = buildFederatedIndex(
      baseSources({
        commands: [{ command: 'fmt', about: 'Format Rust sources' }],
      }),
    );
    const commandEntry = entries.find(e => e.kind === 'command');
    expect(commandEntry).toMatchObject({
      kind: 'command',
      id: 'command:fmt',
      label: 'fmt',
      detail: 'Format Rust sources',
      payload: { type: 'command', command: 'fmt' },
    });
  });

  it('includes action entries from action manifest using action_id and title', () => {
    const entries = buildFederatedIndex(
      baseSources({
        actions: [
          {
            action_id: 'ci.pre-push',
            title: 'Run pre-push CI',
            description: 'Fast local CI gate',
          },
        ],
      }),
    );
    const actionEntry = entries.find(e => e.kind === 'action');
    expect(actionEntry).toMatchObject({
      kind: 'action',
      id: 'action:ci.pre-push',
      label: 'Run pre-push CI',
      detail: 'Fast local CI gate',
      payload: { type: 'action', actionId: 'ci.pre-push' },
    });
  });
});

describe('searchFederatedIndex', () => {
  it('matches policy entry when name contains fmt and query is fmt.rust', () => {
    const entries = buildFederatedIndex(
      baseSources({
        policies: [{ name: 'fmt.rust', status: 'pass' }],
      }),
    );
    const hits = searchFederatedIndex(entries, 'fmt.rust');
    const policyHit = hits.find(h => h.kind === 'policy');
    expect(policyHit).toBeDefined();
    expect(policyHit).toMatchObject({
      kind: 'policy',
      id: 'policy:fmt.rust',
      label: 'fmt.rust',
      payload: { type: 'policy', policyId: 'fmt.rust' },
    });
  });

  it('matches setting entries by keyword from SETTINGS_INDEX-like input', () => {
    const entries = buildFederatedIndex(baseSources());
    const hits = searchFederatedIndex(entries, 'openrouter');
    const settingHit = hits.find(h => h.kind === 'setting');
    expect(settingHit).toBeDefined();
    expect(settingHit).toMatchObject({
      kind: 'setting',
      id: 'setting:llm-max-concurrency',
      payload: { type: 'setting', section: 'llm', settingId: 'llm-max-concurrency' },
    });
  });

  it('filters by kinds when options.kinds is provided', () => {
    const entries = buildFederatedIndex(
      baseSources({
        policies: [{ name: 'fmt.rust', status: 'pass' }],
      }),
    );
    const hits = searchFederatedIndex(entries, 'fmt', { kinds: ['policy'] });
    expect(hits.every(h => h.kind === 'policy')).toBe(true);
    expect(hits.some(h => h.kind === 'surface')).toBe(false);
  });

  it('returns no hits for empty query', () => {
    const entries = buildFederatedIndex(baseSources());
    expect(searchFederatedIndex(entries, '')).toHaveLength(0);
    expect(searchFederatedIndex(entries, '   ')).toHaveLength(0);
  });

  it('finds fmt in command kind when catalog has fmt command', () => {
    const entries = buildFederatedIndex(
      baseSources({
        commands: [{ command: 'fmt', about: 'Format Rust sources' }],
        policies: [{ name: 'fmt.rust', status: 'pass' }],
      }),
    );
    const hits = searchFederatedIndex(entries, 'fmt', { kinds: ['command'] });
    expect(hits.some(h => h.kind === 'command' && h.label === 'fmt')).toBe(true);
    expect(hits.some(h => h.kind === 'policy')).toBe(false);
  });
});
