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
          // A misbehaving/incomplete backend mock (or a genuinely absent
          // command) can resolve with null/undefined; trust the default
          // rather than let a nullish settings object crash every reader
          // of gamifySettings.enabled across the app.
          if (!cancelled) setSettings(s ?? DEFAULT_SETTINGS);
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
