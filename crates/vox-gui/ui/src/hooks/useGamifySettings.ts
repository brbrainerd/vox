import { useEffect, useState } from 'react';
import { voxTransport, type GamifySettingsDto } from '../transport';

export type GamifySettings = GamifySettingsDto;

const DEFAULT_SETTINGS: GamifySettings = { enabled: true, mode: 'balanced' };

/** Polls `get_gamify_settings` — Settings remains SSOT for enable/mode. */
export function useGamifySettings(pollMs = 30_000): GamifySettings {
  const [settings, setSettings] = useState<GamifySettings>(DEFAULT_SETTINGS);

  useEffect(() => {
    let cancelled = false;
    const load = () => {
      voxTransport
        .getGamifySettings()
        .then((s) => {
          if (!cancelled) setSettings(s);
        })
        .catch(() => {});
    };
    load();
    const id = window.setInterval(load, pollMs);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [pollMs]);

  return settings;
}
