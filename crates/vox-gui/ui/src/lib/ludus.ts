export interface LudusProfile {
  user_id: string;
  level: number;
  xp: number;
  xp_to_next_level: number;
  xp_progress: number;
  total_xp_earned: number;
  crystals: number;
  lumens: number;
  energy: number;
  max_energy: number;
  current_streak: number;
  prestige_level: number;
  title: string;
  full_title: string;
  trust_tier: string;
}

/** Clamp a 0..1 progress fraction to a `NN%` width string. */
export function xpBarPct(progress: number): string {
  const clamped = Math.max(0, Math.min(1, progress));
  return `${Math.round(clamped * 100)}%`;
}
