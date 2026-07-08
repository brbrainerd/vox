import { describe, it, expect } from 'vitest';
import { buildOmnibarFacets, FACET_CAP, queryForFederatedSearch, shouldIncludeDocsInOmnibar, type OmnibarSources } from './omnibarFacets';
import { searchFederatedIndex } from './federatedSearchIndex';
import type { FederatedIndexEntry } from './federatedSearchIndex';
import type { UnifiedHit } from '../components/surfaces/Search/searchHelpers';
import type { ContentManifestEntry } from '../hooks/useContentManifest';
import type { SearchableHit } from './searchableRegistry';

const surface = (vk: string, label: string): FederatedIndexEntry => ({
  kind: 'surface',
  id: `surface:${vk}`,
  label,
  detail: 'Runs',
  payload: { type: 'surface', viewKey: vk },
});

const docEntry = (path: string, title: string): FederatedIndexEntry => ({
  kind: 'doc',
  id: `doc:${path}`,
  label: title,
  detail: '',
  payload: { type: 'doc', path },
});

const cmdHit = (cmd: string): UnifiedHit => ({
  source: 'commands',
  kind: 'command',
  path: cmd,
  title: cmd,
  snippet: '',
  score: 0.9,
  provenance: ['commands:catalog'],
  locator: { kind: 'command', value: cmd },
});

const manifestRow = (vk: string): ContentManifestEntry => ({
  viewKey: vk,
  label: 'Activity',
  route: `#view=${vk}`,
  headings: ['3 pending approvals'],
  copy: ['3 pending approvals'],
  commands: [],
  docs: [],
});

const regHit = (label: string): SearchableHit => ({
  surfaceId: 'activity',
  label,
  detail: 'Activity',
  viewKey: 'activity',
});

function sources(partial: Partial<OmnibarSources>): OmnibarSources {
  return {
    query: 'pending',
    federated: [],
    backendHits: [],
    manifest: [],
    runtimeHits: [],
    graph: { rows: [], error: null },
    ...partial,
  };
}

describe('buildOmnibarFacets', () => {
  it('buckets sources into the five facets with provenance labels', () => {
    const facets = buildOmnibarFacets(
      sources({
        query: 'help pending',
        federated: [surface('approvals', 'Approvals'), docEntry('a.md', 'Approvals workflow')],
        backendHits: [cmdHit('vox_resolve_approval')],
        manifest: [manifestRow('activity')],
        runtimeHits: [regHit('3 pending approvals (live)')],
        graph: {
          rows: [{ id: 'n1', label: 'Chat rail', viewKey: 'chat' }],
          error: null,
        },
      }),
    );
    const byKey = Object.fromEntries(facets.map((f) => [f.key, f]));
    expect(byKey.surfaces.rows[0].provenance).toBe('manifest'); // surface-kind → manifest-class label
    expect(byKey.commands.rows[0].provenance).toBe('corpus');
    expect(byKey.onScreen.rows.some((r) => r.provenance === 'runtime')).toBe(true);
    expect(byKey.graph.rows[0].provenance).toBe('graph');
    expect(byKey.docs.rows[0].provenance).toBe('docs');
    expect(facets.map((f) => f.key)).toEqual([
      'surfaces',
      'commands',
      'onScreen',
      'graph',
      'docs',
    ]);
  });

  it('caps each facet at FACET_CAP', () => {
    const many = Array.from({ length: FACET_CAP + 5 }, (_, i) => cmdHit(`cmd_${i}`));
    const facets = buildOmnibarFacets(sources({ backendHits: many }));
    const commands = facets.find((f) => f.key === 'commands')!;
    expect(commands.rows).toHaveLength(FACET_CAP);
  });

  it('graph facet failure is isolated — other facets still populate', () => {
    const facets = buildOmnibarFacets(
      sources({
        federated: [surface('approvals', 'Approvals')],
        graph: { rows: [], error: 'graph facet pending VG-1' },
      }),
    );
    const graph = facets.find((f) => f.key === 'graph')!;
    const surfaces = facets.find((f) => f.key === 'surfaces')!;
    expect(graph.error).toBe('graph facet pending VG-1');
    expect(graph.rows).toHaveLength(0);
    expect(surfaces.rows.length).toBeGreaterThan(0); // not blanked
  });

  it('topHit returns the highest-priority row across non-empty facets', () => {
    const facets = buildOmnibarFacets(
      sources({ federated: [surface('approvals', 'Approvals')] }),
    );
    const order = facets.flatMap((f) => f.rows);
    expect(order[0].facet).toBe('surfaces');
  });

  it('hides doc facet unless the query is help-oriented', () => {
    const hidden = buildOmnibarFacets(
      sources({ query: 'pending', federated: [docEntry('a.md', 'Guide')] }),
    );
    expect(hidden.find((f) => f.key === 'docs')!.rows).toHaveLength(0);

    const shown = buildOmnibarFacets(
      sources({ query: 'help cli', federated: [docEntry('a.md', 'CLI guide')] }),
    );
    expect(shown.find((f) => f.key === 'docs')!.rows).toHaveLength(1);
  });

  it('queryForFederatedSearch strips help prefix for doc matching', () => {
    expect(queryForFederatedSearch('help cli')).toBe('cli');
    const entries = [docEntry('docs/src/reference/cli.md', 'CLI Reference')];
    const hits = searchFederatedIndex(entries, queryForFederatedSearch('help cli'), { kinds: ['doc'] });
    expect(hits).toHaveLength(1);
  });

  it('shows docs facet for skills prefix mode', () => {
    const shown = buildOmnibarFacets(
      sources({
        query: 'cli',
        prefixMode: 'skills',
        federated: [docEntry('a.md', 'CLI guide')],
      }),
    );
    expect(shown.find((f) => f.key === 'docs')!.rows).toHaveLength(1);
    expect(shouldIncludeDocsInOmnibar('cli', { prefixMode: 'skills' })).toBe(true);
  });
});
