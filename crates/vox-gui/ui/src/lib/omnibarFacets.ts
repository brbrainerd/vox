/**
 * Pure faceting for the Omnibar: merge the five sources into capped,
 * provenance-labeled facets. No transport, no DOM. Facets fail independently —
 * a graph-source error is carried on the facet, never propagated to the others.
 *
 * See docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md §3.3, §6.
 */
import type { FederatedIndexEntry } from './federatedSearchIndex';
import type { UnifiedHit } from '../components/surfaces/Search/searchHelpers';
import type { ContentManifestEntry } from '../hooks/useContentManifest';
import type { SearchableHit } from './searchableRegistry';

export const FACET_CAP = 6;

export type FacetKey = 'surfaces' | 'commands' | 'onScreen' | 'graph' | 'docs';
export type Provenance = 'manifest' | 'corpus' | 'runtime' | 'graph' | 'docs';

export interface GraphNeighbor {
  id: string;
  label: string;
  viewKey?: string;
}

export interface OmnibarGraphResult {
  rows: GraphNeighbor[];
  error: string | null;
}

export interface OmnibarSources {
  query: string;
  federated: FederatedIndexEntry[];
  backendHits: UnifiedHit[];
  manifest: ContentManifestEntry[];
  runtimeHits: SearchableHit[];
  graph: OmnibarGraphResult;
}

export type OmnibarActivation =
  | { type: 'navigate'; viewKey: string; anchorId?: string }
  | { type: 'command'; command: string }
  | { type: 'doc'; path: string }
  | { type: 'graph'; node: GraphNeighbor }
  // finding #5: carry CommandPalette's agents/skills/settings/policies arms so
  // the consolidation does not silently lose them. O4 wires the routing.
  | { type: 'agent'; agentId: string }
  | { type: 'skill'; skillId: string }
  | { type: 'setting'; section: string; settingId: string }
  | { type: 'policy'; policyId: string };

export interface OmnibarRow {
  id: string;
  facet: FacetKey;
  label: string;
  detail: string;
  provenance: Provenance;
  activate: OmnibarActivation;
}

export interface OmnibarFacet {
  key: FacetKey;
  label: string;
  provenanceHint: string;
  rows: OmnibarRow[];
  error: string | null;
}

const FACET_LABELS: Record<FacetKey, string> = {
  surfaces: 'Surfaces',
  commands: 'Commands',
  onScreen: 'On Screen',
  graph: 'Graph',
  docs: 'Docs',
};

const FACET_PROVENANCE_HINT: Record<FacetKey, string> = {
  surfaces: 'manifest',
  commands: 'corpus',
  onScreen: 'runtime + manifest',
  graph: 'vox-graph',
  docs: 'docs',
};

function matches(query: string, ...fields: string[]): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return false;
  return fields.some((f) => f.toLowerCase().includes(q));
}

