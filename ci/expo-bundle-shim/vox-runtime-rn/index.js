// JS-only shim for @vox/runtime-rn.
//
// The real package is a uniffi-bindgen-react-native TurboModule backed by the
// `vox-runtime-rn` Rust cdylib; it needs a native dev build. This stand-in lets
// the *generated Expo app* boot and render in Expo Go so we can prove the
// codegen output is a runnable app on a real emulator. Every method logs and
// returns a benign default — no native code, no real device effects.

const log = (name, ...args) =>
  console.log(`[vox-runtime-rn shim] ${name}`, ...args);

export const voxRuntime = {
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
