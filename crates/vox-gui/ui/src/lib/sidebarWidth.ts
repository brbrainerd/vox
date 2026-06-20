// Continuous sidebar width with snap-to-preset for tidy default layouts.
export const SIDEBAR_MIN = 64;   // == rail
export const SIDEBAR_MAX = 420;
export const SIDEBAR_PRESETS = [64, 212, 280]; // rail / default / wide
const SNAP_TOLERANCE = 12;

export function clampSidebarWidth(px: number): number {
  if (Number.isNaN(px)) return 212;
  return Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, Math.round(px)));
}

export function snapToPreset(px: number): number {
  const w = clampSidebarWidth(px);
  for (const preset of SIDEBAR_PRESETS) {
    if (Math.abs(w - preset) <= SNAP_TOLERANCE) return preset;
  }
  return w;
}
