// `@vox/runtime-rn` — Expo-SDK backed implementation of @vox/runtime-types::VoxRuntime.
//
// Mobile-primitive methods (`onBackButton`, `onDeepLink`, `installPushNotifications`,
// `notify`, `vibrate`, `takePhoto`, `transcribeMicrophone`) call real Expo APIs.
//
// Rust-runtime methods (`spawnActor`, `startWorkflow`, `infer`) throw
// `VoxRuntimeError("UnsupportedOnPlatform", ...)` until the uniffi bridge
// (`vox-runtime-rn` Rust crate) lands. This is the honest gap, not a silent
// stub — see `docs/src/architecture/mobile-rn-expo-implementation-spec-2026.md` §11.

export { VoxRuntimeError } from "@vox/runtime-types";
export type {
  ActorHandle,
  AppState,
  BackButtonHandler,
  DeepLinkHandler,
  PushHandlers,
  Unsubscribe,
  VoxRuntime,
  VoxRuntimeErrorCode,
  WorkflowHandle,
} from "@vox/runtime-types";

import { createVoxRuntime } from "./runtime.js";

export { createVoxRuntime } from "./runtime.js";

/// The singleton runtime instance used by emitted Vox apps.
///
/// Vox source emits `import { voxRuntime } from "@vox/runtime-rn"` and calls
/// methods on it. On mobile, mobile-primitive calls route through the Expo
/// SDK; on-device Rust runtime calls route through uniffi (when wired).
export const voxRuntime = createVoxRuntime();
