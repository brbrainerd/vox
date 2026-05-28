// `@vox/runtime` — Tauri 2-backed implementation of `@vox/runtime-types::VoxRuntime`.
//
// Every method is wired to a concrete Tauri API. There are no stubs; methods that
// cannot be expressed on desktop (none today, but reserved for capability gaps) throw
// `VoxRuntimeError("UnsupportedOnPlatform", ...)` so Vox source never silently no-ops.

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

/// The singleton runtime instance used by emitted Vox apps.
///
/// Vox source emits `import { voxRuntime } from "@vox/runtime"` and calls methods on it.
/// On desktop, every method routes through Tauri's `invoke`/`listen` to the linked
/// `vox-runtime` Rust crate.
export const voxRuntime = createVoxRuntime();
