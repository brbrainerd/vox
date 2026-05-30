// JS-only shim for @vox/runtime-rn.
//
// The real package is a uniffi-bindgen-react-native TurboModule backed by the
// `vox-runtime-rn` Rust cdylib; it needs a native dev build. This stand-in lets
// the *generated Expo app* boot and render in Expo Go so we can prove the
// codegen output is a runnable app on a real emulator. Every method logs and
// returns a benign default — no native code, no real device effects.

const log = (name, ...args) =>
  console.log(`[vox-runtime-rn shim] ${name}`, ...args);

// In-memory per-table store. Real durability needs the native journal; in
// Expo Go this persists for the session, enough to demo the data flow.
const _tables = new Map();

export const voxRuntime = {
  recordMutation(name, table, row) {
    log("recordMutation", name, table);
    const rows = _tables.get(table) || [];
    rows.push(row);
    _tables.set(table, rows);
    return Promise.resolve();
  },
  replayTable(table) {
    log("replayTable", table);
    return Promise.resolve([...(_tables.get(table) || [])]);
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

export default { voxRuntime };
