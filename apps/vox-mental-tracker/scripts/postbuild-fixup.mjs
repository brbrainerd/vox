#!/usr/bin/env node
/**
 * Post-vox-build codegen fixups.
 *
 * Now a no-op. Every prior patch has landed in the Vox compiler:
 * - handler invocation / async await / `.length()` → `.length`
 * - runtime globals via the emitted `dist/runtime-install.ts`
 * - the web bootstrap (entry.tsx / vox-app.tsx) is emitted
 * - app glue (error boundary + service-worker registration) is emitted into the
 *   default `dist/app-hooks.tsx` (wiring `dist/vox-error-boundary.tsx` +
 *   `dist/vox-sw-register.ts`), so this app needs no hand-written TypeScript and
 *   no app-hooks override.
 *
 * Kept as a build hook point; remove alongside the `build:fixup` script once we
 * confirm no future emergency patch is needed.
 */
console.log("postbuild-fixup: no patches needed (app is fully Vox-generated)");
