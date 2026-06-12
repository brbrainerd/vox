/** Design tokens — status colors and badge class maps. */

export const STATUS_BADGE_CLASS = {
  pass: 'bg-emerald-400/20 text-emerald-200 ring-1 ring-emerald-400/40',
  fail: 'bg-red-500/20 text-red-300 ring-1 ring-red-500/40',
  warn: 'bg-amber-400/20 text-amber-200 ring-1 ring-amber-400/40',
  not_run: 'bg-white/[0.05] text-zinc-400',
} as const;

export const STATUS_RAIL_BADGE_CLASS = {
  pass: 'bg-emerald-400 text-zinc-950',
  fail: 'bg-red-500 text-zinc-950',
  warn: 'bg-amber-400 text-zinc-950',
  not_run: 'bg-zinc-600 text-zinc-100',
} as const;
