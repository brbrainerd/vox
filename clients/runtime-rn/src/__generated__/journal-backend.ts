// @ts-nocheck
//
// On-device durable journal backend, backed by expo-file-system (one
// append-only NDJSON file per table under the app document directory). This
// works in BOTH Expo Go and a native dev-client and survives a full app kill +
// relaunch — no native module dependency required.
//
// This file lives under `src/__generated__/` (excluded from the package's tsc
// gate) and is loaded by `runtime.ts` only through a string-variable dynamic
// import, so its `expo-file-system` dependency never burdens the contract tsc
// check. `// @ts-nocheck` matches the sibling generated bindings.
//
// (A higher-durability variant can swap in the uniffi `vox-journal` FileJournal
// — which fsyncs each append — by changing only this file.)

import * as FileSystem from "expo-file-system";

const DIR = (FileSystem.documentDirectory || "") + "vox-journal/";
const safe = (t) => String(t).replace(/[^A-Za-z0-9_-]/g, "_");
const fileFor = (t) => DIR + safe(t) + ".ndjson";

async function ensureDir() {
  try {
    const info = await FileSystem.getInfoAsync(DIR);
    if (!info.exists) await FileSystem.makeDirectoryAsync(DIR, { intermediates: true });
  } catch {
    // best-effort; a failed mkdir surfaces on the subsequent write
  }
}

async function readText(path) {
  try {
    return await FileSystem.readAsStringAsync(path);
  } catch {
    return "";
  }
}

const backend = {
  async record(table, row) {
    await ensureDir();
    const path = fileFor(table);
    const existing = await readText(path);
    await FileSystem.writeAsStringAsync(path, existing + JSON.stringify(row) + "\n");
  },
  async replay(table) {
    const txt = await readText(fileFor(table));
    return txt
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  },
};

export default backend;
