/**
 * vox-globals.d.ts — Ambient declarations for Vox built-in types
 *
 * Vox-generated dist/ files reference these without explicit imports.
 * This global declaration file makes them available workspace-wide.
 */
import type { z } from 'zod'

declare global {
  /** Vox `list<T>` → TypeScript `T[]` */
  type list<T> = T[]

  /** Vox `Result<T>` — server fn return type */
  type Result<T, E = string> =
    | { ok: true; value: T }
    | { ok: false; error: E }

  /** @table Item (marquee_app) */
  interface Item {
    id: string
    name: string
    value: number
    created_at: string
  }

  /** Zod schema for Item — referenced in vox-client.ts */
  const ItemSchema: z.ZodObject<{
    id: z.ZodString
    name: z.ZodString
    value: z.ZodNumber
    created_at: z.ZodString
  }>
}

export {}
