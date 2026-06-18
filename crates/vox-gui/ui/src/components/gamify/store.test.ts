import { describe, it, expect, beforeEach } from 'vitest';
import { useLudusStore } from './store';

describe('Ludus Zustand Store', () => {
  beforeEach(() => {
    useLudusStore.getState().reset();
  });

  it('correctly sets and updates citizen positions', () => {
    const store = useLudusStore.getState();
    store.updateAgent('agent_1', { x: 5, y: 7 });

    const updated = useLudusStore.getState().agents['agent_1'];
    expect(updated).toBeDefined();
    expect(updated.x).toBe(5);
    expect(updated.y).toBe(7);
  });

  it('populates defaults for new agents', () => {
    const store = useLudusStore.getState();
    store.updateAgent('agent_2', { x: 1, y: 1 });

    const updated = useLudusStore.getState().agents['agent_2'];
    expect(updated.energy).toBe(100);
    expect(updated.mood).toBe('Happy');
  });

  it('supports partial updates preserving coordinates', () => {
    const store = useLudusStore.getState();
    store.updateAgent('agent_3', { x: 2, y: 3 });
    store.updateAgent('agent_3', { energy: 75, mood: 'Tired' });

    const updated = useLudusStore.getState().agents['agent_3'];
    expect(updated.x).toBe(2);
    expect(updated.y).toBe(3);
    expect(updated.energy).toBe(75);
    expect(updated.mood).toBe('Tired');
  });

  it('correctly tracks warning and error statuses on building files', () => {
    const store = useLudusStore.getState();
    store.updateBuilding('src/lib.rs', { x: 3, y: 5, warnings: 2, errors: 0 });

    const building = useLudusStore.getState().buildings['src/lib.rs'];
    expect(building).toBeDefined();
    expect(building.x).toBe(3);
    expect(building.y).toBe(5);
    expect(building.warnings).toBe(2);
    expect(building.errors).toBe(0);
  });
});
