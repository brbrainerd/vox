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

// The Expo SDK modules are declared as peer dependencies in package.json so
// they bind to whatever versions the consuming Expo project has installed.
// Imports use the `import type`-style runtime shape that Metro can tree-shake.
import { AppState as RnAppState, BackHandler, Platform } from "react-native";
import * as Linking from "expo-linking";
import * as Notifications from "expo-notifications";
import * as Haptics from "expo-haptics";
import * as ImagePicker from "expo-image-picker";

/// Map React Native's AppStateStatus → our portable `AppState` enum.
function normalizeAppState(status: string): AppState {
  switch (status) {
    case "active":
      return "active";
    case "background":
      return "background";
    case "inactive":
      return "inactive";
    default:
      return "inactive";
  }
}

function asRuntimeError(e: unknown): VoxRuntimeError {
  if (e instanceof VoxRuntimeError) return e;
  return new VoxRuntimeError("Internal", e instanceof Error ? e.message : String(e));
}

/// Convenience helper — every "Rust runtime method not yet wired" call site
/// uses the same error shape.
function uniffiNotWired(method: string): never {
  throw new VoxRuntimeError(
    "UnsupportedOnPlatform",
    `${method}: the @vox/runtime-rn uniffi bridge is not yet wired. ` +
      `See docs/src/architecture/mobile-rn-expo-implementation-spec-2026.md §11.`,
  );
}

// ── On-device journal backend (tsc-decoupled) ──────────────────────────────
//
// The real durable store is the uniffi-bridged `vox-journal` FileJournal, whose
// generated bindings live under `src/__generated__/` and depend on the native
// module + expo-file-system. Those are EXCLUDED from the package's tsc gate. To
// keep `runtime.ts` type-checkable WITHOUT pulling the excluded generated code
// into the program, we (a) declare a thin local interface here and (b) load the
// real implementation through a STRING-VARIABLE dynamic import whose specifier
// tsc cannot statically resolve (a literal import would re-introduce the
// excluded file; a static import fails outright). The impl is
// `src/__generated__/journal-backend.ts`.
interface JournalLineLike {
  json: string;
}
interface FileJournalHandleLike {
  append(line: JournalLineLike): void;
  replayAll(): JournalLineLike[];
}
interface JournalBackend {
  openFileJournal(path: string): FileJournalHandleLike;
  documentDirectory(): string;
}

let cachedJournalBackend: JournalBackend | undefined;
async function journalBackend(): Promise<JournalBackend> {
  if (cachedJournalBackend) return cachedJournalBackend;
  // Non-literal specifier on purpose — keeps the excluded backend opaque to tsc.
  const spec = "./__generated__/journal-backend.js";
  const mod = (await import(/* @vite-ignore */ spec)) as { default: JournalBackend };
  cachedJournalBackend = mod.default;
  return cachedJournalBackend;
}

const __voxJournalHandles = new Map<string, FileJournalHandleLike>();
function sanitizeTableName(table: string): string {
  return table.replace(/[^A-Za-z0-9_-]/g, "_");
}

class ExpoVoxRuntime implements VoxRuntime {
  // ── Lifecycle ────────────────────────────────────────────────────────────

  onAppStateChange(handler: (state: AppState) => void): Unsubscribe {
    const sub = RnAppState.addEventListener("change", (status) => {
      handler(normalizeAppState(status));
    });
    return () => sub.remove();
  }

  // ── Mobile primitives ───────────────────────────────────────────────────

  onBackButton(handler: BackButtonHandler): Unsubscribe {
    if (Platform.OS !== "android") {
      // iOS doesn't have a hardware back button. Returning a no-op
      // unsubscribe lets the same Vox source compile and run cross-platform
      // without per-platform branches.
      return () => {};
    }
    const onPress = () => {
      let consumed = false;
      try {
        const result = handler();
        // `BackHandler` requires a synchronous boolean. If the handler
        // returns a Promise we treat it as "consumed = true" optimistically
        // and let the promise resolve in the background.
        if (typeof result === "object" && result !== null && "then" in result) {
          (result as Promise<boolean>).catch((e) =>
            console.error("[vox/runtime-rn] back-button handler rejected:", e),
          );
          consumed = true;
        } else {
          consumed = result;
        }
      } catch (e) {
        console.error("[vox/runtime-rn] back-button handler threw:", e);
      }
      return consumed;
    };
    const sub = BackHandler.addEventListener("hardwareBackPress", onPress);
    return () => sub.remove();
  }

  onDeepLink(handler: DeepLinkHandler): Unsubscribe {
    const sub = Linking.addEventListener("url", async (event) => {
      try {
        const route = await Promise.resolve(handler(event.url));
        if (route !== null && route !== undefined) {
          // Hand the navigation target back to the host's router via a
          // global custom event. The RN emit's `useDeepLinkRouting` hook
          // listens for it and calls the navigation function.
          if (typeof globalThis !== "undefined") {
            const dispatch = (globalThis as { dispatchEvent?: (e: unknown) => void })
              .dispatchEvent;
            if (typeof dispatch === "function") {
              try {
                dispatch.call(globalThis, { type: "vox:navigate", detail: route });
              } catch {
                // dispatchEvent may not be available on every JS engine;
                // silent fallback is intentional here.
              }
            }
          }
        }
      } catch (e) {
        console.error("[vox/runtime-rn] deep-link handler threw:", e);
      }
    });
    return () => sub.remove();
  }

