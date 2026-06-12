// Pure mapper for the vox_skill_discover MCP envelope → typed rows for the
// Discovered tab. Tolerates malformed/partial records (the daemon should send
// well-formed rows, but the UI must never throw on a bad payload).

export interface DiscoveredSkill {
  id: string;
  name: string;
  description: string;
  path: string;
  installed: boolean;
}

interface RawDiscovered {
  id?: unknown;
  name?: unknown;
  description?: unknown;
  path?: unknown;
  installed?: unknown;
}

function str(v: unknown): string {
  return typeof v === 'string' ? v : '';
}

/** Map a raw vox_skill_discover result array to DiscoveredSkill rows, dropping
 *  entries with no id (unusable). Non-array input → empty list. */
export function mapDiscoveredSkills(raw: unknown): DiscoveredSkill[] {
  if (!Array.isArray(raw)) return [];
  const out: DiscoveredSkill[] = [];
  for (const r of raw as RawDiscovered[]) {
    const id = str(r?.id);
    if (!id) continue;
    out.push({
      id,
      name: str(r?.name) || id,
      description: str(r?.description),
      path: str(r?.path),
      installed: r?.installed === true,
    });
  }
  return out;
}
