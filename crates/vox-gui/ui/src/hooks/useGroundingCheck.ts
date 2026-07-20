import { useLocalStorage } from './useLocalStorage';

/** Per-session localStorage key for the opt-in grounding/hallucination check. */
export function groundingCheckKey(sessionId: string): string {
  return `gui.chat.groundingCheck.v1.${sessionId}`;
}

/**
 * Opt-in, per-session toggle for the post-reply grounding check. When
 * enabled, the backend runs a non-blocking evaluate_socrates_gate pass
 * after streaming a chat reply and may flag it as low-confidence. Defaults
 * to false (off) for every session until explicitly enabled.
 */
export function useGroundingCheck(sessionId: string) {
  return useLocalStorage<boolean>(groundingCheckKey(sessionId), false);
}
