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

  // Mirrors the VoxRuntime contract in clients/runtime-types/src/index.ts.
  // Kept strongly typed (no index signature) so the tsc gate catches real
  // misuse of the on-device runtime surface in emitted code.
  export interface VoxRuntime {
    onAppStateChange(handler: (state: string) => void): () => void;
    onBackButton(handler: () => boolean | Promise<boolean>): () => void;
    onDeepLink(
      handler: (
        url: string,
      ) => string | null | undefined | Promise<string | null | undefined>,
    ): () => void;
    installPushNotifications(handlers: {
      onRegister?: (token: string) => void | Promise<void>;
      onNotification?: (payload: unknown) => void | Promise<void>;
      onAction?: (payload: unknown) => void | Promise<void>;
    }): Promise<void>;
    notify(title: string, body: string): Promise<void>;
    takePhoto(): Promise<string>;
    vibrate(): Promise<void>;
    transcribe(audioBytes: Uint8Array, langHint?: string): Promise<string>;
    transcribeMicrophone(): Promise<string>;
    spawnActor(name: string, initState: Uint8Array): unknown;
    startWorkflow(id: string, payload: Uint8Array): unknown;
    infer(modelId: string, input: Uint8Array): Promise<Uint8Array>;
    recordMutation(name: string, table: string, row: unknown): Promise<void>;
    replayTable(table: string): Promise<unknown[]>;
    uuid(): string;
  }

  export function createVoxRuntime(): VoxRuntime;
  // Singleton instance the real package exports (clients/runtime-rn/index.ts);
  // emitted mobile.ts / mobile-utils.ts import it directly.
  export const voxRuntime: VoxRuntime;
}
