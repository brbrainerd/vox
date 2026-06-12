// Unit tests for the @vox/runtime-rn Expo config plugin.
//
// config-plugins mods are pure functions over the Expo config object, so the
// plugin's core logic is tested here against mock config fragments — no real
// Expo project, no @expo/config-plugins install required (the dependency is
// only require()d lazily when the plugin runs inside `expo prebuild`).
//
// Run via `node --test tests/` (the `test` script in package.json).

import { test } from "node:test";
import assert from "node:assert/strict";
import path from "node:path";

import {
  KNOWN_FEATURES,
  validateProps,
  addPermissionsToManifest,
  addUsageDescriptionsToInfoPlist,
  modelAssetPatterns,
  androidCopyOps,
  iosFrameworkOps,
  ANDROID_ABIS,
} from "../plugin/index.js";

import rootPlugin from "../plugin.js";
import appPlugin from "../app.plugin.js";

// ---------------------------------------------------------------------------
// Plugin entry points
// ---------------------------------------------------------------------------

test("plugin.js default export is a config plugin function", () => {
  assert.equal(typeof rootPlugin, "function");
});

test("app.plugin.js re-exports the same plugin function", () => {
  assert.equal(appPlugin, rootPlugin);
});

// ---------------------------------------------------------------------------
// Prop validation
// ---------------------------------------------------------------------------

test("validateProps accepts empty/omitted props", () => {
  assert.deepEqual(validateProps(undefined).features, []);
  assert.deepEqual(validateProps({}).models, []);
});

test("validateProps rejects unknown features with a helpful error", () => {
  assert.throws(
    () => validateProps({ features: ["microphone", "telepathy"] }),
    new RegExp(`telepathy.*${[...KNOWN_FEATURES].join(", ").replace(/[-/\\^$*+?.()|[\]{}]/g, "\\$&")}`),
  );
});

test("validateProps rejects non-array models", () => {
  assert.throws(() => validateProps({ models: "model.safetensors" }), /models.*array/);
});

test("validateProps rejects unknown Android ABIs", () => {
  assert.throws(() => validateProps({ androidAbis: ["riscv64"] }), /riscv64/);
});

// ---------------------------------------------------------------------------
// AndroidManifest.xml permissions
// ---------------------------------------------------------------------------

function emptyManifest() {
  return { manifest: { $: { "xmlns:android": "http://schemas.android.com/apk/res/android" } } };
}

function permissionNames(manifestJson) {
  return (manifestJson.manifest["uses-permission"] ?? []).map((p) => p.$["android:name"]);
}

test("microphone feature adds RECORD_AUDIO", () => {
  const m = addPermissionsToManifest(emptyManifest(), ["microphone"]);
  assert.deepEqual(permissionNames(m), ["android.permission.RECORD_AUDIO"]);
});

test("notifications feature adds POST_NOTIFICATIONS and VIBRATE", () => {
  const m = addPermissionsToManifest(emptyManifest(), ["notifications"]);
  assert.deepEqual(permissionNames(m).sort(), [
    "android.permission.POST_NOTIFICATIONS",
    "android.permission.VIBRATE",
  ]);
});

test("camera feature adds CAMERA", () => {
  const m = addPermissionsToManifest(emptyManifest(), ["camera"]);
  assert.deepEqual(permissionNames(m), ["android.permission.CAMERA"]);
});

test("no features adds no permissions", () => {
  const m = addPermissionsToManifest(emptyManifest(), []);
  assert.deepEqual(permissionNames(m), []);
});

test("permissions are deduplicated against existing manifest entries", () => {
  const start = emptyManifest();
  start.manifest["uses-permission"] = [
    { $: { "android:name": "android.permission.RECORD_AUDIO" } },
  ];
  const m = addPermissionsToManifest(start, ["microphone", "camera"]);
  assert.deepEqual(permissionNames(m).sort(), [
    "android.permission.CAMERA",
    "android.permission.RECORD_AUDIO",
  ]);
});

// ---------------------------------------------------------------------------
// Info.plist usage descriptions
// ---------------------------------------------------------------------------

test("microphone feature sets NSMicrophoneUsageDescription with default text", () => {
  const plist = addUsageDescriptionsToInfoPlist({}, ["microphone"], {});
  assert.equal(typeof plist.NSMicrophoneUsageDescription, "string");
  assert.ok(plist.NSMicrophoneUsageDescription.length > 0);
  assert.equal(plist.NSCameraUsageDescription, undefined);
});

