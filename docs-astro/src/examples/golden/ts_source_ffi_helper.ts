// Sibling TS helper for examples/golden/ts_source_ffi.vox.
//
// We test the extern-fn surface against `zod` because vox-codegen-ts already
// emits zod schemas (see crates/vox-compiler/src/codegen_ts/zod_emit.rs), so
// every project produced by `vox build` ships with zod as a runtime dep —
// the test target therefore has zero incremental dependency cost.

import { z } from "zod";

const emailSchema = z.string().email();
const positiveIntSchema = z.number().int().positive();

/** Validate that `s` is a well-formed email address. */
export function isValidEmail(s: string): boolean {
    return emailSchema.safeParse(s).success;
}

/** Coerce `n` to a positive integer, falling back to `0` on failure. */
export function clampPositive(n: number): number {
    const r = positiveIntSchema.safeParse(n);
    return r.success ? r.data : 0;
}
