import type { CommandCatalogEntry } from '../types/catalog';

export interface InstalledSkill {
  id: string;
  name: string;
  description: string;
}

/** Unwrap MCP ToolResult envelopes (`{ success, data }` or bare `{ data }`). */
export function unwrapMcpToolData(result: unknown): unknown {
  if (result == null || typeof result !== 'object') return result;
  const r = result as Record<string, unknown>;
  if (r.success === true && r.data !== undefined) return r.data;
  if (r.data !== undefined) return r.data;
  return result;
}

/** Normalize `vox_skill_list` rows to slash/palette-ready skill records. */
export function parseInstalledSkills(raw: unknown): InstalledSkill[] {
  const list = Array.isArray(raw) ? raw : [];
  return list
    .map((row) => {
      const r = row as Record<string, string>;
      const name = (r.name ?? '').trim();
      const id = (r.id ?? name).trim();
      const description = (r.description ?? r.about ?? '').trim();
      if (!name || !/^[a-z0-9][a-z0-9-]*$/.test(name)) return null;
      return { id, name, description };
    })
    .filter((s): s is InstalledSkill => s !== null);
}

export function installedSkillToCatalogEntry(s: InstalledSkill): CommandCatalogEntry {
  return {
    path: ['skill', s.name],
    command: s.name,
    about: s.description,
    aliases: [],
    has_subcommands: false,
    compiled_in: true,
    source_group: 'skill',
    feature_gate: null,
    tier: 'recommended',
    capability_id: s.id,
  };
}
