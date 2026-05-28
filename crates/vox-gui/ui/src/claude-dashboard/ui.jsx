// Shared low-level UI bits for Vox Imperium
// Glyphs, sparklines, status pills, the glass surface helper.

const { useRef, useEffect, useState } = React;

// — Glass surface ————————————————————————————————————————————————————
function Glass({ className = "", inset = true, children, ...rest }) {
  return (
    <div
      {...rest}
      className={
        "relative rounded-2xl border border-white/[0.06] bg-white/[0.025] " +
        "backdrop-blur-2xl shadow-[0_1px_0_rgba(255,255,255,0.04)_inset,0_24px_60px_-30px_rgba(0,0,0,0.9)] " +
        className
      }
    >
      {inset && (
        <div className="pointer-events-none absolute inset-0 rounded-2xl ring-1 ring-inset ring-white/[0.04]" />
      )}
      {children}
    </div>
  );
}

// — Status pill ——————————————————————————————————————————————————————
const PHASE_TONE = {
  Verifying:   { dot: "bg-violet-400", ring: "ring-violet-400/30", text: "text-violet-300",  glow: "shadow-[0_0_18px_-4px_rgba(167,139,250,0.55)]" },
  Executing:   { dot: "bg-brass",      ring: "ring-brass/30",      text: "text-brass",       glow: "shadow-[0_0_18px_-4px_rgba(212,175,55,0.55)]" },
  Planning:    { dot: "bg-cyan-400",   ring: "ring-cyan-400/30",   text: "text-cyan-300",    glow: "shadow-[0_0_18px_-4px_rgba(34,211,238,0.55)]" },
  Paused:      { dot: "bg-zinc-500",   ring: "ring-zinc-500/30",   text: "text-zinc-300",    glow: "" },
  Validated:   { dot: "bg-emerald-400",ring: "ring-emerald-400/30",text: "text-emerald-300", glow: "" },
  Doubted:     { dot: "bg-amber-400",  ring: "ring-amber-400/30",  text: "text-amber-300",   glow: "" },
  Speculative: { dot: "bg-violet-400", ring: "ring-violet-400/30", text: "text-violet-300",  glow: "" },
  Active:      { dot: "bg-cyan-400",   ring: "ring-cyan-400/30",   text: "text-cyan-300",    glow: "" },
  Root:        { dot: "bg-white",      ring: "ring-white/30",      text: "text-white",       glow: "shadow-[0_0_22px_-2px_rgba(255,255,255,0.5)]" },
};

function Pill({ phase, label, className = "" }) {
  const t = PHASE_TONE[phase] || PHASE_TONE.Paused;
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px] font-medium tracking-wider uppercase ring-1 ${t.ring} ${t.text} bg-white/[0.02] ${className}`}>
      <span className={`relative inline-block size-1.5 rounded-full ${t.dot}`}>
        {phase !== "Paused" && (
          <span className={`absolute inset-0 rounded-full ${t.dot} animate-vox-ping opacity-60`} />
        )}
      </span>
      {label || phase}
    </span>
  );
}

// — Sparkline ——————————————————————————————————————————————————————
function Sparkline({ data, color = "currentColor", width = 84, height = 22, fill = true }) {
  if (!data || !data.length) return null;
  const min = Math.min(...data), max = Math.max(...data);
  const range = max - min || 1;
  const stepX = width / (data.length - 1);
  const pts = data.map((v, i) => [i * stepX, height - ((v - min) / range) * (height - 4) - 2]);
  const d = pts.map((p, i) => (i === 0 ? `M${p[0]},${p[1]}` : `L${p[0]},${p[1]}`)).join(" ");
  const area = `${d} L${width},${height} L0,${height} Z`;
  const gid = "g" + Math.abs(data.join("").length + data[0] * 7 | 0);
  return (
    <svg width={width} height={height} className="overflow-visible">
      <defs>
        <linearGradient id={gid} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.35" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      {fill && <path d={area} fill={`url(#${gid})`} />}
      <path d={d} fill="none" stroke={color} strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round" />
      <circle cx={pts[pts.length - 1][0]} cy={pts[pts.length - 1][1]} r="2" fill={color} />
    </svg>
  );
}

