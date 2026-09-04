/** Mode slashes handled inside Loquela (no IPC). `/plan` moved to
 *  `APP_SLASH_COMMANDS` below — it now dispatches a real turn (`vox_plan`
 *  via `Execution::Plan`) instead of just switching Loquela's mode chip. */
export const INTERNAL_MODE_SLASHES = {
  '/verify': 'verify',
  '/act': 'act',
} as const;

/** `'plan'` stays a valid mode id — the composer's mode chip and
 *  `buildChatTurn`'s `mode` field both still use it — even though `/plan`
 *  no longer resolves through `INTERNAL_MODE_SLASHES`. */
export type LoquelaModeId = 'plan' | 'act' | 'verify';

/** App-level slash commands routed through `onSlashCommand` in App.tsx. */
export const APP_SLASH_COMMANDS = [
  '/plan',
  '/memory',
  '/audit',
  '/spawn',
  '/rollback',
  '/doubt',
  '/diff',
] as const;

export type AppSlashCommand = (typeof APP_SLASH_COMMANDS)[number];

/** Normalize a composer token to the bare slash command (e.g. `/plan foo` → `/plan`). */
export function slashCommandBase(cmd: string): string {
  return cmd.trim().split(/\s+/)[0]!.toLowerCase();
}

/** Returns a Loquela mode id when `cmd` is `/plan`, `/verify`, or `/act`. */
export function resolveInternalModeSlash(cmd: string): LoquelaModeId | null {
  const base = slashCommandBase(cmd);
  const mode = INTERNAL_MODE_SLASHES[base as keyof typeof INTERNAL_MODE_SLASHES];
  return (mode as LoquelaModeId | undefined) ?? null;
}

export function isAppSlashCommand(cmd: string): boolean {
  const base = slashCommandBase(cmd);
  return (APP_SLASH_COMMANDS as readonly string[]).includes(base);
}

/** Display string for session budget next to token estimate. */
export function formatSessionBudget(spent: number, cap: number): string {
  return `session $${spent.toFixed(2)} / $${cap.toFixed(2)}`;
}
