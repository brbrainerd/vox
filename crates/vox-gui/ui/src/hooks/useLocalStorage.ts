import { useState, useEffect } from 'react';

/**
 * Browser-local persistence for GUI shell state. Canonical key names live in
 * `lib/shellPersistence.ts` / `contracts/gui/shell-persistence.v1.yaml`.
 * Prefer `voxTransport.getGuiPreference` for Tier-A prefs when backend sync is required.
 */
export function useLocalStorage<T>(key: string, initialValue: T) {
  const [storedValue, setStoredValue] = useState<T>(() => {
    try {
      const item = window.localStorage.getItem(key);
      return item ? JSON.parse(item) : initialValue;
    } catch (error) {
      console.log(error);
      return initialValue;
    }
  });

  useEffect(() => {
    try {
      window.localStorage.setItem(key, JSON.stringify(storedValue));
    } catch (error) {
      console.log(error);
    }
  }, [key, storedValue]);

  return [storedValue, setStoredValue] as const;
}
