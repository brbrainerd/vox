import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  type ActorHandle,
  type AppState,
  type BackButtonHandler,
  type DeepLinkHandler,
  type PushHandlers,
  type Unsubscribe,
  type VoxRuntime,
  VoxRuntimeError,
  type WorkflowHandle,
} from "@vox/runtime-types";

// Tauri emits these events from the Rust side of the shell.
const EVT_APP_STATE = "vox-app-state";
const EVT_BACK_BUTTON = "vox-back-button";
const EVT_DEEP_LINK = "vox-deep-link";
const EVT_PUSH_REGISTRATION = "vox-push-registration";
const EVT_PUSH_NOTIFICATION = "vox-push-notification";
const EVT_PUSH_ACTION = "vox-push-action";

/// Tauri commands invoked from the Vox runtime methods. These names mirror the
/// `vox-runtime` crate's `#[tauri::command]` exports — keep both in sync.
const CMD_NOTIFY = "vox_runtime_notify";
const CMD_TAKE_PHOTO = "vox_runtime_take_photo";
const CMD_VIBRATE = "vox_runtime_vibrate";
const CMD_TRANSCRIBE = "vox_runtime_transcribe";
const CMD_TRANSCRIBE_MIC = "vox_runtime_transcribe_microphone";
const CMD_SPAWN_ACTOR = "vox_runtime_spawn_actor";
const CMD_ACTOR_SEND = "vox_runtime_actor_send";
const CMD_ACTOR_CLOSE = "vox_runtime_actor_close";
const CMD_START_WORKFLOW = "vox_runtime_start_workflow";
const CMD_WORKFLOW_AWAIT = "vox_runtime_workflow_await";
const CMD_WORKFLOW_SUSPEND = "vox_runtime_workflow_suspend";
const CMD_WORKFLOW_RESUME = "vox_runtime_workflow_resume";
const CMD_INFER = "vox_runtime_infer";
const CMD_REQUEST_PUSH = "vox_runtime_request_push_registration";
const CMD_EXIT = "plugin:process|exit";

