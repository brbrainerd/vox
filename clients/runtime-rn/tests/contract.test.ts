// Contract test for `@vox/runtime-rn`.
//
// Pure-type checks via tsc (no runtime test framework). Run via
// `npx tsc -p tsconfig.test.json --noEmit`. If this file compiles, the
// `ExpoVoxRuntime` class satisfies the `VoxRuntime` interface and every
// uniffi-deferred method has the right "not yet wired" signature.

import type { VoxRuntime, ActorHandle, WorkflowHandle } from "@vox/runtime-types";
import { VoxRuntimeError } from "@vox/runtime-types";
import { voxRuntime, createVoxRuntime } from "../src/index.js";

// 1. The exported `voxRuntime` value must satisfy the VoxRuntime interface.
const _check_runtime_satisfies_contract: VoxRuntime = voxRuntime;
void _check_runtime_satisfies_contract;

// 2. `createVoxRuntime()` returns the same type.
const _check_create_returns_contract: VoxRuntime = createVoxRuntime();
void _check_create_returns_contract;

// 3. Lifecycle method returns an Unsubscribe function.
const _unsub: () => void = voxRuntime.onAppStateChange((s) => {
  // `s` must be the AppState union; assigning to a wider string forces a
  // compile-time check that the narrow type holds.
  const _wide: "active" | "background" | "inactive" = s;
  void _wide;
});
void _unsub;

// 4. Mobile primitive return types compile correctly.
const _backUnsub: () => void = voxRuntime.onBackButton(() => true);
void _backUnsub;
const _linkUnsub: () => void = voxRuntime.onDeepLink((url) => {
  const _url: string = url;
  void _url;
  return null;
});
void _linkUnsub;

// 5. std.mobile bridge methods return promises of the right shape.
const _notifyResult: Promise<void> = voxRuntime.notify("t", "b");
void _notifyResult;
const _photoResult: Promise<string> = voxRuntime.takePhoto();
void _photoResult;
const _vibrateResult: Promise<void> = voxRuntime.vibrate();
void _vibrateResult;
const _transcribeResult: Promise<string> = voxRuntime.transcribe(new Uint8Array());
void _transcribeResult;
const _micResult: Promise<string> = voxRuntime.transcribeMicrophone();
void _micResult;

// 6. Rust-runtime methods (uniffi-deferred) return the right handle types.
const _actor: ActorHandle = voxRuntime.spawnActor("name", new Uint8Array());
void _actor;
const _workflow: WorkflowHandle = voxRuntime.startWorkflow("id", new Uint8Array());
void _workflow;
const _inferResult: Promise<Uint8Array> = voxRuntime.infer("model", new Uint8Array());
void _inferResult;

// 7. The error class is the expected one (caught at runtime in the bridge).
function _check_error_type(): VoxRuntimeError {
  return new VoxRuntimeError("UnsupportedOnPlatform", "test");
}
void _check_error_type;

// If this file compiles, the contract holds.
export const _CONTRACT_OK: true = true;
