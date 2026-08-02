---
title: "Vox Axis STT Accuracy Design (2026-08-01)"
description: "Design for fixing voice-dictation accuracy in the Vox Axis GUI: wiring the unused eval harness, fixing broken correction rules, switching the default ASR backend from Candle-Whisper-tiny to NeMo Parakeet-TDT via sherpa-onnx, unblocking code-dictation symbol expansion, and exposing the resulting knobs in Settings."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: false
---

# Vox Axis STT Accuracy Design (2026-08-01)

## Problem

Voice dictation into the Vox Axis GUI chat composer (the Loquela surface) goes
through a real, fairly mature pipeline — Candle Whisper capture, VAD, a
rule-based + optional-LLM correction pass, contextual bias, a lexicon — but a
code audit found the pipeline is unmeasured and has several concrete, fixable
defects. This design fixes the defects, replaces the default ASR engine with a
faster and more accurate one, and unblocks a code-dictation capability that
already has an implementation but is not reachable from the shipped GUI.

## Audit findings (input to this design)

1. **Code-dictation symbol expansion never fires in the GUI.**
   `speech_normalize.rs` has real "open paren" → `(`, "camel case get user
   name" → `getUserName` logic, but it is only applied as an alternate n-best
   candidate inside `transcript_rerank::build_transcript_candidates`
   (`crates/vox-speech/src/transcript_rerank.rs:25-29`). Candidate selection
   (`pick_best_transcript_index_with_raw`) is a no-op returning index 0 unless
   the `compiler-rerank` Cargo feature is enabled
   (`transcript_rerank.rs:101-116`). `vox-gui/Cargo.toml:40` enables
   `vox-speech` with only `features = ["stt-candle"]` — `compiler-rerank` is
   neither enabled nor a default feature
   (`crates/vox-speech/Cargo.toml:16`, `default = []`). Net effect: dictating
   code produces literal words, not symbols/casing, in the shipped app.

2. **The eval corpus is never run.** `crates/vox-speech/tests/fixtures/eval_manifests/`
   has three manifests, including a purpose-built code-dictation set
   (`vox_code_corpus_v1.jsonl`), and `eval.rs` has WER/CER scoring functions —
   but nothing in `cargo test` or CI reads these manifests. They're exercised
   only by a manual CLI eval command
   (`crates/vox-ml-cli/src/commands/oratio_cmd.rs:636-637`). No automated
   regression signal exists for ASR accuracy.

3. **Broken correction rules — and unreachable at runtime.** In
   `crates/vox-speech/src/refine/rules.rs`, `code_confusion_map()`'s entries
   — `("mut self", "mut self")` (line 43, an identity no-op),
   `("impl for", "impl for ")` (line 42, only appends a trailing space),
   `("box dine", "Box<dyn ")` (line 31, an unbalanced `<` with no closing
   logic), and `("print len", "println!")` / `("print el in", "println!")`
   (lines 39-40, unvalidated phonetic guesses) — are all keyed on multi-word
   phrases. `refine_transcript`'s matching loop (`rules.rs:115-153`) only
   looks up single whitespace-split tokens via `confusion.get(lower.as_str())`,
   so none of these multi-word keys can ever match: the map is dead code
   today, not a set of rules that misfire at runtime. Confirmed empirically —
   `refine_transcript("box dine error", &ctx)` returns the input unchanged on
   the current code. Any fix to the mapped values must be paired with a fix
   to this token-vs-phrase mismatch in the matching logic, or the corrected
   entries remain equally unreachable.

4. **Lexicon collisions with common English words.** `default_domain_lexicon`
   (`rules.rs:47-64`) includes generic words `"status"` and `"workflow"`,
   which get force-lowercased mid-transcript by `domain_lexicon_case`
   (`rules.rs:140-151`) regardless of normal English capitalization context.

5. **Default ASR model chosen without benchmarking.** `candle_whisper.rs:210,462`
   defaults to `openai/whisper-tiny.en` with no comments or docs discussing an
   accuracy/latency tradeoff, despite the WER/CER infra in finding #2 existing
   to measure exactly this choice.

