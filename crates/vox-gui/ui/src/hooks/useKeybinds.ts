import { useEffect } from 'react';
import { chordFromEvent, matchAction, type ActionId, type Bindings } from '../lib/keybinds';
export function useKeybinds(handlers: Partial<Record<ActionId, () => void>>, bindings: Bindings) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const id = matchAction(chordFromEvent(e), bindings);
      const fn = id ? handlers[id] : undefined;
      if (fn) { e.preventDefault(); fn(); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [handlers, bindings]);
}
