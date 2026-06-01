// Ambient declarations covering the @tauri-apps/api surface this package consumes.
// Lets the contract test type-check without installing the full @tauri-apps/api
// (which the consuming Tauri project supplies at install time as a peerDependency).

declare module "@tauri-apps/api/event" {
  export function listen<T>(
    event: string,
    handler: (event: { payload: T }) => void | Promise<void>,
  ): Promise<() => void>;
}

declare module "@tauri-apps/api/core" {
  export function invoke<T = unknown>(
    cmd: string,
    args?: Record<string, unknown>,
  ): Promise<T>;
}
