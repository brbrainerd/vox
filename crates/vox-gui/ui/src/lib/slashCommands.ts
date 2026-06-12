// Slash-command entries for the Loquela chat composer.
//
// Builtin verbs are stable and always present; installed skills are appended
// as `/skill-name` so the same `/` menu surfaces both. This keeps the GUI
// chat in lockstep with the SSOT skill registry (vox_skill_list) — adding a
// skill to any discovery root makes it a slash command with no UI change.

export interface SlashEntry {
  cmd: string;
  desc: string;
  icon: string;
  kind: 'builtin' | 'skill';
  skillId?: string;
}

// The 8 verbs previously hardcoded in Loquela.tsx (LQ_SLASH), moved here so
// they can be unit-tested and merged with dynamic skill entries.
export const BUILTIN_SLASH: SlashEntry[] = [
  { cmd: '/plan',     desc: 'Draft a multi-step plan without executing',   icon: 'flow',   kind: 'builtin' },
  { cmd: '/spawn',    desc: 'Spin up a sub-agent on this branch',          icon: 'agent',  kind: 'builtin' },
  { cmd: '/audit',    desc: 'Socrates citation + invariant audit on file', icon: 'shield', kind: 'builtin' },
  { cmd: '/verify',   desc: 'Run rule-pack + property tests',              icon: 'check',  kind: 'builtin' },
  { cmd: '/doubt',    desc: 'Inject doubt at threshold N',                 icon: 'alert',  kind: 'builtin' },
  { cmd: '/memory',   desc: 'Query Mnemosyne (RAG over project memory)',   icon: 'memory', kind: 'builtin' },
  { cmd: '/rollback', desc: 'Revert to last durable checkpoint',           icon: 'back',   kind: 'builtin' },
  { cmd: '/diff',     desc: 'Show pending diff staged by agent',           icon: 'file',   kind: 'builtin' },
];

interface SkillRecord {
  // Accepts both the skill-registry shape ({id,name,description}) and the
  // command-catalog shape ({command,capability_id,about}) that the GUI
  // currently feeds Loquela — whichever fields are present win.
  id?: string;
  name?: string;
  description?: string;
  command?: string;
  capability_id?: string;
  about?: string;
}

/**
 * Builtins first (stable order), then installed skills as `/skill-name`,
 * deduped by command (a skill whose name collides with a builtin verb is
 * dropped — the builtin wins). Malformed skill records are skipped.
 */
export function buildSlashEntries(skills: (SkillRecord | null | undefined)[]): SlashEntry[] {
  const out = [...BUILTIN_SLASH];
  const taken = new Set(out.map((e) => e.cmd));
  for (const s of skills ?? []) {
    const name = (s?.name ?? s?.command)?.trim();
    // Only spec-shaped names (agentskills.io: ^[a-z0-9][a-z0-9-]*$) make typeable
    // slash commands. Display names with spaces/dots are skipped — they can't be
    // matched in the composer anyway.
    if (!name || !/^[a-z0-9][a-z0-9-]*$/.test(name)) continue;
    const cmd = `/${name}`;
    if (taken.has(cmd)) continue;
    taken.add(cmd);
    out.push({
      cmd,
      desc: s?.description ?? s?.about ?? '',
      icon: 'bolt',
      kind: 'skill',
      skillId: s?.id ?? s?.capability_id ?? name,
    });
  }
  return out;
}
