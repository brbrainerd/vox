/**
 * Browser-side runtime shims for Vox-emitted React.
 *
 * The codegen emits bare references to Vox builtins like `str(...)`,
 * `Speech.transcribe_microphone()`, `std.time.now_ms()` that have no JS
 * import. Until the compiler emits proper imports for these (tracked in
 * docs/superpowers/plans/language/2026-05-08-codegen-ts-bugs-blocking-tracker.md
 * follow-up), this module installs them on globalThis so the emitted
 * code resolves at runtime.
 *
 * Imported once from src/main.tsx — the side-effects do all the work.
 */

declare global {
  // eslint-disable-next-line no-var
  var str: (x: unknown) => string;
  // eslint-disable-next-line no-var
  var len: (x: unknown) => number;
  // eslint-disable-next-line no-var
  var Speech: {
    transcribe_microphone: () => SpeechResult | Promise<SpeechResult>;
  };
  // eslint-disable-next-line no-var
  var std: {
    time: { now_ms: () => number };
    crypto: { uuid: () => string; hash_secure: (s: string) => Promise<string> };
    json: { parse: (s: string) => unknown };
    regex: { compile: (p: string) => { _tag: "Ok"; _p0: RegExp } | { _tag: "Error"; _p0: string } };
  };
  // eslint-disable-next-line no-var
  var mobile: {
    transcribe_microphone: () => SpeechResult | Promise<SpeechResult>;
  };
}

type SpeechResult = { _tag: "Ok"; _p0: string } | { _tag: "Error"; _p0: string };

(globalThis as unknown as { str: typeof globalThis.str }).str = (x: unknown): string => String(x);
(globalThis as unknown as { len: typeof globalThis.len }).len = (x: unknown): number => {
  if (Array.isArray(x)) return x.length;
  if (typeof x === "string") return x.length;
  if (x && typeof x === "object" && "length" in x) return (x as { length: number }).length;
  return 0;
};

/**
 * Returns a SpeechResult synchronously when __VOX_TEST_TRANSCRIPT__ is set
 * (so the codegen-emitted match-on-_tag works without an `await`), or a
 * Promise otherwise (Web Speech API / Capacitor plugin paths). The async
 * path requires codegen-side support for awaiting before destructuring,
 * tracked compiler-side.
 */
function transcribeMicrophone(): SpeechResult | Promise<SpeechResult> {
  const testTranscript = (globalThis as unknown as { __VOX_TEST_TRANSCRIPT__?: string })
    .__VOX_TEST_TRANSCRIPT__;
  if (typeof testTranscript === "string") {
    return { _tag: "Ok", _p0: testTranscript };
  }

  // Web Speech API (browser). Capacitor builds wire this to SpeechRecognizer
  // via the Sherpa plugin package; this fallback keeps the dev/preview path
  // working for in-browser smoke tests.
  const SR =
    (window as unknown as { SpeechRecognition?: unknown; webkitSpeechRecognition?: unknown })
      .SpeechRecognition ||
    (window as unknown as { SpeechRecognition?: unknown; webkitSpeechRecognition?: unknown })
      .webkitSpeechRecognition;

  if (typeof SR === "function") {
    return new Promise<SpeechResult>((resolve) => {
      const r = new (SR as new () => {
        continuous: boolean;
        interimResults: boolean;
        lang: string;
        start(): void;
        onresult: (e: { results: ArrayLike<ArrayLike<{ transcript: string }>> }) => void;
        onerror: (e: { error: string }) => void;
      })();
      r.continuous = false;
      r.interimResults = false;
      r.lang = "en-US";
      r.onresult = (e) => {
        const transcript = e.results[0]?.[0]?.transcript ?? "";
        resolve({ _tag: "Ok", _p0: transcript });
      };
      r.onerror = (e) => resolve({ _tag: "Error", _p0: e.error });
      try {
        r.start();
      } catch (e) {
        resolve({ _tag: "Error", _p0: String(e) });
      }
    });
  }

  // Last-resort prompt for environments without Web Speech API.
  const text = window.prompt("Voice input (no STT engine available — type instead):");
  if (text) return { _tag: "Ok", _p0: text };
  return { _tag: "Error", _p0: "cancelled" };
}

const speechShim = { transcribe_microphone: transcribeMicrophone };
(globalThis as unknown as { Speech: typeof globalThis.Speech }).Speech = speechShim;
(globalThis as unknown as { mobile: typeof globalThis.mobile }).mobile = speechShim;

(globalThis as unknown as { std: typeof globalThis.std }).std = {
  time: { now_ms: () => Date.now() },
  crypto: {
    uuid: () =>
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `vox-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`,
    hash_secure: async (s: string): Promise<string> => {
      const buf = new TextEncoder().encode(s);
      const digest = await crypto.subtle.digest("SHA-256", buf);
      return Array.from(new Uint8Array(digest))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
    },
  },
  json: { parse: (s: string) => JSON.parse(s) },
  regex: {
    compile: (p: string) => {
      try {
        return { _tag: "Ok" as const, _p0: new RegExp(p) };
      } catch (e) {
        return { _tag: "Error" as const, _p0: String(e) };
      }
    },
  },
};

export {};
