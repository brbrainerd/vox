// Ambient stub for the on-device runtime package, dropped into each fixture's
// out dir by the tsc gate (see `assert_tsc_compiles`). The generated RN
// `vox-client.ts` preamble imports `@vox/runtime-rn`; the real Expo app resolves
// the actual native-backed package from its node_modules, but the cli-tests
// harness has no node install for it. This declaration lets `tsc --noEmit`
// resolve the import and type-check the emitted on-device bodies without pulling
// the heavy Expo-native dependency chain.
declare module "@vox/runtime-rn" {
  export class VoxRuntimeError extends Error {
    constructor(code: string, message: string);
    readonly code: string;
  }

  export interface VoxRuntime {
    recordMutation(name: string, table: string, row: unknown): Promise<void>;
    replayTable(table: string): Promise<unknown[]>;
    uuid(): string;
    notify(title: string, body: string): Promise<void>;
    vibrate(): Promise<void>;
    takePhoto(): Promise<string>;
    transcribe(audioBytes: Uint8Array, langHint?: string): Promise<string>;
    transcribeMicrophone(): Promise<string>;
    // Forward-compat: other VoxRuntime methods type-check without re-stubbing.
    [method: string]: unknown;
  }

  export function createVoxRuntime(): VoxRuntime;
}
