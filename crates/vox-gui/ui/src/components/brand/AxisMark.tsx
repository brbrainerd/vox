/**
 * Vox Axis (Axis) brand mark — a gimbal / gyroscope (two tilted rings + outer ring)
 * pierced by a spin-axis arrow. Monochrome: the rings + axis use `currentColor`, so the
 * mark follows the caller's `text-*` color (e.g. `text-brass` → it re-themes with the
 * accent: arcane gold / void violet / glacier cyan). The hub uses the `bg-base` token so
 * it punches through on any tile. No hardcoded color values — see AxisMark.tokens.test.ts.
 *
 * Geometry ported verbatim from crates/vox-gui/icons/source/axis.svg (the committed master).
 */
export function AxisMark({ className, title = 'Axis' }: { className?: string; title?: string }) {
  return (
    <svg
      viewBox="0 0 1024 1024"
      className={className}
      role="img"
      aria-label={title}
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
    >
      <title>{title}</title>
      {/* gimbal rings — monochrome via currentColor */}
      <g stroke="currentColor" strokeLinecap="round">
        <circle cx="512" cy="512" r="292" strokeOpacity="0.5" strokeWidth="24" />
        <ellipse cx="512" cy="512" rx="292" ry="116" strokeOpacity="0.85" strokeWidth="30" transform="rotate(34 512 512)" />
        <ellipse cx="512" cy="512" rx="292" ry="116" strokeOpacity="0.85" strokeWidth="30" transform="rotate(-34 512 512)" />
      </g>
      {/* spin axis + direction arrow */}
      <line x1="512" y1="236" x2="512" y2="872" stroke="currentColor" strokeWidth="46" strokeLinecap="round" />
      <polygon points="512,140 466,244 558,244" fill="currentColor" />
      {/* hub at the origin */}
      <circle cx="512" cy="512" r="54" className="fill-bg-base" />
      <circle cx="512" cy="512" r="54" fill="none" stroke="currentColor" strokeWidth="22" />
    </svg>
  );
}
