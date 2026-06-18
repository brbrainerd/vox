import type { OpenLocator } from '../types/tauri';

/** Map an `open_locator` DTO to the GUI view that should host the result. */
export function viewKeyForLocator(locator: OpenLocator): string {
  switch (locator.kind) {
    case 'file':
      return 'repository';
    case 'web':
      return 'browser';
    case 'command':
      return 'catalog';
    case 'chat':
      return 'chat';
    case 'memory':
      return 'memory';
    case 'setting':
      return 'settings';
    default:
      return 'search';
  }
}
