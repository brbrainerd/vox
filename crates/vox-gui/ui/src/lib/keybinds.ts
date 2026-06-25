export type ActionId =
  | 'open-palette' | 'toggle-sidebar' | 'toggle-hud' | 'dispatch-intent'
  | 'pause-resume-agent';
export interface ActionDef { id: ActionId; label: string }
export const ACTION_REGISTRY: ActionDef[] = [
  { id: 'open-palette',  label: 'Open command palette' },
  { id: 'toggle-sidebar', label: 'Toggle sidebar width' },
  { id: 'toggle-hud',    label: 'Cycle HUD display' },
  { id: 'dispatch-intent', label: 'Dispatch intent (in composer)' },
  { id: 'pause-resume-agent', label: 'Pause/resume selected agent' },
];
export type Bindings = Record<string, string>;
export const DEFAULT_BINDINGS: Bindings = {
  'open-palette': 'Mod+K',
  'toggle-sidebar': 'Mod+B',
  'toggle-hud': 'Mod+Shift+H',
  'dispatch-intent': 'Mod+Enter',
  'pause-resume-agent': 'Mod+.',
};
export function chordFromEvent(e: Pick<KeyboardEvent,'key'|'metaKey'|'ctrlKey'|'shiftKey'|'altKey'>): string {
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push('Mod');
  if (e.shiftKey) parts.push('Shift');
  if (e.altKey) parts.push('Alt');
  const k = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  parts.push(k);
  return parts.join('+');
}
export function matchAction(chord: string, bindings: Bindings): ActionId | null {
  const hit = (Object.keys(bindings) as ActionId[]).find(id => bindings[id] === chord);
  return hit ?? null;
}
export function serializeBindings(b: Bindings): string { return JSON.stringify(b); }
export function parseBindings(json: string | null): Bindings {
  if (!json) return { ...DEFAULT_BINDINGS };
  try { return { ...DEFAULT_BINDINGS, ...JSON.parse(json) }; } catch { return { ...DEFAULT_BINDINGS }; }
}
