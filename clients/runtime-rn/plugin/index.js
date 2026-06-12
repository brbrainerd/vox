// Expo config plugin for @vox/runtime-rn (spec §9.3,
// docs/src/architecture/mobile-rn-expo-implementation-spec-2026.md).
//
// Consumed by `expo prebuild` via @expo/config-plugins. It:
//   1. Links the precompiled native library into the consuming app:
//      Android — copies `libvox_runtime_rn.so` per-ABI into
//      `android/app/src/main/jniLibs/<abi>/`; iOS — copies
//      `vox_runtime_rn.xcframework` into `ios/Frameworks/` and registers it
//      with the Xcode project.
//   2. Adds permission entries gated by the features the consuming app
//      declares (AndroidManifest.xml `uses-permission` + Info.plist usage
//      descriptions, default strings overridable via props).
//   3. Registers Candle model asset paths as bundled assets via
//      `assetBundlePatterns`.
//
// This file lives in its own `plugin/` directory with `"type": "commonjs"`
// because the package root is `"type": "module"` — Expo's plugin resolver
// loads plugins with `require()`, so the implementation must be CommonJS.
//
// @expo/config-plugins is require()d lazily inside the plugin function: it is
// guaranteed to be present in any Expo app running `expo prebuild` (declared
// as an optional peerDependency), but must not be needed merely to import the
// pure helpers below (which is how the unit tests exercise this module).

"use strict";

const fs = require("node:fs");
const path = require("node:path");

// ---------------------------------------------------------------------------
// Feature → permission tables
// ---------------------------------------------------------------------------

const ANDROID_ABIS = Object.freeze(["arm64-v8a", "armeabi-v7a", "x86", "x86_64"]);

const ANDROID_LIB_NAME = "libvox_runtime_rn.so";
const IOS_FRAMEWORK_NAME = "vox_runtime_rn.xcframework";

/** feature → Android permission names. */
const ANDROID_PERMISSIONS = Object.freeze({
  microphone: ["android.permission.RECORD_AUDIO"],
  notifications: ["android.permission.POST_NOTIFICATIONS", "android.permission.VIBRATE"],
  camera: ["android.permission.CAMERA"],
});

/** feature → { Info.plist key, default usage-description string }. */
const IOS_USAGE_DESCRIPTIONS = Object.freeze({
  microphone: {
    key: "NSMicrophoneUsageDescription",
    default: "This app uses the microphone to record and transcribe audio.",
  },
  camera: {
    key: "NSCameraUsageDescription",
    default: "This app uses the camera to take photos.",
  },
});

const KNOWN_FEATURES = Object.freeze([
  ...new Set([...Object.keys(ANDROID_PERMISSIONS), ...Object.keys(IOS_USAGE_DESCRIPTIONS)]),
]);

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested directly in tests/plugin.test.mjs)
// ---------------------------------------------------------------------------

/**
 * Validate and normalize plugin props.
 *
 * @param {object|undefined} props
 * @returns {{features: string[], models: string[], descriptions: object,
 *            androidAbis: string[], androidLibDir: string|undefined,
 *            iosFrameworkDir: string|undefined}}
 */
function validateProps(props) {
  const p = props ?? {};
  const features = p.features ?? [];
  if (!Array.isArray(features)) {
    throw new TypeError("@vox/runtime-rn plugin: `features` must be an array of strings");
  }
  for (const f of features) {
    if (!KNOWN_FEATURES.includes(f)) {
      throw new Error(
        `@vox/runtime-rn plugin: unknown feature "${f}". Known features: ${KNOWN_FEATURES.join(", ")}`,
      );
    }
  }
  const models = p.models ?? [];
  if (!Array.isArray(models) || models.some((m) => typeof m !== "string")) {
    throw new TypeError("@vox/runtime-rn plugin: `models` must be an array of asset path strings");
  }
  const androidAbis = p.androidAbis ?? [...ANDROID_ABIS];
  if (!Array.isArray(androidAbis)) {
    throw new TypeError("@vox/runtime-rn plugin: `androidAbis` must be an array");
  }
  for (const abi of androidAbis) {
    if (!ANDROID_ABIS.includes(abi)) {
      throw new Error(
        `@vox/runtime-rn plugin: unknown Android ABI "${abi}". Supported: ${ANDROID_ABIS.join(", ")}`,
      );
    }
  }
  const descriptions = p.descriptions ?? {};
  if (typeof descriptions !== "object" || descriptions === null || Array.isArray(descriptions)) {
    throw new TypeError("@vox/runtime-rn plugin: `descriptions` must be an object");
  }
  return {
    features: [...features],
    models: [...models],
    descriptions,
    androidAbis: [...androidAbis],
    androidLibDir: p.androidLibDir,
    iosFrameworkDir: p.iosFrameworkDir,
  };
}

