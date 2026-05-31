// Vox runtime interface contract — single source of truth.
//
// Every Vox emit target produces JavaScript that calls these methods on `voxRuntime`.
// Implementations:
//   - @vox/runtime       — desktop (Tauri 2 backed)
//   - @vox/runtime-rn    — mobile (React Native + Expo + uniffi backed)
//
// Adding a new mobile capability that Vox source can address means:
//   1. Add the method to `VoxRuntime` here.
//   2. Implement it in BOTH adapters.
//   3. Update `mobile_emit.rs` (or whichever lowering invokes it).
// No emitter has direct knowledge of Tauri or Expo; the contract is the only seam.

/// Lifecycle state reported to `onAppStateChange` handlers.
export type AppState = "active" | "background" | "inactive";

/// Subscription cancellation closure returned by `on*` methods.
export type Unsubscribe = () => void;

/// A handler that decides whether the back button was consumed.
/// Returning `false` lets the runtime fall back to its default
/// (e.g. exit-app on desktop, system back on Android).
export type BackButtonHandler = () => boolean | Promise<boolean>;

/// A handler invoked for an incoming deep-link URL. Returning a non-null
/// string routes the app to that path; returning `null` consumes the link
/// without navigation.
export type DeepLinkHandler = (url: string) => Promise<string | null> | string | null;

/// Push-notification lifecycle handlers.
export interface PushHandlers {
  /// Called when the OS issues (or rotates) a device push token.
  onRegister?: (token: string) => void | Promise<void>;
  /// Called when a remote notification arrives while the app is running.
  onNotification?: (payload: unknown) => void | Promise<void>;
  /// Called when the user taps a notification action button.
  onAction?: (payload: unknown) => void | Promise<void>;
}

/// Live handle to a Vox actor instance. Returned from `spawnActor`.
export interface ActorHandle {
  /// Stable identifier assigned by the runtime.
  readonly id: string;
  /// Send a message into the actor's mailbox (best-effort, non-blocking).
  send(message: Uint8Array): void;
  /// Close the mailbox; pending messages drain, no new sends accepted.
  close(): void;
}

/// Live handle to a Vox workflow instance. Returned from `startWorkflow`.
export interface WorkflowHandle {
  /// Stable identifier assigned by the runtime.
  readonly id: string;
  /// Resolve when the workflow reaches a terminal state.
  await(): Promise<Uint8Array>;
  /// Persist current journal entries; safe to call during app suspend.
  suspend(): void;
  /// Resume from the last persisted journal entry.
  resume(): void;
}

/// Vox-runtime errors raised from the underlying Rust runtime.
export class VoxRuntimeError extends Error {
  constructor(
    public readonly code: VoxRuntimeErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "VoxRuntimeError";
  }
}

export type VoxRuntimeErrorCode =
  | "NotInitialized"
  | "ModelLoadFailed"
  | "WorkflowNotFound"
  | "ActorNotFound"
  | "UnsupportedOnPlatform"
  | "Internal";

/// The platform-portable runtime contract.
///
/// Each method is mandatory; adapters that cannot fulfill a method on their platform
/// MUST throw `VoxRuntimeError("UnsupportedOnPlatform", ...)` rather than silently
/// no-op. This keeps the JS source target-agnostic without hiding split-brain bugs.
export interface VoxRuntime {
  // ── Lifecycle ────────────────────────────────────────────────────────────

  /// Subscribe to OS-level app lifecycle changes. Idempotent: registering twice
  /// returns two distinct subscription handles, each independently cancellable.
  onAppStateChange(handler: (state: AppState) => void): Unsubscribe;

  // ── Mobile primitives (Vox annotation surface) ───────────────────────────

  /// Subscribe to the hardware back button (Android) or simulated back gesture (iOS / desktop).
  /// The handler decides whether the press was consumed. Returns `Unsubscribe`.
  onBackButton(handler: BackButtonHandler): Unsubscribe;

  /// Subscribe to inbound deep-link URLs (iOS Universal Links, Android App Links,
  /// custom-scheme URLs, desktop-protocol handlers).
  onDeepLink(handler: DeepLinkHandler): Unsubscribe;

  /// Register push-notification handlers and request the OS-level permission if needed.
  /// Resolves once registration is complete (or rejects with `VoxRuntimeError`).
  installPushNotifications(handlers: PushHandlers): Promise<void>;

  // ── std.mobile bridge ────────────────────────────────────────────────────

  /// Display a system notification with the given title and body.
  notify(title: string, body: string): Promise<void>;

  /// Open the platform photo picker / camera. Resolves with the captured asset's local URI.
  takePhoto(): Promise<string>;

  /// Trigger a short haptic vibration (no-op on platforms without a vibrator).
  vibrate(): Promise<void>;

  /// Transcribe raw audio bytes to text via the on-device ML model.
  /// `langHint` is a BCP-47 tag (e.g. "en", "ja"); pass `undefined` to auto-detect.
  transcribe(audioBytes: Uint8Array, langHint?: string): Promise<string>;

  /// Open the microphone, record until the user stops, transcribe, and resolve.
  /// Convenience wrapper around `transcribe` for UI flows that don't need raw audio access.
  transcribeMicrophone(): Promise<string>;

  // ── Vox-runtime first-class APIs (uniffi-bridged on mobile, Tauri-IPC on desktop) ───

  /// Spawn a Vox actor with the given declared name and initial state bytes.
  spawnActor(name: string, initState: Uint8Array): ActorHandle;

  /// Start a Vox workflow instance identified by `id` with the given payload bytes.
  startWorkflow(id: string, payload: Uint8Array): WorkflowHandle;

  /// Run on-device ML inference. `modelId` matches a declared Vox model resource;
  /// `input` is the model-specific encoded input. Resolves with raw output bytes.
  infer(modelId: string, input: Uint8Array): Promise<Uint8Array>;

  // ── On-device durable persistence (@mutation / @query execution seam) ─────
  //
  // The codegen runs a @mutation/@query body locally on mobile (no server) and
  // routes its `db` operations through these. They are the ONLY storage seam:
  // richer reads (get/count/filter) fold over `replayTable` in the emitted body.

  /// Durably append `row` to `table`'s append-only journal (one `db.<table>.insert`).
  /// `name` is the mutation fn name (tracing). Survives app relaunch on mobile.
  recordMutation(name: string, table: string, row: unknown): Promise<void>;

  /// Replay every row appended for `table`, in insertion order. Backs
  /// `db.<table>.all()`; the emitted body applies any further projection.
  replayTable(table: string): Promise<unknown[]>;

  /// Generate a RFC-4122 UUID string (backs `std.crypto.uuid()`), used by
  /// mutations that mint ids before insert.
  uuid(): string;
}