// — Tiny icon set (stroke only, premium feel) ———————————————————————————
const Icon = {
  dashboard: (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><rect x="3" y="3" width="7.5" height="9" rx="1.5"/><rect x="13.5" y="3" width="7.5" height="5" rx="1.5"/><rect x="13.5" y="11" width="7.5" height="10" rx="1.5"/><rect x="3" y="15" width="7.5" height="6" rx="1.5"/></svg>,
  flow:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><circle cx="5" cy="12" r="2.2"/><circle cx="12" cy="5" r="2.2"/><circle cx="12" cy="19" r="2.2"/><circle cx="19" cy="12" r="2.2"/><path d="M7 12h10M12 7v10M7.5 10.5l3.2-3.2M16.5 10.5l-3.2-3.2M7.5 13.5l3.2 3.2M16.5 13.5l-3.2 3.2"/></svg>,
  catalog:   (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M4 5.5C4 4.67 4.67 4 5.5 4H10v16H5.5A1.5 1.5 0 0 1 4 18.5v-13Z"/><path d="M14 4h4.5A1.5 1.5 0 0 1 20 5.5v13a1.5 1.5 0 0 1-1.5 1.5H14V4Z"/><path d="M10 8h4M10 12h4M10 16h4"/></svg>,
  matrix:    (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M12 3l8 4.6v8.8L12 21l-8-4.6V7.6L12 3Z"/><path d="M12 3v18M4 7.6l16 8.8M20 7.6L4 16.4"/></svg>,
  memory:    (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><rect x="3.5" y="6" width="17" height="12" rx="2"/><path d="M7 6v12M11 6v12M15 6v12M19 6v12"/><path d="M7 3v3M11 3v3M15 3v3M7 18v3M11 18v3M15 18v3"/></svg>,
  settings:  (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.04 1.56V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 9 19.4a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.04H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1.04-1.56V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.04 1.51 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V9c.41.16.78.42 1.07.74"/></svg>,
  search:    (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>,
  send:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="m4 12 16-8-5 18-3-7-8-3Z"/></svg>,
  bolt:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M13 3 4 14h6l-1 7 9-11h-6l1-7Z"/></svg>,
  trophy:    (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M8 4h8v4a4 4 0 1 1-8 0V4Z"/><path d="M8 6H5a3 3 0 0 0 3 3M16 6h3a3 3 0 0 1-3 3M9 14h6l-1 4h-4l-1-4Z"/><path d="M8 21h8"/></svg>,
  crystal:   (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="m6 9 6-6 6 6-6 12L6 9Z"/><path d="M6 9h12M12 3v18"/></svg>,
  flame:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M12 3s4 4 4 8a4 4 0 1 1-8 0c0-2 2-4 2-4s-1 4 2 4 0-8 0-8Z"/></svg>,
  pause:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/></svg>,
  play:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M7 5v14l12-7L7 5Z"/></svg>,
  doubt:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1 1-1 1.7V14M12 17v.01"/></svg>,
  gavel:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="m9 11 6-6 4 4-6 6-4-4ZM4 20h10M7 17l5-5"/></svg>,
  alert:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M12 3 2 20h20L12 3Z"/><path d="M12 10v5M12 18v.01"/></svg>,
  check:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="m4 12 5 5L20 6"/></svg>,
  spark:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M12 3v4M12 17v4M3 12h4M17 12h4M5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M5.6 18.4l2.8-2.8M15.6 8.4l2.8-2.8"/></svg>,
  plus:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M12 5v14M5 12h14"/></svg>,
  x:         (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="m6 6 12 12M18 6 6 18"/></svg>,
  expand:    (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/></svg>,
  command:   (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6Z"/></svg>,
  file:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9l-6-6Z"/><path d="M14 3v6h6"/></svg>,
  cpu:       (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><rect x="6" y="6" width="12" height="12" rx="1.5"/><rect x="9" y="9" width="6" height="6" rx=".5"/><path d="M10 3v3M14 3v3M10 18v3M14 18v3M3 10h3M3 14h3M18 10h3M18 14h3"/></svg>,
  scale:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M12 3v18M5 7h14M5 7l-2 6a4 4 0 0 0 8 0L9 7M19 7l-2 6a4 4 0 0 0 8 0l-2-6"/></svg>,
  chev:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="m6 9 6 6 6-6"/></svg>,
  chevR:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="m9 6 6 6-6 6"/></svg>,
  chevL:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="m15 6-6 6 6 6"/></svg>,
  more:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><circle cx="5" cy="12" r="1.4"/><circle cx="12" cy="12" r="1.4"/><circle cx="19" cy="12" r="1.4"/></svg>,
  eye:       (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/></svg>,
  mic:       (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><rect x="9" y="3" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3"/></svg>,
  stream:    (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M3 7c4 0 4 4 8 4s4-4 8-4M3 13c4 0 4 4 8 4s4-4 8-4"/></svg>,
  link:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 1 0-5.7-5.7L11 7M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 1 0 5.7 5.7L13 17"/></svg>,
  image:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m3 18 5-5 4 4 3-3 6 6"/></svg>,
  git:       (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><circle cx="6" cy="6" r="2"/><circle cx="6" cy="18" r="2"/><circle cx="18" cy="12" r="2"/><path d="M6 8v8M8 18h4a4 4 0 0 0 4-4v-2"/></svg>,
  clock:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>,
  users:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><circle cx="9" cy="9" r="3"/><path d="M3 20a6 6 0 0 1 12 0"/><circle cx="17" cy="8" r="2.5"/><path d="M15 20a5 5 0 0 1 6-4.7"/></svg>,
  back:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M3 7v6h6M3 13a9 9 0 1 0 3-7"/></svg>,
  shield:    (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6l-8-3Z"/></svg>,
  agent:     (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><circle cx="12" cy="9" r="3"/><path d="M5 20a7 7 0 0 1 14 0"/></svg>,
  dock:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/></svg>,
  pin:       (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M12 17v5M9 3h6l-1 5 3 3-8 1 1-8-1-1Z"/></svg>,
  bell:      (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M6 16V11a6 6 0 1 1 12 0v5l1 2H5l1-2ZM10 20a2 2 0 0 0 4 0"/></svg>,
  refresh:   (p) => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" {...p}><path d="M3 12a9 9 0 0 1 15-6.7L21 8M21 4v4h-4M21 12a9 9 0 0 1-15 6.7L3 16M3 20v-4h4"/></svg>,
};

// — Animated background grid + scanlines ——————————————————————————————
function ArcaneBackdrop() {
  return (
    <>
      <div className="pointer-events-none fixed inset-0 -z-10 bg-[#09090b]" />
      <div className="pointer-events-none fixed inset-0 -z-10 opacity-[0.18] [background-image:linear-gradient(to_right,rgba(255,255,255,0.06)_1px,transparent_1px),linear-gradient(to_bottom,rgba(255,255,255,0.06)_1px,transparent_1px)] [background-size:48px_48px]" />
      <div className="pointer-events-none fixed inset-0 -z-10 [background:radial-gradient(900px_500px_at_18%_-10%,rgba(212,175,55,0.10),transparent_60%),radial-gradient(900px_500px_at_82%_110%,rgba(139,92,246,0.10),transparent_60%),radial-gradient(700px_400px_at_50%_50%,rgba(34,211,238,0.05),transparent_70%)]" />
      <div className="pointer-events-none fixed inset-0 -z-10 mix-blend-overlay opacity-[0.06] [background:repeating-linear-gradient(0deg,rgba(255,255,255,0.4)_0,rgba(255,255,255,0.4)_1px,transparent_1px,transparent_3px)]" />
    </>
  );
}

Object.assign(window, { Glass, Pill, Sparkline, Icon, ArcaneBackdrop, PHASE_TONE });