test("camera feature sets NSCameraUsageDescription with default text", () => {
  const plist = addUsageDescriptionsToInfoPlist({}, ["camera"], {});
  assert.ok(plist.NSCameraUsageDescription.length > 0);
  assert.equal(plist.NSMicrophoneUsageDescription, undefined);
});

test("notifications feature adds no Info.plist usage keys", () => {
  const plist = addUsageDescriptionsToInfoPlist({}, ["notifications"], {});
  assert.deepEqual(Object.keys(plist), []);
});

test("description props override the defaults", () => {
  const plist = addUsageDescriptionsToInfoPlist({}, ["microphone", "camera"], {
    microphone: "Mic for voice notes.",
  });
  assert.equal(plist.NSMicrophoneUsageDescription, "Mic for voice notes.");
  assert.notEqual(plist.NSCameraUsageDescription, "Mic for voice notes.");
});

test("existing app-authored plist values are preserved unless overridden", () => {
  const existing = { NSMicrophoneUsageDescription: "App-specific reason." };
  const noOverride = addUsageDescriptionsToInfoPlist({ ...existing }, ["microphone"], {});
  assert.equal(noOverride.NSMicrophoneUsageDescription, "App-specific reason.");
  const withOverride = addUsageDescriptionsToInfoPlist({ ...existing }, ["microphone"], {
    microphone: "Plugin override.",
  });
  assert.equal(withOverride.NSMicrophoneUsageDescription, "Plugin override.");
});

// ---------------------------------------------------------------------------
// Candle model assets
// ---------------------------------------------------------------------------

test("models prop is appended to assetBundlePatterns", () => {
  const patterns = modelAssetPatterns(undefined, ["assets/models/whisper-tiny.safetensors"]);
  assert.deepEqual(patterns, ["assets/models/whisper-tiny.safetensors"]);
});

test("model patterns are deduplicated and existing patterns kept", () => {
  const patterns = modelAssetPatterns(
    ["assets/images/*", "assets/models/a.gguf"],
    ["assets/models/a.gguf", "assets/models/b.gguf"],
  );
  assert.deepEqual(patterns, [
    "assets/images/*",
    "assets/models/a.gguf",
    "assets/models/b.gguf",
  ]);
});

test("no models leaves patterns untouched", () => {
  assert.deepEqual(modelAssetPatterns(["x/*"], []), ["x/*"]);
});

// ---------------------------------------------------------------------------
// Android native library copy plan
// ---------------------------------------------------------------------------

test("androidCopyOps emits one copy per default ABI", () => {
  const ops = androidCopyOps({
    projectRoot: "/app",
    libDir: "/pkg/android/jniLibs",
    abis: ANDROID_ABIS,
  });
  assert.equal(ops.length, 4);
  const abis = ops.map((o) => o.abi);
  assert.deepEqual(abis, ["arm64-v8a", "armeabi-v7a", "x86", "x86_64"]);
  for (const op of ops) {
    assert.equal(op.src, path.join("/pkg/android/jniLibs", op.abi, "libvox_runtime_rn.so"));
    assert.equal(
      op.dest,
      path.join("/app", "android", "app", "src", "main", "jniLibs", op.abi, "libvox_runtime_rn.so"),
    );
  }
});

test("androidCopyOps honors a custom ABI subset", () => {
  const ops = androidCopyOps({
    projectRoot: "/app",
    libDir: "/pkg/android/jniLibs",
    abis: ["arm64-v8a"],
  });
  assert.equal(ops.length, 1);
  assert.equal(ops[0].abi, "arm64-v8a");
});

// ---------------------------------------------------------------------------
// iOS framework plan
// ---------------------------------------------------------------------------

test("iosFrameworkOps computes copy source/dest and Xcode-relative path", () => {
  const ops = iosFrameworkOps({
    projectRoot: "/app",
    frameworkDir: "/pkg/ios/vox_runtime_rn.xcframework",
  });
  assert.equal(ops.src, "/pkg/ios/vox_runtime_rn.xcframework");
  assert.equal(ops.dest, path.join("/app", "ios", "Frameworks", "vox_runtime_rn.xcframework"));
  // Path stored in the Xcode project must be POSIX-relative to the ios/ dir.
  assert.equal(ops.xcodePath, "Frameworks/vox_runtime_rn.xcframework");
});
