import { describe, it, expect } from 'vitest';
import { useLudusStore } from './store';

describe('Ludus Zustand Store', () => {
  it('correctly sets and updates citizen positions', () => {
    const store = useLudusStore.getState();
    store.updateAgent('agent_1', { x: 5, y: 7 });

    const updated = useLudusStore.getState().agents['agent_1'];
    expect(updated).toBeDefined();
    expect(updated.x).toBe(5);
    expect(updated.y).toBe(7);
  });
});
