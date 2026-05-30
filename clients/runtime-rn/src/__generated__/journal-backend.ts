// @ts-nocheck
//
// Real on-device journal backend: wires the uniffi-bridged `vox-journal`
// FileJournal (generated `vox_runtime_rn.ts`) to a writable path from
// expo-file-system. This file lives under `src/__generated__/` — which the
// package's tsconfig.test.json EXCLUDES — and is loaded by `runtime.ts` only
// through a string-variable dynamic import, so neither tsc nor the contract
// gate ever sees the native dependency. `// @ts-nocheck` matches the sibling
// generated bindings.
//
// Swapping the durable store (e.g. to expo-sqlite) later is a change to THIS
// file alone — `runtime.ts` and the codegen are unaffected.

import { openFileJournal } from "./vox_runtime_rn";
import * as FileSystem from "expo-file-system";

const backend = {
  openFileJournal(path) {
    return openFileJournal(path);
  },
  documentDirectory() {
    return FileSystem.documentDirectory;
  },
};

export default backend;