/**
 * Add feature-gated `uses-permission` entries to an AndroidManifest JSON
 * object (the `modResults` shape from withAndroidManifest). Deduplicates
 * against existing entries. Returns the same (mutated) object.
 */
function addPermissionsToManifest(manifestJson, features) {
  const manifest = manifestJson.manifest;
  const wanted = features.flatMap((f) => ANDROID_PERMISSIONS[f] ?? []);
  if (wanted.length === 0) return manifestJson;
  if (!Array.isArray(manifest["uses-permission"])) {
    manifest["uses-permission"] = [];
  }
  const existing = new Set(
    manifest["uses-permission"].map((p) => p.$ && p.$["android:name"]).filter(Boolean),
  );
  for (const name of wanted) {
    if (!existing.has(name)) {
      manifest["uses-permission"].push({ $: { "android:name": name } });
      existing.add(name);
    }
  }
  return manifestJson;
}

/**
 * Add feature-gated usage-description keys to an Info.plist object (the
 * `modResults` shape from withInfoPlist). Explicit `descriptions` props win;
 * otherwise existing app-authored values are preserved; otherwise the plugin
 * default is used. Returns the same (mutated) object.
 */
function addUsageDescriptionsToInfoPlist(infoPlist, features, descriptions) {
  for (const feature of features) {
    const entry = IOS_USAGE_DESCRIPTIONS[feature];
    if (!entry) continue;
    const override = descriptions[feature];
    if (typeof override === "string" && override.length > 0) {
      infoPlist[entry.key] = override;
    } else if (typeof infoPlist[entry.key] !== "string" || infoPlist[entry.key].length === 0) {
      infoPlist[entry.key] = entry.default;
    }
  }
  return infoPlist;
}

/**
 * Merge Candle model asset paths into an `assetBundlePatterns` list,
 * preserving order and deduplicating. Returns a new array.
 */
function modelAssetPatterns(existingPatterns, models) {
  const out = [...(existingPatterns ?? [])];
  for (const m of models) {
    if (!out.includes(m)) out.push(m);
  }
  return out;
}

/**
 * Compute the per-ABI copy operations that place `libvox_runtime_rn.so` into
 * the consuming app's `android/app/src/main/jniLibs/<abi>/` directory.
 *
 * @param {{projectRoot: string, libDir: string, abis: string[]}} args
 *   `libDir` contains one subdirectory per ABI, each holding the `.so`.
 * @returns {{abi: string, src: string, dest: string}[]}
 */
function androidCopyOps({ projectRoot, libDir, abis }) {
  return abis.map((abi) => ({
    abi,
    src: path.join(libDir, abi, ANDROID_LIB_NAME),
    dest: path.join(projectRoot, "android", "app", "src", "main", "jniLibs", abi, ANDROID_LIB_NAME),
  }));
}

/**
 * Compute the iOS framework copy operation and the POSIX path under which the
 * Xcode project references it (relative to the `ios/` directory).
 *
 * @param {{projectRoot: string, frameworkDir: string}} args
 * @returns {{src: string, dest: string, xcodePath: string}}
 */
function iosFrameworkOps({ projectRoot, frameworkDir }) {
  const name = path.basename(frameworkDir);
  return {
    src: frameworkDir,
    dest: path.join(projectRoot, "ios", "Frameworks", name),
    xcodePath: `Frameworks/${name}`,
  };
}

// ---------------------------------------------------------------------------
// Impure plumbing (filesystem + @expo/config-plugins mods)
// ---------------------------------------------------------------------------

function loadConfigPlugins() {
  try {
    // Lazy: only needed when the plugin actually runs inside `expo prebuild`,
    // where the Expo app guarantees @expo/config-plugins is installed.
    return require("@expo/config-plugins");
  } catch (cause) {
    throw new Error(
      "@vox/runtime-rn plugin requires @expo/config-plugins, which ships with every Expo app. " +
        "Run this plugin via `expo prebuild` (or install @expo/config-plugins).",
      { cause },
    );
  }
}

function copyRecursive(src, dest) {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.cpSync(src, dest, { recursive: true });
}

/** Default location of the prebuilt per-ABI Android libs inside this package. */
function defaultAndroidLibDir() {
  return path.join(__dirname, "..", "android", "jniLibs");
}

/** Default location of the prebuilt iOS xcframework inside this package. */
function defaultIosFrameworkDir() {
  return path.join(__dirname, "..", "ios", IOS_FRAMEWORK_NAME);
}

