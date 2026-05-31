// JS-only shim for @vox/runtime-rn.
//
// The real package is a uniffi-bindgen-react-native TurboModule backed by the
// `vox-runtime-rn` Rust cdylib; it needs a native dev build. This stand-in lets
// the *generated Expo app* boot and render in Expo Go so we can prove the
// codegen output is a runnable app on a real emulator. Every method logs and
// returns a benign default — no native code, no real device effects.

import * as FileSystem from "expo-file-system";

const log = (name, ...args) =>
  console.log(`[vox-runtime-rn shim] ${name}`, ...args);

// CROSS-RELAUNCH DURABLE persistence via expo-file-system (works in Expo Go —
// no native module needed). One append-only NDJSON file per table under the
// app's document directory; data survives a full app kill + relaunch.
const _DIR = (FileSystem.documentDirectory || "") + "vox-journal/";
const _safe = (t) => String(t).replace(/[^A-Za-z0-9_-]/g, "_");
const _file = (t) => _DIR + _safe(t) + ".ndjson";
async function _ensureDir() {
  try {
    const info = await FileSystem.getInfoAsync(_DIR);
    if (!info.exists) await FileSystem.makeDirectoryAsync(_DIR, { intermediates: true });
  } catch (e) {
    log("ensureDir failed", e);
  }
}
async function _readText(path) {
  try {
    return await FileSystem.readAsStringAsync(path);
  } catch {
    return "";
  }
}

export const voxRuntime = {
  async recordMutation(name, table, row) {
    log("recordMutation", name, table);
    await _ensureDir();
    const path = _file(table);
    const existing = await _readText(path);
    await FileSystem.writeAsStringAsync(path, existing + JSON.stringify(row) + "\n");
  },
  async replayTable(table) {
    log("replayTable", table);
    const txt = await _readText(_file(table));
    return txt
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  },
  uuid() {
    return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
      const r = (Math.random() * 16) | 0;
      return (c === "x" ? r : (r & 0x3) | 0x8).toString(16);
    });
  },
  notify(title, body) {
    log("notify", title, body);
    return Promise.resolve();
  },
  vibrate() {
    log("vibrate");
    return Promise.resolve();
  },
  takePhoto() {
    log("takePhoto");
    return Promise.resolve("");
  },
  transcribe(_bytes, hint) {
    log("transcribe", hint);
    return Promise.resolve("");
  },
  transcribeMicrophone() {
    log("transcribeMicrophone");
    return Promise.resolve("(shim) microphone transcription unavailable in Expo Go");
  },
  installPushNotifications(handlers) {
    log("installPushNotifications", Object.keys(handlers || {}));
    return () => log("uninstallPushNotifications");
  },
  onBackButton(cb) {
    log("onBackButton (registered)");
    void cb;
    return () => log("onBackButton (unsubscribed)");
  },
  onDeepLink(cb) {
    log("onDeepLink (registered)");
    void cb;
    return () => log("onDeepLink (unsubscribed)");
  },
};

// The generated on-device vox-client imports these too.
export class VoxRuntimeError extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
    this.name = "VoxRuntimeError";
  }
}
export function createVoxRuntime() {
  return voxRuntime;
}

export default { voxRuntime, createVoxRuntime, VoxRuntimeError };
