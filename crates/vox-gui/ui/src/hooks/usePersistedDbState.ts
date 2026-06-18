import { useState, useEffect, useRef } from 'react';
import { voxTransport } from '../transport';

export function usePersistedDbState<T>(key: string, initialValue: T) {
  const [storedValue, setStoredValue] = useState<T>(initialValue);
  const [isLoaded, setIsLoaded] = useState(false);
  const pendingWrite = useRef<T | undefined>(undefined);

  useEffect(() => {
    let active = true;
    voxTransport.getGuiPreference(key)
      .then((val) => {
        if (!active) return;
        if (val) {
          try {
            setStoredValue(JSON.parse(val));
          } catch (e) {
            console.error(e);
          }
        }
        setIsLoaded(true);
        if (pendingWrite.current !== undefined) {
          voxTransport.setGuiPreference(key, JSON.stringify(pendingWrite.current))
            .catch(err => console.error("Failed to save preference to db", err));
          pendingWrite.current = undefined;
        }
      })
      .catch((err) => {
        console.error("Failed to load preference from db", err);
        setIsLoaded(true);
      });
    return () => { active = false; };
  }, [key]);

  const setValue = (value: T | ((val: T) => T)) => {
    try {
      const valueToStore = value instanceof Function ? value(storedValue) : value;
      setStoredValue(valueToStore);
      if (isLoaded) {
        voxTransport.setGuiPreference(key, JSON.stringify(valueToStore))
          .catch(err => console.error("Failed to save preference to db", err));
      } else {
        pendingWrite.current = valueToStore;
      }
    } catch (error) {
      console.error(error);
    }
  };

  return [storedValue, setValue, isLoaded] as const;
}
