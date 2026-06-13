import { invoke } from '@tauri-apps/api/core';

/**
 * Typed wrappers over the Task-18 Discovery Inbox Tauri commands. Every invoke
 * arg key is camelCase — the Tauri boundary deserializes these into the
 * snake_case Rust command parameters.
 *
 * The inbox lists UNACKNOWLEDGED surfaced research candidates (rows in
 * `scientia_discovery_inbox`); acknowledging a row removes it from the list.
 */

/** One unacknowledged surfaced research candidate (inbox row). */
export interface DiscoveryInboxRow {
  id: number;
  publication_id: string;
  surfaced_at_ms: number;
  /** e.g. `strong_candidate` | `review_suggested` | `auto_intake`. */
  intake_tier: string;
  signal_codes: string[];
}

/** List unacknowledged discoveries, newest first (default limit 50). */
export function listDiscoveryInbox(limit?: number): Promise<DiscoveryInboxRow[]> {
  return invoke<DiscoveryInboxRow[]>('list_discovery_inbox', {
    limit: limit ?? null,
  });
}

/** Mark a discovery row acknowledged; it then drops out of the inbox list. */
export function acknowledgeDiscovery(id: number): Promise<void> {
  return invoke<void>('acknowledge_discovery', { id });
}
