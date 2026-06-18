/**
 * Federated OmniSearch index builder — SSOT kinds from contracts/gui/omnisearch-index.v1.yaml
 */

import type { SettingEntry } from '../components/surfaces/Settings/settingsIndex';
import type { DocEntryLike, SurfaceEntryLike } from '../components/layout/paletteSources';

/** v1 client builder subset (see omnisearch-index.v1.yaml index_kinds_v1). */
export const FEDERATED_INDEX_KINDS_V1 = [
  'surface',
  'setting',
  'policy',
  'command',
  'action',
  'skill',
  'doc',
] as const;

export type FederatedIndexKind = (typeof FEDERATED_INDEX_KINDS_V1)[number];

export type FederatedPayload =
  | { type: 'surface'; viewKey: string }
  | { type: 'setting'; section: string; settingId: string }
  | { type: 'policy'; policyId: string }
  | { type: 'doc'; path: string }
  | { type: 'skill'; skillId: string }
  | { type: 'command'; command: string }
  | { type: 'action'; actionId: string };

export interface FederatedIndexEntry {
  kind: FederatedIndexKind;
  id: string;
  label: string;
  detail: string;
  score?: number;
  payload: FederatedPayload;
  /** Optional search aliases (e.g. SETTINGS_INDEX keywords). */
  keywords?: string[];
}

export interface PolicySourceRow {
  name: string;
  status?: string;
}

export interface SkillSourceRow {
  id: string;
  name: string;
  description?: string;
}

/** Minimal CLI catalog row for federated command indexing. */
export interface CommandCatalogRow {
  command: string;
  about?: string;
  path?: string[];
}

/** Minimal action manifest row for federated action indexing. */
export interface ActionManifestRow {
  action_id: string;
  title: string;
  description?: string;
}

export interface FederatedIndexSources {
  surfaces: SurfaceEntryLike[];
  settings: SettingEntry[];
  policies: PolicySourceRow[];
  docs: DocEntryLike[];
  skills: SkillSourceRow[];
  commands?: CommandCatalogRow[];
  actions?: ActionManifestRow[];
}

function scoreMatch(query: string, ...fields: string[]): number {
  const q = query.toLowerCase();
  let best = 0;
  for (const field of fields) {
    const f = field.toLowerCase();
    if (!f) continue;
    if (f === q) best = Math.max(best, 100);
    else if (f.startsWith(q)) best = Math.max(best, 80);
    else if (f.includes(q)) best = Math.max(best, 50);
  }
  return best;
}

export function buildFederatedIndex(sources: FederatedIndexSources): FederatedIndexEntry[] {
  const entries: FederatedIndexEntry[] = [];

  for (const s of sources.surfaces) {
    if (!s.viewKey || !s.navLabel) continue;
    entries.push({
      kind: 'surface',
      id: `surface:${s.viewKey}`,
      label: s.navLabel,
      detail: s.navGroup ?? '',
      payload: { type: 'surface', viewKey: s.viewKey },
    });
  }

  for (const s of sources.settings) {
    entries.push({
      kind: 'setting',
      id: `setting:${s.id}`,
      label: s.label,
      detail: s.hint,
      keywords: s.keywords,
      payload: { type: 'setting', section: s.section, settingId: s.id },
    });
  }

  for (const p of sources.policies) {
    const policyId = p.name;
    entries.push({
      kind: 'policy',
      id: `policy:${policyId}`,
      label: policyId,
      detail: p.status ?? '',
      payload: { type: 'policy', policyId },
    });
  }

  for (const d of sources.docs) {
    entries.push({
      kind: 'doc',
      id: `doc:${d.path}`,
      label: d.title,
      detail: d.description,
      payload: { type: 'doc', path: d.path },
    });
  }

  for (const sk of sources.skills) {
    entries.push({
      kind: 'skill',
      id: `skill:${sk.id}`,
      label: sk.name,
      detail: sk.description ?? '',
      payload: { type: 'skill', skillId: sk.id },
    });
  }

  for (const cmd of sources.commands ?? []) {
    if (!cmd.command) continue;
    entries.push({
      kind: 'command',
      id: `command:${cmd.command}`,
      label: cmd.command,
      detail: cmd.about ?? '',
      payload: { type: 'command', command: cmd.command },
    });
  }

  for (const act of sources.actions ?? []) {
    if (!act.action_id || !act.title) continue;
    entries.push({
      kind: 'action',
      id: `action:${act.action_id}`,
      label: act.title,
      detail: act.description ?? '',
      payload: { type: 'action', actionId: act.action_id },
    });
  }

  return entries;
}

function entryMatchesQuery(entry: FederatedIndexEntry, query: string): number {
  const idTail = entry.id.includes(':') ? entry.id.split(':').slice(1).join(':') : entry.id;
  const fields = [entry.label, entry.detail, idTail, ...(entry.keywords ?? [])];
  return scoreMatch(query, ...fields);
}

export interface SearchFederatedIndexOptions {
  kinds?: string[];
}

export function searchFederatedIndex(
  entries: FederatedIndexEntry[],
  query: string,
  options?: SearchFederatedIndexOptions,
): FederatedIndexEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];

  const kindFilter = options?.kinds?.length ? new Set(options.kinds) : null;

  const scored: FederatedIndexEntry[] = [];
  for (const entry of entries) {
    if (kindFilter && !kindFilter.has(entry.kind)) continue;
    const score = entryMatchesQuery(entry, q);
    if (score > 0) {
      scored.push({ ...entry, score });
    }
  }

  return scored.sort((a, b) => (b.score ?? 0) - (a.score ?? 0));
}
