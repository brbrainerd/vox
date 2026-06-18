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

export type StatusToneKind =
  | 'pass'
  | 'fail'
  | 'warn'
  | 'info'
  | 'neutral'
  | 'accent'
  | 'Executing'
  | 'Verifying'
  | 'Planning'
  | 'Paused'
  | 'Validated'
  | 'Doubted'
  | 'Speculative'
  | 'Active'
  | 'Root';

export const STATUS_TONE = {
  pass:   { dot: 'bg-emerald-400',  ring: 'ring-emerald-400/30',  text: 'text-emerald-300',  soft: 'bg-emerald-400/10',  solid: 'bg-emerald-400',  onSolid: 'text-zinc-950' },
  fail:   { dot: 'bg-red-500',      ring: 'ring-red-500/30',      text: 'text-red-300',      soft: 'bg-red-500/10',      solid: 'bg-red-500',      onSolid: 'text-zinc-950' },
  warn:   { dot: 'bg-amber-400',    ring: 'ring-amber-400/30',    text: 'text-amber-300',    soft: 'bg-amber-400/10',    solid: 'bg-amber-400',    onSolid: 'text-zinc-950' },
  info:   { dot: 'bg-sky-400',      ring: 'ring-sky-400/30',      text: 'text-sky-300',      soft: 'bg-sky-400/10',      solid: 'bg-sky-400',      onSolid: 'text-zinc-950' },
  neutral:{ dot: 'bg-zinc-500',     ring: 'ring-zinc-500/30',     text: 'text-zinc-300',     soft: 'bg-white/[0.04]',    solid: 'bg-zinc-500',     onSolid: 'text-zinc-100' },
  accent: { dot: 'bg-brass',        ring: 'ring-brass/30',        text: 'text-brass',        soft: 'bg-brass/10',        solid: 'bg-brass',        onSolid: 'text-zinc-950' },
  Executing:   { dot: 'bg-brass',     ring: 'ring-brass/30',       text: 'text-brass',       soft: 'bg-brass/10',       solid: 'bg-brass',       onSolid: 'text-zinc-950' },
  Verifying:   { dot: 'bg-violet-400',ring: 'ring-violet-400/30', text: 'text-violet-300',  soft: 'bg-violet-400/10',  solid: 'bg-violet-400',  onSolid: 'text-zinc-950' },
  Planning:    { dot: 'bg-cyan-400',  ring: 'ring-cyan-400/30',    text: 'text-cyan-300',    soft: 'bg-cyan-400/10',    solid: 'bg-cyan-400',    onSolid: 'text-zinc-950' },
  Paused:      { dot: 'bg-zinc-500',  ring: 'ring-zinc-500/30',    text: 'text-zinc-300',    soft: 'bg-white/[0.04]',   solid: 'bg-zinc-500',    onSolid: 'text-zinc-100' },
  Validated:   { dot: 'bg-emerald-400',ring:'ring-emerald-400/30', text: 'text-emerald-300', soft: 'bg-emerald-400/10', solid: 'bg-emerald-400', onSolid: 'text-zinc-950' },
  Doubted:     { dot: 'bg-amber-400', ring: 'ring-amber-400/30',   text: 'text-amber-300',   soft: 'bg-amber-400/10',   solid: 'bg-amber-400',   onSolid: 'text-zinc-950' },
  Speculative: { dot: 'bg-violet-400',ring: 'ring-violet-400/30',  text: 'text-violet-300',  soft: 'bg-violet-400/10',  solid: 'bg-violet-400',  onSolid: 'text-zinc-950' },
  Active:      { dot: 'bg-cyan-400',  ring: 'ring-cyan-400/30',    text: 'text-cyan-300',    soft: 'bg-cyan-400/10',    solid: 'bg-cyan-400',    onSolid: 'text-zinc-950' },
  Root:        { dot: 'bg-white',     ring: 'ring-white/30',       text: 'text-white',       soft: 'bg-white/[0.06]',   solid: 'bg-white',       onSolid: 'text-zinc-950' },
} as const;

