import { beforeEach, describe, expect, it, vi } from "vitest";

describe("runtime shims (vox builtins on globalThis)", () => {
  beforeEach(async () => {
    vi.resetModules();
    delete (globalThis as unknown as { __TAURI__?: unknown }).__TAURI__;
    delete (globalThis as unknown as { window?: Window }).window;
    (globalThis as unknown as { window?: Window }).window = globalThis as unknown as Window;
    await import("../src/runtime");
  });

  it("installs Speech.transcribe_microphone", () => {
    expect(typeof (globalThis as unknown as { Speech?: { transcribe_microphone: unknown } }).Speech?.transcribe_microphone).toBe(
      "function",
    );
  });

  it("mirrors mobile.transcribe_microphone", () => {
    const g = globalThis as unknown as {
      Speech?: { transcribe_microphone: () => unknown };
      mobile?: { transcribe_microphone: () => unknown };
    };
    expect(g.Speech?.transcribe_microphone).toBe(g.mobile?.transcribe_microphone);
  });
});
