function lum(hex: string): number {
  const h = hex.replace('#', '');
  const n = h.length === 3 ? h.split('').map(c => c + c).join('') : h;
  const [r, g, b] = [0, 2, 4].map(i => parseInt(n.slice(i, i + 2), 16) / 255)
    .map(c => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
export function contrastRatio(fg: string, bg: string): number {
  const a = lum(fg), b = lum(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}
type Pairs = Record<string, { fg: string; bg: string }>;
export const BASALT: Pairs = {
  textPrimaryOnBase: { fg: '#fafafa', bg: '#0c0e10' },
  textSecondaryOnSurface: { fg: '#c4ccd2', bg: '#15191c' },
  accentOnBase: { fg: '#c9a24a', bg: '#0c0e10' },
  accentSecondaryOnSurface: { fg: '#4a9e8f', bg: '#15191c' },
};
export const TRAVERTINE: Pairs = {
  textPrimaryOnBase: { fg: '#2a2620', bg: '#ece5d6' },
  textSecondaryOnSurface: { fg: '#4a443a', bg: '#f4eee1' },
  accentOnBase: { fg: '#8a6a26', bg: '#ece5d6' },
  accentSecondaryOnSurface: { fg: '#1f5a50', bg: '#f4eee1' },
};
