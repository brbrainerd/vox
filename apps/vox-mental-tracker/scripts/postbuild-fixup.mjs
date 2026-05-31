#!/usr/bin/env node
/**
 * Post-vox-build codegen fixups.
 *
 * All prior codegen-gap patches have landed in the compiler (handler invocation,
 * `.length()` → `.length`, async await, and the runtime globals now resolve via
 * the Vox-emitted `dist/runtime-install.ts`).
 *
 * The one remaining job: inject this app's bootstrap glue. The Vox web emitter
 * writes a DEFAULT no-op `dist/app-hooks.tsx` (so `entry.tsx`'s `./app-hooks`
 * import resolves for any app). This app overrides it with a re-export of the
 * app-owned `src/app-hooks.tsx` (React error boundary + service-worker
 * registration), keeping that glue source-relative (its `./ErrorBoundary` /
 * `./sync` imports resolve from `src/`).
 */
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const distAppHooks = join(here, "..", "dist", "app-hooks.tsx");

writeFileSync(
  distAppHooks,
  `// Overridden by postbuild-fixup.mjs — re-export the app-owned bootstrap glue.\n` +
    `export { wrapApp, onBoot } from "../src/app-hooks";\n`,
);

console.log("postbuild-fixup: injected app-hooks override (dist/app-hooks.tsx -> ../src/app-hooks)");