export function buildOmnibarFacets(src: OmnibarSources): OmnibarFacet[] {
  const cap = <T>(rows: T[]) => rows.slice(0, FACET_CAP);

  // SURFACES — federated surface entries + manifest labels (manifest provenance).
  const surfaceRows: OmnibarRow[] = [];
  for (const e of src.federated) {
    if (e.kind !== 'surface' || e.payload.type !== 'surface') continue;
    surfaceRows.push({
      id: e.id,
      facet: 'surfaces',
      label: e.label,
      detail: e.detail,
      provenance: 'manifest',
      activate: { type: 'navigate', viewKey: e.payload.viewKey },
    });
  }
  for (const m of src.manifest) {
    if (!matches(src.query, m.label, ...m.headings)) continue;
    if (surfaceRows.some((r) => r.activate.type === 'navigate' && r.activate.viewKey === m.viewKey)) {
      continue;
    }
    surfaceRows.push({
      id: `manifest-surface:${m.viewKey}`,
      facet: 'surfaces',
      label: m.label,
      detail: m.route,
      provenance: 'manifest',
      activate: { type: 'navigate', viewKey: m.viewKey },
    });
  }

  // COMMANDS — backend command hits + federated command/action + the carried-in
  // settings/policies/skills arms (finding #5: consolidate, never silently lose).
  const commandRows: OmnibarRow[] = [];
  for (const h of src.backendHits) {
    if (h.kind !== 'command') continue;
    commandRows.push({
      id: `cmd:${h.path}`,
      facet: 'commands',
      label: h.title ?? h.path ?? '',
      detail: h.snippet,
      provenance: 'corpus',
      activate: { type: 'command', command: String(h.locator.value ?? h.path) },
    });
  }
  for (const e of src.federated) {
    if (e.kind === 'command' && e.payload.type === 'command') {
      commandRows.push({
        id: e.id,
        facet: 'commands',
        label: e.label,
        detail: e.detail,
        provenance: 'corpus',
        activate: { type: 'command', command: e.payload.command },
      });
    } else if (e.kind === 'action' && e.payload.type === 'action') {
      commandRows.push({
        id: e.id,
        facet: 'commands',
        label: e.label,
        detail: e.detail,
        provenance: 'corpus',
        activate: { type: 'command', command: e.payload.actionId },
      });
    } else if (e.kind === 'setting' && e.payload.type === 'setting') {
      commandRows.push({
        id: e.id,
        facet: 'commands',
        label: e.label,
        detail: e.detail,
        provenance: 'corpus',
        activate: { type: 'setting', section: e.payload.section, settingId: e.payload.settingId },
      });
    } else if (e.kind === 'policy' && e.payload.type === 'policy') {
      commandRows.push({
        id: e.id,
        facet: 'commands',
        label: e.label,
        detail: e.detail,
        provenance: 'corpus',
        activate: { type: 'policy', policyId: e.payload.policyId },
      });
    } else if (e.kind === 'skill' && e.payload.type === 'skill') {
      commandRows.push({
        id: e.id,
        facet: 'commands',
        label: e.label,
        detail: e.detail,
        provenance: 'corpus',
        activate: { type: 'skill', skillId: e.payload.skillId },
      });
    }
  }

  // ON SCREEN — runtime registry hits + manifest copy/headings.
  const onScreenRows: OmnibarRow[] = [];
  for (const r of src.runtimeHits) {
    onScreenRows.push({
      id: `runtime:${r.surfaceId}:${r.label}`,
      facet: 'onScreen',
      label: r.label,
      detail: r.detail ?? '',
      provenance: 'runtime',
      activate: { type: 'navigate', viewKey: r.viewKey, anchorId: r.anchorId },
    });
  }
  for (const m of src.manifest) {
    const text = [...m.copy, ...m.headings].find((t) => matches(src.query, t));
    if (!text) continue;
    onScreenRows.push({
      id: `manifest-copy:${m.viewKey}:${text}`,
      facet: 'onScreen',
      label: text,
      detail: m.label,
      provenance: 'manifest',
      activate: { type: 'navigate', viewKey: m.viewKey },
    });
  }

  // GRAPH — graph-discover neighbors; error carried, never propagated.
  const graphRows: OmnibarRow[] = src.graph.error
    ? []
    : src.graph.rows.map((n) => ({
        id: `graph:${n.id}`,
        facet: 'graph' as const,
        label: n.label,
        detail: 'relates to',
        provenance: 'graph' as const,
        activate: { type: 'graph' as const, node: n },
      }));

  // DOCS — federated doc entries.
  const docRows: OmnibarRow[] = [];
  for (const e of src.federated) {
    if (e.kind !== 'doc' || e.payload.type !== 'doc') continue;
    docRows.push({
      id: e.id,
      facet: 'docs',
      label: e.label,
      detail: e.detail,
      provenance: 'docs',
      activate: { type: 'doc', path: e.payload.path },
    });
  }

  return [
    { key: 'surfaces', label: FACET_LABELS.surfaces, provenanceHint: FACET_PROVENANCE_HINT.surfaces, rows: cap(surfaceRows), error: null },
    { key: 'commands', label: FACET_LABELS.commands, provenanceHint: FACET_PROVENANCE_HINT.commands, rows: cap(commandRows), error: null },
    { key: 'onScreen', label: FACET_LABELS.onScreen, provenanceHint: FACET_PROVENANCE_HINT.onScreen, rows: cap(onScreenRows), error: null },
    { key: 'graph', label: FACET_LABELS.graph, provenanceHint: FACET_PROVENANCE_HINT.graph, rows: cap(graphRows), error: src.graph.error },
    { key: 'docs', label: FACET_LABELS.docs, provenanceHint: FACET_PROVENANCE_HINT.docs, rows: cap(docRows), error: null },
  ];
}

/** Flattened, facet-ordered rows (Surfaces → Commands → On-Screen → Graph → Docs). */
export function omnibarRowsInOrder(facets: OmnibarFacet[]): OmnibarRow[] {
  return facets.flatMap((f) => f.rows);
}

/** The top hit Enter activates: first row in facet order. */
export function omnibarTopHit(facets: OmnibarFacet[]): OmnibarRow | null {
  return omnibarRowsInOrder(facets)[0] ?? null;
}
