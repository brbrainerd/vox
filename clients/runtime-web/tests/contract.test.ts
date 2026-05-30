// Contract test for `@vox/runtime` (Tauri impl).
//
// Pure-type checks via tsc (no runtime test framework). Run via
// `npx tsc -p tsconfig.test.json --noEmit`. If this file compiles, the
// `TauriVoxRuntime` class satisfies the `VoxRuntime` interface.

import type { VoxRuntime, ActorHandle, WorkflowHandle } from "@vox/runtime-types";
import { VoxRuntimeError } from "@vox/runtime-types";
import { voxRuntime } from "../src/index.js";

// 1. The exported `voxRuntime` value must satisfy the VoxRuntime interface.
const _check_runtime_satisfies_contract: VoxRuntime = voxRuntime;
void _check_runtime_satisfies_contract;

// 2. Lifecycle method returns an Unsubscribe.
const _unsub: () => void = voxRuntime.onAppStateChange((s) => {
  const _wide: "active" | "background" | "inactive" = s;
  void _wide;
});
void _unsub;

// 3. Mobile-primitive return types compile correctly.
const _backUnsub: () => void = voxRuntime.onBackButton(() => true);
void _backUnsub;
const _linkUnsub: () => void = voxRuntime.onDeepLink((url) => {
  const _url: string = url;
  void _url;
  return null;
});
void _linkUnsub;

// 4. std.mobile bridge methods return promises of the right shape.
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

// 5. Vox-runtime methods (Tauri IPC backed) return the right handle types.
const _actor: ActorHandle = voxRuntime.spawnActor("name", new Uint8Array());
void _actor;
const _workflow: WorkflowHandle = voxRuntime.startWorkflow("id", new Uint8Array());
void _workflow;
const _inferResult: Promise<Uint8Array> = voxRuntime.infer("model", new Uint8Array());
void _inferResult;

// 6. The error class is the expected one.
function _check_error_type(): VoxRuntimeError {
  return new VoxRuntimeError("Internal", "test");
}
void _check_error_type;

export const _CONTRACT_OK: true = true;