  async installPushNotifications(handlers: PushHandlers): Promise<void> {
    // Request notification permission. On iOS this prompts the user; on
    // Android (API >= 33) the POST_NOTIFICATIONS runtime permission is
    // handled here too.
    const { status } = await Notifications.requestPermissionsAsync();
    if (status !== "granted") {
      throw new VoxRuntimeError(
        "Internal",
        `notification permission not granted (status: ${status})`,
      );
    }

    if (handlers.onRegister) {
      const cb = handlers.onRegister;
      try {
        const token = await Notifications.getExpoPushTokenAsync();
        await Promise.resolve(cb(token.data));
      } catch (e) {
        console.error("[vox/runtime-rn] push onRegister threw:", e);
      }
    }

    if (handlers.onNotification) {
      const cb = handlers.onNotification;
      Notifications.addNotificationReceivedListener(async (notif) => {
        try {
          await Promise.resolve(cb(notif.request.content.data));
        } catch (e) {
          console.error("[vox/runtime-rn] push onNotification threw:", e);
        }
      });
    }

    if (handlers.onAction) {
      const cb = handlers.onAction;
      Notifications.addNotificationResponseReceivedListener(async (resp) => {
        try {
          await Promise.resolve(cb(resp.notification.request.content.data));
        } catch (e) {
          console.error("[vox/runtime-rn] push onAction threw:", e);
        }
      });
    }
  }

  // ── std.mobile bridge ───────────────────────────────────────────────────

  async notify(title: string, body: string): Promise<void> {
    try {
      await Notifications.scheduleNotificationAsync({
        content: { title, body },
        trigger: null,
      });
    } catch (e) {
      throw asRuntimeError(e);
    }
  }

  async takePhoto(): Promise<string> {
    try {
      const { status } = await ImagePicker.requestCameraPermissionsAsync();
      if (status !== "granted") {
        throw new VoxRuntimeError(
          "Internal",
          `camera permission not granted (status: ${status})`,
        );
      }
      const result = await ImagePicker.launchCameraAsync({
        mediaTypes: ImagePicker.MediaTypeOptions.Images,
        quality: 0.85,
      });
      if (result.canceled) {
        throw new VoxRuntimeError("Internal", "user canceled camera capture");
      }
      const asset = result.assets[0];
      if (!asset) {
        throw new VoxRuntimeError("Internal", "camera returned no asset");
      }
      return asset.uri;
    } catch (e) {
      throw asRuntimeError(e);
    }
  }

  async vibrate(): Promise<void> {
    try {
      await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium);
    } catch (e) {
      // Vibration is best-effort; surface failures via console rather than
      // throwing — Vox source assuming vibration always works should not
      // crash because the device lacks a vibrator.
      console.warn("[vox/runtime-rn] vibrate failed:", e);
    }
  }

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async transcribe(_audioBytes: Uint8Array, _langHint?: string): Promise<string> {
    // Backed by uniffi-bridged Candle Whisper running on-device. Until the
    // bridge lands, throw the explicit "not yet wired" error so callers
    // know to either wait for Phase 2 or set up a remote transcription
    // endpoint as an interim.
    uniffiNotWired("transcribe");
  }

  async transcribeMicrophone(): Promise<string> {
    uniffiNotWired("transcribeMicrophone");
  }

  // ── Vox-runtime first-class APIs (uniffi-bridged when wired) ────────────

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  spawnActor(_name: string, _initState: Uint8Array): ActorHandle {
    uniffiNotWired("spawnActor");
  }

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  startWorkflow(_id: string, _payload: Uint8Array): WorkflowHandle {
    uniffiNotWired("startWorkflow");
  }

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async infer(_modelId: string, _input: Uint8Array): Promise<Uint8Array> {
    uniffiNotWired("infer");
  }

  // ── On-device durable persistence ─────────────────────────────────────────

  private async handleFor(table: string): Promise<FileJournalHandleLike> {
    const key = sanitizeTableName(table);
    const cached = __voxJournalHandles.get(key);
    if (cached) return cached;
    const be = await journalBackend();
    const handle = be.openFileJournal(`${be.documentDirectory()}vox-journal/${key}.ndjson`);
    __voxJournalHandles.set(key, handle);
    return handle;
  }

  async recordMutation(name: string, table: string, row: unknown): Promise<void> {
    try {
      const handle = await this.handleFor(table);
      // FileJournal fsyncs each append (vox-journal/src/file.rs), so the row is
      // durable and survives a relaunch once this resolves.
      handle.append({ json: JSON.stringify({ name, row }) });
    } catch (e) {
      throw asRuntimeError(e);
    }
  }

  async replayTable(table: string): Promise<unknown[]> {
    try {
      const handle = await this.handleFor(table);
      return handle
        .replayAll()
        .map((line) => (JSON.parse(line.json) as { row: unknown }).row);
    } catch (e) {
      throw asRuntimeError(e);
    }
  }

  uuid(): string {
    // RFC-4122 v4. Hermes lacks a reliable global crypto.randomUUID, so build
    // one from Math.random — record ids don't need cryptographic strength.
    return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
      const r = (Math.random() * 16) | 0;
      const v = c === "x" ? r : (r & 0x3) | 0x8;
      return v.toString(16);
    });
  }
}

export function createVoxRuntime(): VoxRuntime {
  return new ExpoVoxRuntime();
}
