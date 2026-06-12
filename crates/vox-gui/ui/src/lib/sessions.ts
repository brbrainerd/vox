export interface ChatSession {
  id: string; // session_id sent to the orchestrator, prefix 'gui-'
  title: string;
  createdAt: number;
  /** Paths auto-attached to every submission's file affinity (working set). */
  scopePaths: string[];
}

let counter = 0;

export function createSession(
  existing: ChatSession[],
  opts?: { scopePaths?: string[] },
): ChatSession {
  counter += 1;
  const id = `gui-${Date.now().toString(36)}-${counter.toString(36)}`;
  const n =
    existing.reduce((max, s) => {
      const m = /^Chat (\d+)$/.exec(s.title);
      return m ? Math.max(max, Number(m[1])) : max;
    }, 0) + 1;
  return { id, title: `Chat ${n}`, createdAt: Date.now(), scopePaths: opts?.scopePaths ?? [] };
}

export function closeSession(
  sessions: ChatSession[],
  id: string,
): { sessions: ChatSession[]; nextActiveId: string } {
  if (sessions.length <= 1) {
    return { sessions, nextActiveId: sessions[0]?.id ?? '' };
  }
  const idx = sessions.findIndex(s => s.id === id);
  if (idx === -1) return { sessions, nextActiveId: sessions[0].id };
  const remaining = sessions.filter(s => s.id !== id);
  const neighbor = remaining[Math.max(0, idx - 1)] ?? remaining[0];
  return { sessions: remaining, nextActiveId: neighbor.id };
}

export function renameSession(
  sessions: ChatSession[],
  id: string,
  title: string,
): ChatSession[] {
  const t = title.trim();
  if (!t) return sessions;
  return sessions.map(s => (s.id === id ? { ...s, title: t } : s));
}
