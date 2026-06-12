import { SettingEntry } from '../surfaces/Settings/settingsIndex';

export interface SurfaceEntryLike {
  viewKey: string | null;
  navLabel: string | null;
  navGroup: string | null;
  navIcon: string | null;
  cliGroup: string | null;
  tier: string;
}

export interface DocEntryLike {
  title: string;
  description: string;
  path: string;
}

export type PaletteItem =
  | { kind: 'surface'; label: string; detail: string; viewKey: string }
  | { kind: 'setting'; label: string; detail: string; targetSection: string }
  | { kind: 'doc'; label: string; detail: string; path: string };

export interface PaletteSources {
  surfaces: SurfaceEntryLike[];
  settings: SettingEntry[];
  docs: DocEntryLike[];
}

const MAX_PER_KIND = 5;

export function buildPaletteItems(query: string, sources: PaletteSources): PaletteItem[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const items: PaletteItem[] = [];

  for (const s of sources.surfaces) {
    if (!s.viewKey || !s.navLabel) continue;
    if (s.navLabel.toLowerCase().includes(q) || (s.navGroup ?? '').toLowerCase().includes(q)) {
      items.push({ kind: 'surface', label: s.navLabel, detail: s.navGroup ?? '', viewKey: s.viewKey });
    }
  }

  let settingCount = 0;
  for (const s of sources.settings) {
    if (settingCount >= MAX_PER_KIND) break;
    if (
      s.label.toLowerCase().includes(q) ||
      s.hint.toLowerCase().includes(q) ||
      s.keywords.some(k => k.includes(q))
    ) {
      items.push({ kind: 'setting', label: s.label, detail: s.hint, targetSection: s.section });
      settingCount += 1;
    }
  }

  let docCount = 0;
  for (const d of sources.docs) {
    if (docCount >= MAX_PER_KIND) break;
    if (d.title.toLowerCase().includes(q) || d.description.toLowerCase().includes(q)) {
      items.push({ kind: 'doc', label: d.title, detail: d.description, path: d.path });
      docCount += 1;
    }
  }

  return items;
}
