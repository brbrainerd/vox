/** Preference keys stored via `vox_preference_*` / orchestrator — single place for string literals */
export const VoxPreferenceKey = {
    budgetCapUsd: 'budget_cap_usd',
    activeModel: 'active_model',
    socratesGateEnforced: 'socrates_gate_enforced',
} as const;

export function byokPreferenceKey(provider: string): string {
    return `byok_key_${provider}`;
}