/// Adapt a `listen(...)` promise into an `Unsubscribe`. The Tauri `listen` API resolves
/// to an `UnlistenFn`; we wrap it so callers can synchronously cancel even if the
/// `listen` promise hasn't resolved yet.
function asUnsubscribe<T>(promise: Promise<(...args: T[]) => void>): Unsubscribe {
  let unlisten: ((...args: T[]) => void) | undefined;
  let cancelled = false;
  void promise.then((fn) => {
    if (cancelled) {
      fn();
    } else {
      unlisten = fn;
    }
  });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

function asRuntimeError(e: unknown): VoxRuntimeError {
  if (e instanceof VoxRuntimeError) return e;
  return new VoxRuntimeError("Internal", e instanceof Error ? e.message : String(e));
}

interface TauriBackButtonPayload {
  // Reserved for future per-event metadata; today empty.
}

interface TauriDeepLinkPayload {
  url: string;
}

interface TauriPushRegistrationPayload {
  token: string;
}

interface TauriPushNotificationPayload {
  data: unknown;
}

interface TauriAppStatePayload {
  state: AppState;
}

class TauriActorHandle implements ActorHandle {
  constructor(public readonly id: string) {}
  send(message: Uint8Array): void {
    void invoke(CMD_ACTOR_SEND, { id: this.id, message: Array.from(message) });
  }
  close(): void {
    void invoke(CMD_ACTOR_CLOSE, { id: this.id });
  }
}

class TauriWorkflowHandle implements WorkflowHandle {
  constructor(public readonly id: string) {}
  async await(): Promise<Uint8Array> {
    try {
      const bytes = await invoke<number[]>(CMD_WORKFLOW_AWAIT, { id: this.id });
      return new Uint8Array(bytes);
    } catch (e) {
      throw asRuntimeError(e);
    }
  }
  suspend(): void {
    void invoke(CMD_WORKFLOW_SUSPEND, { id: this.id });
  }
  resume(): void {
    void invoke(CMD_WORKFLOW_RESUME, { id: this.id });
  }
}

class TauriVoxRuntime implements VoxRuntime {
  // ── Lifecycle ────────────────────────────────────────────────────────────

  onAppStateChange(handler: (state: AppState) => void): Unsubscribe {
    return asUnsubscribe(
      listen<TauriAppStatePayload>(EVT_APP_STATE, (e) => {
        handler(e.payload.state);
      }),
    );
  }

  // ── Mobile primitives ───────────────────────────────────────────────────

  onBackButton(handler: BackButtonHandler): Unsubscribe {
    return asUnsubscribe(
      listen<TauriBackButtonPayload>(EVT_BACK_BUTTON, async () => {
        let consumed = false;
        try {
          consumed = await Promise.resolve(handler());
        } catch (e) {
          // Surface uncaught handler errors via console; do not crash the runtime.
          console.error("[vox/runtime] back-button handler threw:", e);
        }
        if (!consumed) {
          void invoke(CMD_EXIT);
        }
      }),
    );
  }

  onDeepLink(handler: DeepLinkHandler): Unsubscribe {
    return asUnsubscribe(
      listen<TauriDeepLinkPayload>(EVT_DEEP_LINK, async (e) => {
        try {
          const route = await Promise.resolve(handler(e.payload.url));
          if (route !== null && route !== undefined) {
            // The host app's router subscribes via `onDeepLink`'s return path —
            // we re-emit a routing event for it to pick up. This keeps the runtime
            // free of any framework-specific (TanStack / Expo Router) coupling.
            window.dispatchEvent(new CustomEvent("vox:navigate", { detail: route }));
          }
        } catch (e) {
          console.error("[vox/runtime] deep-link handler threw:", e);
        }
      }),
    );
  }

  async installPushNotifications(handlers: PushHandlers): Promise<void> {
    // Register OS push first; if denied the rejection bubbles before we wire listeners.
    try {
      await invoke(CMD_REQUEST_PUSH);
    } catch (e) {
      throw asRuntimeError(e);
    }
    if (handlers.onRegister) {
      const cb = handlers.onRegister;
      void listen<TauriPushRegistrationPayload>(EVT_PUSH_REGISTRATION, async (e) => {
        try {
          await Promise.resolve(cb(e.payload.token));
        } catch (err) {
          console.error("[vox/runtime] push onRegister threw:", err);
        }
      });
    }
    if (handlers.onNotification) {
      const cb = handlers.onNotification;
      void listen<TauriPushNotificationPayload>(EVT_PUSH_NOTIFICATION, async (e) => {
        try {
          await Promise.resolve(cb(e.payload.data));
        } catch (err) {
          console.error("[vox/runtime] push onNotification threw:", err);
        }
      });
    }
    if (handlers.onAction) {
      const cb = handlers.onAction;
      void listen<TauriPushNotificationPayload>(EVT_PUSH_ACTION, async (e) => {
        try {
          await Promise.resolve(cb(e.payload.data));
        } catch (err) {
          console.error("[vox/runtime] push onAction threw:", err);
        }
      });
    }
  }

  // ── std.mobile bridge ───────────────────────────────────────────────────

  async notify(title: string, body: string): Promise<void> {
    try {
      await invoke(CMD_NOTIFY, { title, body });
    } catch (e) {
      throw asRuntimeError(e);
    }
  }

  async takePhoto(): Promise<string> {
    try {
      return await invoke<string>(CMD_TAKE_PHOTO);
    } catch (e) {
      throw asRuntimeError(e);
    }
  }

  async vibrate(): Promise<void> {
    try {
      await invoke(CMD_VIBRATE);
    } catch (e) {
      throw asRuntimeError(e);
    }
  }

  async transcribe(audioBytes: Uint8Array, langHint?: string): Promise<string> {
    try {
      return await invoke<string>(CMD_TRANSCRIBE, {
        audioBytes: Array.from(audioBytes),
        langHint: langHint ?? null,
      });
    } catch (e) {
      throw asRuntimeError(e);
    }
  }

  async transcribeMicrophone(): Promise<string> {
    try {
      return await invoke<string>(CMD_TRANSCRIBE_MIC);
    } catch (e) {
      throw asRuntimeError(e);
    }
  }

  // ── Vox-runtime first-class APIs ────────────────────────────────────────

  spawnActor(name: string, initState: Uint8Array): ActorHandle {
    // Spawn is async on the Rust side; we return a handle immediately whose `id` is
    // assigned by the runtime. Subsequent `send`/`close` block on the spawn settling.
    const idPromise = invoke<string>(CMD_SPAWN_ACTOR, {
      name,
      initState: Array.from(initState),
    });
    // Resolve synchronously by creating a proxy handle that waits for the id.
    return new PendingActorHandle(idPromise);
  }

  startWorkflow(id: string, payload: Uint8Array): WorkflowHandle {
    void invoke(CMD_START_WORKFLOW, {
      id,
      payload: Array.from(payload),
    });
    return new TauriWorkflowHandle(id);
  }

  async infer(modelId: string, input: Uint8Array): Promise<Uint8Array> {
    try {
      const bytes = await invoke<number[]>(CMD_INFER, {
        modelId,
        input: Array.from(input),
      });
      return new Uint8Array(bytes);
    } catch (e) {
      throw asRuntimeError(e);
    }
  }
}

/// Actor handle that buffers `send`/`close` until the Rust runtime returns the id.
class PendingActorHandle implements ActorHandle {
  private resolvedId: string | undefined;
  private queue: Array<() => void> = [];

  constructor(idPromise: Promise<string>) {
    void idPromise.then(
      (id) => {
        this.resolvedId = id;
        for (const op of this.queue) op();
        this.queue.length = 0;
      },
      (e) => {
        console.error("[vox/runtime] spawnActor failed:", e);
      },
    );
  }

  get id(): string {
    return this.resolvedId ?? "<pending>";
  }

  send(message: Uint8Array): void {
    const op = () => {
      if (this.resolvedId) {
        void invoke(CMD_ACTOR_SEND, {
          id: this.resolvedId,
          message: Array.from(message),
        });
      }
    };
    if (this.resolvedId) op();
    else this.queue.push(op);
  }

  close(): void {
    const op = () => {
      if (this.resolvedId) {
        void invoke(CMD_ACTOR_CLOSE, { id: this.resolvedId });
      }
    };
    if (this.resolvedId) op();
    else this.queue.push(op);
  }
}

export function createVoxRuntime(): VoxRuntime {
  return new TauriVoxRuntime();
}
