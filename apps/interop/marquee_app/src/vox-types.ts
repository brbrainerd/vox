/**
 * vox-types.ts — Runtime shims for Vox built-in types
 *
 * Vox's type system has `list<T>` and `Result<T, E>` as built-ins.
 * TypeScript uses `T[]` and a discriminated union. This file bridges the gap
 * so that Vox-generated `dist/` files compile cleanly.
 *
 * Also defines Zod schemas for @table types (e.g. Item → ItemSchema).
 */
import { z } from 'zod'

// ─── Vox Built-in Type Aliases ────────────────────────────────────────────────

/** Vox `list<T>` → TypeScript `T[]` */
export type list<T> = T[]

/** Vox `Result<T>` → discriminated union */
export type Result<T, E = string> =
  | { ok: true; value: T }
  | { ok: false; error: E }

// ─── @table Item (from apps/interop/marquee_app/src/main.vox) ──────────────────────────────

export interface Item {
  id: string
  name: string
  value: number
  created_at: string
}

export const ItemSchema = z.object({
  id: z.string(),
  name: z.string(),
  value: z.number().int(),
  created_at: z.string(),
})