function withVoxAndroidNativeLib(config, props, plugins) {
  return plugins.withDangerousMod(config, [
    "android",
    (cfg) => {
      const libDir = props.androidLibDir
        ? path.resolve(cfg.modRequest.projectRoot, props.androidLibDir)
        : defaultAndroidLibDir();
      const ops = androidCopyOps({
        projectRoot: cfg.modRequest.platformProjectRoot
          ? path.dirname(cfg.modRequest.platformProjectRoot)
          : cfg.modRequest.projectRoot,
        libDir,
        abis: props.androidAbis,
      });
      const copied = [];
      for (const op of ops) {
        if (fs.existsSync(op.src)) {
          copyRecursive(op.src, op.dest);
          copied.push(op.abi);
        }
      }
      if (copied.length === 0) {
        throw new Error(
          `@vox/runtime-rn plugin: no prebuilt ${ANDROID_LIB_NAME} found under ${libDir} ` +
            `for ABIs [${props.androidAbis.join(", ")}]. Build the native library first ` +
            "(see clients/runtime-rn/scripts/generate-bindings.mjs and spec §9), or point " +
            "the `androidLibDir` plugin prop at a directory containing <abi>/" +
            ANDROID_LIB_NAME +
            ".",
        );
      }
      return cfg;
    },
  ]);
}

function withVoxIosNativeLib(config, props, plugins) {
  // Copy the xcframework into ios/Frameworks/ ...
  config = plugins.withDangerousMod(config, [
    "ios",
    (cfg) => {
      const frameworkDir = props.iosFrameworkDir
        ? path.resolve(cfg.modRequest.projectRoot, props.iosFrameworkDir)
        : defaultIosFrameworkDir();
      const ops = iosFrameworkOps({
        projectRoot: cfg.modRequest.projectRoot,
        frameworkDir,
      });
      if (!fs.existsSync(ops.src)) {
        throw new Error(
          `@vox/runtime-rn plugin: prebuilt iOS framework not found at ${ops.src}. ` +
            "Build the native library first (see spec §9), or point the `iosFrameworkDir` " +
            "plugin prop at the vox_runtime_rn.xcframework directory.",
        );
      }
      copyRecursive(ops.src, ops.dest);
      return cfg;
    },
  ]);
  // ...and register it with the Xcode project.
  return plugins.withXcodeProject(config, (cfg) => {
    const project = cfg.modResults;
    const frameworkDir = props.iosFrameworkDir
      ? path.resolve(cfg.modRequest.projectRoot, props.iosFrameworkDir)
      : defaultIosFrameworkDir();
    const { xcodePath } = iosFrameworkOps({
      projectRoot: cfg.modRequest.projectRoot,
      frameworkDir,
    });
    if (!project.hasFile(xcodePath)) {
      project.addFramework(xcodePath, { customFramework: true, embed: false, link: true });
    }
    return cfg;
  });
}

// ---------------------------------------------------------------------------
// Plugin entry
// ---------------------------------------------------------------------------

/**
 * Expo config plugin entry.
 *
 * Usage in app.json / app.config.js:
 *   "plugins": [["@vox/runtime-rn/plugin", {
 *     "features": ["microphone", "notifications", "camera"],
 *     "models": ["assets/models/whisper-tiny.safetensors"],
 *     "descriptions": { "microphone": "Custom mic reason." }
 *   }]]
 */
function withVoxRuntime(config, rawProps) {
  const props = validateProps(rawProps);
  const plugins = loadConfigPlugins();

  // 2. Feature-gated permissions.
  config = plugins.withAndroidManifest(config, (cfg) => {
    cfg.modResults = addPermissionsToManifest(cfg.modResults, props.features);
    return cfg;
  });
  config = plugins.withInfoPlist(config, (cfg) => {
    cfg.modResults = addUsageDescriptionsToInfoPlist(
      cfg.modResults,
      props.features,
      props.descriptions,
    );
    return cfg;
  });

  // 3. Candle model assets, bundled via assetBundlePatterns.
  if (props.models.length > 0) {
    config.assetBundlePatterns = modelAssetPatterns(config.assetBundlePatterns, props.models);
  }

  // 1. Native library linking.
  config = withVoxAndroidNativeLib(config, props, plugins);
  config = withVoxIosNativeLib(config, props, plugins);

  return config;
}

module.exports = withVoxRuntime;
module.exports.default = withVoxRuntime;
// Pure helpers, exported for unit testing.
module.exports.KNOWN_FEATURES = KNOWN_FEATURES;
module.exports.ANDROID_ABIS = ANDROID_ABIS;
module.exports.validateProps = validateProps;
module.exports.addPermissionsToManifest = addPermissionsToManifest;
module.exports.addUsageDescriptionsToInfoPlist = addUsageDescriptionsToInfoPlist;
module.exports.modelAssetPatterns = modelAssetPatterns;
module.exports.androidCopyOps = androidCopyOps;
module.exports.iosFrameworkOps = iosFrameworkOps;
