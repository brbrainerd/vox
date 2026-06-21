export function moodFromPhase(phase: string): string {
  switch (phase) {
    case 'Executing':
    case 'Verifying':
      return 'Excited';
    case 'Planning':
    case 'Active':
      return 'Neutral';
    case 'Paused':
      return 'Tired';
    case 'Validated':
      return 'Happy';
    case 'Doubted':
      return 'Sad';
    default:
      return 'Neutral';
  }
}

export function integrityFromDiag(diag: { errors: number; warnings: number }): 'intact' | 'cracked' | 'ruined' {
  if (diag.errors > 0) {
    return 'cracked';
  }
  return 'intact';
}