6. **`oratio.rs`/`oratioVoiceInput.ts` are unwired, but not dead code —
   correction from an earlier pass of this audit.** `crates/vox-gui/src/commands/oratio.rs`
   has zero live frontend callers (confirmed via exhaustive search of
   `vox-gui/ui/src` for `invoke('oratio_transcribe'...)` — only its own tests
   reference it), so an initial pass of this audit called it dead code. On
   closer inspection it is not: it routes through
   `vox_plugin_host::cached_code_plugin("oratio")` (the `vox-plugin-speech`
   plugin, see `crates/vox-plugin-speech/Plugin.toml` — *"Extracted from
   vox-speech in Unit 4"*), which is the exact same plugin
   `crates/vox-orchestrator-mcp/src/oratio_tools.rs` already uses in
   **production** for agent-facing voice tools, and is the literal target of
   the TODO in `crates/vox-speech/Cargo.toml:21` ("drop stt-candle after
   audio-ingress rewire/retirement... once ingress is rewired through
   `vox_plugin_host::load_code_plugin` + `as_speech_to_text()`"). It is a
   correct implementation of the intended target architecture that the
   frontend was simply never switched to — not throwaway code.
   It is genuinely unwired for a real reason, though:
   `oratio_transcribe(seconds: f32)` records a **fixed duration**, while
   Loquela's actual dictation UX (`start_mic_capture` /
   `stop_mic_capture_and_transcribe` in `mic.rs`) is **push-to-talk**
   (user-controlled start/stop). Reconciling that interaction-model mismatch
   is a real, separable migration (routing `mic.rs`'s capture through the
   plugin's `SpeechToText`/`AudioCapture` accessors instead of calling
   `vox_speech::transcribe_path_detailed` in-process) — **out of scope for
   this design** and tracked as a separate follow-up
   (`task_f971226b`). This design does not touch `oratio.rs` or
   `oratioVoiceInput.ts`.

7. **No GUI Settings exposure.** ~30 `VOX_ORATIO_*` env vars
   (`crates/vox-speech/src/runtime_config.rs`) control STT behavior, but
   `Settings/SettingsView.tsx` has no voice/STT section — every knob requires
   editing environment variables or a TOML file outside the app.

## External research: is there a better ASR model than Whisper-tiny?

Yes. **NVIDIA Parakeet-TDT-0.6B (v2/v3)**, a non-autoregressive NeMo
transducer model:

- **6.32% WER** vs Whisper large-v3's **7.44%** on the Open ASR Leaderboard —
  and a much larger margin over `whisper-tiny.en`, the model Vox actually
  ships.
- Orders of magnitude faster at inference (RTFx in the thousands vs Whisper's
  tens/hundreds) — public benchmarks describe transcribing 60 minutes of audio
  in ~1 second on suitable hardware. This matters for a real-time dictation
  button, not just batch transcription. **Caveat**: that RTFx figure is
  steady-state batch throughput and does not include model-load/session-init
  time. In the current codebase, every dictation stop re-instantiates the ASR
  backend from scratch — `create_backend()` is called fresh per transcription
  with no caching (`crates/vox-speech/src/traits.rs`,
  `crates/vox-speech/src/backend_dispatch.rs`; no `OnceCell`/`LazyLock`/static
  backend cache exists anywhere in `vox-speech`). If Parakeet becomes the
  default under this same call pattern, resolving the ~671MB int8 ONNX model
  and constructing a fresh `OfflineRecognizer` ONNX Runtime session happens on
  every utterance, not once at startup — overhead this design does not
  measure and must account for before shipping (see the Phase 2 gate below).
- **CC-BY-4.0 license** — commercial use is permitted, but CC-BY-4.0 is not
  an unconditional grant: it conditions all use, commercial or not, on
  attribution (credit to the creator, a link to the license, and indication
  of any changes made). Neither this design nor the current product has an
  attribution mechanism (a NOTICE/third-party-licenses file, an About/Licenses
  panel in Settings, or README credit) to satisfy this for a shipped-default
  model; Phase 2 or Phase 4 needs to add one before Parakeet ships as the
  default.
- English-only (v2) or 25-language (v3, still English-strong) — sufficient for
  a coding-agent product where English technical dictation is the primary
  workload.
- Pre-built **int8-quantized ONNX exports already exist** for sherpa-onnx
  (e.g. `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8` on Hugging Face), using
  ~3GB RAM unquantized, less when quantized.

NVIDIA **Canary-1B-v2** was also evaluated (also CC-BY-4.0, ONNX exports
exist) but rejected for this use case: it's multilingual/translation-focused
and nearly 2x the parameter count, which is unnecessary for English technical
dictation and would cost more latency/memory for no accuracy benefit here.

**Why this is cheap to ship**: `crates/vox-speech/src/backends/sherpa_onnx.rs`
already exists and already wraps the `sherpa-onnx` Rust crate — it is
currently hardcoded to `OfflineWhisperModelConfig`. The `sherpa-onnx` bindings
also expose `OfflineTransducerModelConfig`, which is what NeMo transducer
models (Parakeet) need. The GUI does not currently compile this backend in at
all (`vox-gui/Cargo.toml:40` enables only `stt-candle`). Switching the default
engine is a config/wiring change at the source-code-surface level, not a new
backend or new dependency — but "config/wiring change" describes the code
diff, not the runtime cost; see the per-utterance re-instantiation caveat
above and the model-distribution and platform risks below, none of which are
config/wiring-sized concerns.

**Known risks, explicitly designed around below**:

- **Native-library packaging**: `sherpa-onnx` has never been built into a
  shipped binary in this project — it is an optional dependency referenced
  only within `vox-speech`'s own `Cargo.toml`
  (`stt-sherpa = ["dep:sherpa-onnx"]`, `crates/vox-speech/Cargo.toml:36`), and
  that crate's Cargo.toml carries a TODO suggesting the opposite migration was
  once planned ("drop stt-candle after audio-ingress rewire/retirement",
  `Cargo.toml:21`). Unlike Candle (pure Rust, no native library), `sherpa-onnx`
  links against the ONNX Runtime native shared library, which the Tauri
  installer must bundle correctly on Windows, macOS, and Linux — untested for
  this project. This design treats "packages and runs on all three platforms"
  as an explicit gate before Parakeet becomes the shipped default, with
  Candle-Whisper retained as an automatic fallback.
- **Windows-specific CRT conflict, not currently in the gate**: the pinned
  `sherpa-onnx-sys` crate (v1.13.3) links ONNX Runtime statically by default
  using the MSVC static CRT (`/MT`), which is documented upstream
  (k2-fsa/sherpa-onnx discussion #1202, "Linking and search path") to conflict
  with the dynamic CRT (`/MD`) used by typical production Rust/MSVC builds —
  a Windows packaging failure mode distinct from, and more specific than, the
  general "packages and runs on all three platforms" smoke test above. The
  cross-platform packaging gate must explicitly verify the Windows build
  links cleanly (or pin a build configuration that avoids the `/MT`/`/MD`
  mismatch), not just that the app launches.
- **Model distribution/bundling is unaddressed**: `resolve_sherpa_model_paths`
  (and its Phase 2 extension for the joiner file) falls back to a live
  Hugging Face Hub API download whenever the model-directory env var isn't
  set, with no offline-only mode or progress callback
  (`crates/vox-speech/src/backends/sherpa_model_config.rs`), and
  `vox-gui/tauri.conf.json`'s bundle section has no `resources` entry to ship
  the model with the installer (only `externalBin` for the CLI binary).
  Combined with the per-utterance re-instantiation risk above, making sherpa
  the tried-first default means a fresh install's first dictation attempt has
  a live network dependency and, on a cold model cache, blocks synchronously
  on a multi-hundred-megabyte download with no progress UI, timeout, or
  documented offline failure mode. This design does not yet specify a
  bundling or pre-fetch strategy; one is needed before Phase 2 ships.

## Design: phased plan

### Phase 0 — Eval harness + dead-code removal (foundation)

- Add an automated test (`crates/vox-speech/tests/eval_regression.rs` or
  equivalent) that loads the three fixture manifests, runs them through the
  real pipeline, computes WER/CER via the existing `eval.rs` functions, and
  asserts against a checked-in baseline (fails on regression beyond a small
  tolerance).
  - **Open question to resolve during implementation**: confirm whether audio
    files exist alongside the JSONL manifests, or whether they are
    transcript-only. If no audio corpus exists, this phase includes sourcing
    or synthesizing minimal audio fixtures for at least the code-dictation
    manifest (`vox_code_corpus_v1.jsonl`), since that's the highest-value one
    to protect against regression.
- **Output**: a CI-visible accuracy signal that every later phase is graded
  against.
- (No dead-code removal in this phase — see the corrected finding #6 above;
  `oratio.rs`/`oratioVoiceInput.ts` are left untouched, tracked separately.)

### Phase 1 — Correction-rule bug fixes

- Remove or fix the no-op/broken rules in `rules.rs`: drop the `mut self`
  identity mapping; fix or drop `impl for` (trailing-space-only); fix
  `box dine` → `Box<dyn ` to also handle the closing `>` correctly (or scope
  it to a validated pattern with a following type token); remove the
  unvalidated `print len`/`print el in` phonetic guesses unless backed by real
  transcription samples showing they're needed. **These content fixes are
  necessary but not sufficient**: per audit finding #3, `code_confusion_map`'s
  keys are multi-word phrases, but the matching loop that consults the map
  only looks up single whitespace-split tokens, so none of these entries can
  fire regardless of their mapped value. Fixing the map's contents must be
  paired with fixing (or replacing) the matching logic to actually look up
  multi-word phrases against the transcript — otherwise the corrected entries
  stay unreachable dead code, same as today.
- Remove `"status"` and `"workflow"` from `default_domain_lexicon` (or scope
  the case-forcing rule to only apply when the word appears in a
  clearly-technical context, e.g. adjacent to other domain terms).
- Each fix gets a direct unit test reproducing the bug and asserting the fix.

### Phase 2 — Parakeet-via-sherpa-onnx as default backend

- Extend `sherpa_model_config.rs::resolve_sherpa_model_paths` to also resolve
  a `joiner.onnx` path (transducer models need encoder + decoder + joiner,
  vs. Whisper's encoder + decoder), and change the default HF model ID from
  `k2-fsa/sherpa-onnx-whisper-tiny.en` to the pre-built int8 Parakeet-TDT
  repo. **Open question to resolve during implementation**: pin the exact HF
  repo ID and filenames at implementation time (candidates found during
  research: `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`,
  `sherpa-onnx-nemo-parakeet-unified-en-0.6b-int8-non-streaming`) — verify
  which is actually published and maintained before wiring the default.
- Extend `sherpa_onnx.rs` to build `OfflineRecognizerConfig` with
  `model_config.transducer = OfflineTransducerModelConfig{..}` instead of
  `model_config.whisper`, selected by which model files are present (or by an
  explicit model-kind field).
- Flip `vox-gui/Cargo.toml:40` to enable `stt-sherpa` in addition to
  `stt-candle` (Candle stays compiled in as the fallback, not removed).
- `backend_dispatch.rs`: default backend selection tries sherpa+Parakeet
  first; on init failure (missing native runtime, unsupported platform,
  model download failure), fall back to Candle-Whisper and log the reason at
  `warn` level so it's diagnosable in the field.
- **Gate before this becomes the shipped default**: the Phase-0 eval harness
  must show Parakeet's WER/CER beating current Candle-`tiny.en` on all three
  fixture manifests, AND the Tauri build must package and run successfully on
  Windows (including verifying the `/MT`-vs-`/MD` CRT linking does not
  conflict), macOS, and Linux with the native ONNX Runtime dependency
  bundled, AND per-utterance latency under the existing no-caching
  `create_backend()` call pattern (model resolution + fresh
  `OfflineRecognizer` session construction on every dictation stop) must be
  measured and shown acceptable for real-time dictation, AND a model
  bundling/pre-fetch strategy (or a documented, user-visible offline/cold-cache
  behavior) must be in place so a fresh install does not silently block on an
  unbounded network download.

### Phase 3 — Code-dictation symbol expansion

- Remove the `compiler-rerank` feature gate from the reranking path that
  applies `speech_normalize`'s symbol/casing rules, either by making it a
  default feature of `vox-speech` or by folding the always-useful part of
  `pick_best_transcript_index_with_raw` into the unconditional path.
- Re-run the Phase-0 eval harness specifically against
  `vox_code_corpus_v1.jsonl`, now on top of the Phase-2 Parakeet baseline, to
  confirm the symbol-expansion layer still measurably helps once the base ASR
  is more accurate — don't assume it does, verify.

### Phase 4 — GUI Settings surface

- Add a small STT/voice section to `Settings/SettingsView.tsx` exposing only
  the knobs an end user would plausibly want, not all ~30 env vars:
  - ASR backend selection (Auto / Parakeet / Whisper)
  - Correction aggressiveness (the existing `Conservative` / `Balanced` /
    `Aggressive` `CorrectionContext` profiles)
  - Custom lexicon entries (simple add/remove list UI backed by the existing
    `speech_lexicon.rs` JSON schema)
- This phase is UX-only and depends on Phases 1-3 having settled sane
  defaults first — it exposes tuning, it doesn't fix correctness.

## Testing / verification strategy

- **Phase 0's eval harness is the acceptance gate for Phases 2 and 3**:
  WER/CER must not regress versus baseline, and must measurably improve for
  the backend swap.
- **Phase 1** rule fixes get one direct unit test per fixed rule, reproducing
  the specific bug found in the audit.
- **Phase 2**'s cross-platform packaging gate is verified by building and
  smoke-testing the Tauri app on all three target OSes before flipping the
  default — this cannot be verified by an automated browser preview (STT
  requires real microphone access and native library loading).
- **Phase 4** is UI-only and verified by hand in the running app for the same
  reason (mic permission, native audio capture).

## Out of scope

- The separate "automatic prompt-engineering enhancer" question (per-model
  prompt adaptation for Vox Mens / Qwen / Claude Sonnet / Opus) is a
  independent research track, not covered by this design. See the harness
  parity docs (`vox-harness-parity-plan-2026-07-30.md` and siblings) for
  related but distinct work on prompt/harness quality.
- Mobile/Capacitor STT paths — already removed per an existing ADR
  (desktop-only `vox-tauri-stt`).
- Full exposure of all ~30 `VOX_ORATIO_*` env vars in Settings — Phase 4
  scopes to the user-relevant subset only.
