import { useLocalStorage } from './useLocalStorage';

export type ChatVerbosity = 'quiet' | 'normal' | 'verbose';

export const CHAT_VERBOSITY_KEY = 'gui.chat.verbosity.v1';

/**
 * Global chat-feed verbosity: quiet (status line only), normal (adds a
 * one-line done-in/cost summary per turn), verbose (adds collapsed
 * per-phase breadcrumbs, still without leaving the chat tab). Full detail
 * is always available in the Flow panel regardless of this setting.
 */
export function useChatVerbosity() {
  return useLocalStorage<ChatVerbosity>(CHAT_VERBOSITY_KEY, 'normal');
}
