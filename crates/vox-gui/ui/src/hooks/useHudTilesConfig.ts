import { useCallback, useMemo } from 'react';
import { useLocalStorage } from './useLocalStorage';
import { SHELL_PREFERENCE_KEYS } from '../lib/shellPersistence';
import {
  defaultHudTiles,
  validateHudTilesConfig,
  resolveVisibleHudTiles,
  type HudTilesConfig,
  type HudTileKind,
} from './useHudTiles';

function parseHudTilesConfig(raw: unknown): HudTilesConfig {
  try {
    return validateHudTilesConfig(raw);
  } catch {
    return defaultHudTiles();
  }
}

export function useHudTilesConfig(): {
  config: HudTilesConfig;
  setConfig: (next: HudTilesConfig) => void;
  visibleTiles: HudTileKind[];
} {
  const [stored, setStored] = useLocalStorage<unknown>(
    SHELL_PREFERENCE_KEYS.hudTiles,
    defaultHudTiles(),
  );

  const config = useMemo(() => parseHudTilesConfig(stored), [stored]);
  const visibleTiles = useMemo(() => resolveVisibleHudTiles(config), [config]);

  const setConfig = useCallback(
    (next: HudTilesConfig) => {
      setStored(validateHudTilesConfig(next));
    },
    [setStored],
  );

  return { config, setConfig, visibleTiles };
}
