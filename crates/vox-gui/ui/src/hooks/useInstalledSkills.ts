import { useEffect, useState } from 'react';
import { voxTransport } from '../transport';
import {
  type InstalledSkill,
  parseInstalledSkills,
  unwrapMcpToolData,
} from '../lib/installedSkills';

export function useInstalledSkills(enabled = true): InstalledSkill[] {
  const [skills, setSkills] = useState<InstalledSkill[]>([]);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;

    (async () => {
      try {
        const res = await voxTransport.invokeMcpTool('vox_skill_list', {});
        if (cancelled || res?.is_error) {
          if (!cancelled) setSkills([]);
          return;
        }
        const raw = unwrapMcpToolData(res?.result);
        if (!cancelled) setSkills(parseInstalledSkills(raw));
      } catch {
        if (!cancelled) setSkills([]);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [enabled]);

  return skills;
}
