---
title: "Architecture: ASR Speech-to-Code"
description: "Detailed system design for integrating high-fidelity speech-to-code dictation into the Vox MENS pipeline."
category: "Architecture SSOTs"
status: "research"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox Speech-to-Code Architecture Research — April 2026
<!-- SSOT: docs/src/architecture/asr-speech-to-code-architecture-2026.md -->
<!-- Related: asr-speech-to-code-findings-2026.md (initial scouting), telemetry-trust-ssot.md -->
<!-- DO NOT delete: linked from architecture-index.md -->

## Purpose

This document synthesizes 25+ targeted web searches conducted in April 2026 to determine the optimal, highest-accuracy architecture for feeding spoken audio into Vox's MENS model pipeline. It considers three strategic pillars:

1. **Best-off-the-shelf ASR** — transcribe speech at the lowest WER and feed text straight into MENS.
2. **Code-domain–adapted ASR** — fine-tune an existing model (LoRA/QLoRA) for Rust/TypeScript vocabulary.
3. **Custom speech-to-code** — train or integrate a model purpose-built for dictating identifiers, symbols, and code structure.

The RTX 4080 Super (16 GB VRAM) is the target inference GPU. The Rust/Candle + ONNX/sherpa-onnx ecosystem is the preferred deployment surface, consistent with Vox's existing Burn-based MENS pipeline. Python is acceptable for the **training** phase only.

---

## 1. Baseline WER Landscape (April 2026)

All WER numbers are on standard English benchmark suites (LibriSpeech test-clean / test-other / OpenASR leaderboard composite). Code-domain WER will be **higher**; see Section 4 for the delta.

| Model | Params | WER (En avg) | RTFx (A100) | VRAM | Streaming | Notes |
|---|---|---|---|---|---|---|
| **Cohere Transcribe** | — | **5.42%** | 524× | API-only | No | Top API, closed |
| **Canary-Qwen 2.5B** (NVIDIA) | 2.5 B | **5.63%** | ~418× | ~10 GB | No (batch) | SALM; FastConformer + Qwen decoder |
| **Qwen3-ASR-1.7B** (Alibaba) | 1.7 B | **~5.7%** | RTF 0.015–0.13 | ~8 GB | Yes (unified) | AuT encoder + Qwen3 decoder |
| **IBM Granite Speech 3.3 8B** | 8 B | **5.85%** | — | ~16 GB | No | Fits 4080S just; enterprise |
| **Deepgram Nova-3** | — | **5.26%** | — | API-only | Yes | Best API; domain variants |
| **Whisper Large-v3** | 1.54 B | **6.8%** | ~180× | ~10 GB | No | 99+ languages; batch |
| **Whisper Large-v3-Turbo** | ~809 M | ~7.0–7.2% | ~6× large-v3 | ~6 GB | No | 4-decoder-layer distillation |
| **Distil-Whisper large-v3** | ~756 M | ~7.1–7.5% | ~6× base | ~5 GB | No | 2-decoder-layer distillation |
| **Faster-Whisper (CTranslate2)** | same | same | 2–4× over OpenAI | −40% VRAM | No | Inference engine, not model |
| **NVIDIA Parakeet-TDT 1.1B** | 1.1 B | ~5.8% | **>2 000×** | ~6 GB | Yes (native) | FastConformer + TDT decoder |
| **Moonshine Medium** | ~330 M | ~7–8% | 40×+ vs Lv3 | ~2 GB | Yes (native) | RoPE; TTFT <150 ms |
| **Vosk** | ~50 MB | ~12–18% | fastest CPU | <1 GB | Yes | Extreme edge; low accuracy |

> **Key insight:** Parakeet-TDT offers near–Canary accuracy at >2 000× RTFx in a fully streaming mode. Canary-Qwen and Qwen3-ASR-1.7B are the top-tier LLM-decoder hybrids for max accuracy but require batch or chunked inference rather than true sub-utterance streaming.

archived_date: 2026-04-18
---

## 2. Architecture Concepts for Quality Maximization

### 2.1 Why Decoder Architecture Determines Code WER

| Decoder | Context | Why matters for code |
|---|---|---|
| **CTC** | None (label independence assumed) | Collapses repeated frames but cannot correct *which* token is most likely given adjacent tokens — identifier homonyms explode WER. |
| **Transducer (RNN-T / TDT)** | Prediction network ≈ internal LM | Can model `getItem` vs `get_item` if the vocabulary is seeded correctly. Native streaming. |
| **Attention Encoder-Decoder (AED)** | Global (full utterance) | Best correction but requires full audio. Whisper and Canary-Qwen use this. |
| **SALM (AED + LLM decoder)** | Full audio + LLM world knowledge | LLM decoder already knows Rust/TS syntax. Can produce `unwrap_or_else` naturally. **Best for code.** |

### 2.2 The Preprocessing Stack (and What to Skip)

Research confirms a counter-intuitive finding: **aggressive conventional noise filtering *hurts* modern neural ASR** because it removes formant transitions used by the encoder. The optimal input pipeline is:

```
[Mic / WAV] 
  → Resample to 16 kHz mono
  → RMS loudness normalization (target ~−18 dBFS)
  → Silero-VAD (ONNX; 512-sample = 32 ms chunks @ 16 kHz)
     ↳ discard silence  →  prevents Whisper hallucinations
  → Buffer speech segments
  → Log-Mel spectrogram (80 or 128 channels, 25 ms window, 10 ms stride)
  → Feed to ASR model
```

**Do NOT apply:** Wiener filtering, spectral subtraction, or heavy noise gate before the ASR encoder. Use a noise-*trained* model instead (Canary, Qwen3-ASR, etc.).

### 2.3 Chunk Sizing and Latency Budget

For a code dictation scenario the latency budget is generous (developer is speaking intent, not reacting to sound). Recommended:

| Stage | Chunk size | Expected latency |
|---|---|---|
| VAD (Silero) | 32 ms | <1 ms per chunk on CPU |
| Streaming fast-path (Moonshine/Parakeet) | 160–320 ms | TTFT ~150–300 ms |
| Accuracy batch pass (Canary/Qwen3-ASR) | Full utterance (on silence/endpointing) | 200–800 ms |
| LLM post-correction (Qwen3-0.6B) | Per sentence | ~100–250 ms on 4080S |

**Two-pass streaming:** deliver a Parakeet-TDT or Moonshine transcript immediately for typing echo, then replace with Canary/Qwen3-ASR output once silence is detected. The MENS model always receives the high-accuracy batch-pass output.

---

## 3. Recommended Rust Architecture

### 3.1 Crates and Runtime Boundaries

```
audio input (cpal or rodio)
     │
     ▼
vox-voice  ─── owns all ASR logic
  ├── silero_vad_rs  (stateful VAD per stream, ONNX/ort)
  ├── asr_backend  (trait: transcribe_segment(audio) → TranscriptResult)
  │     ├── WhisperBackend   (candle-based; fastest to ship)
  │     ├── CanaryBackend    (sherpa-onnx or ort; ONNX export from NeMo)
  │     └── Qwen3AsrBackend  (sherpa-onnx; official ONNX release)
  ├── post_processor::CodeCorrector  (Qwen3-0.6B ONNX / ort)
  ├── context_biaser  (prefix tree / TCPGen hotword injection)
  └── transcript_sink  → MENS input channel (async tokio mpsc)
```

**Trait design (SSOT for all backends):**

```rust
/// vox-voice/src/asr_backend.rs
#[async_trait::async_trait]
pub trait AsrBackend: Send + Sync {
    async fn transcribe(&self, pcm: &[f32]) -> anyhow::Result<TranscriptResult>;
    fn name(&self) -> &'static str;
    fn supports_streaming(&self) -> bool { false }
}

pub struct TranscriptResult {
    pub text: String,
    pub confidence: f32,       // 0.0–1.0; from log-prob
    pub n_best: Vec<String>,   // top-K hypotheses for LLM rescoring
    pub word_timestamps: Vec<(String, f32, f32)>,
}
```

This pattern means **adding Canary** is simply implementing `AsrBackend` on a new struct that wraps the `sherpa-onnx` or `ort` session. No changes to the MENS pipeline.

### 3.2 ONNX vs Candle: When to Use Each

| Criterion | Candle | ONNX Runtime (`ort`) |
|---|---|---|
| Pure-Rust, no native libs | ✅ | ❌ (needs shared .dll/.so) |
| TensorRT execution provider | ❌ | ✅ |
| FastConformer (Canary encoder) | Needs hand-implementation | ✅ via NeMo ONNX export |
| Whisper | ✅ (existing impl) | ✅ via faster-whisper export |
| INT8 / FP16 quantization | Partial | ✅ full support |
| Streaming-stateful (RNN-T) | Hard | ✅ via sherpa-onnx |

**Practical decision tree:**
- Ship Whisper immediately via Candle (already supported in the Vox ML ecosystem, aligns with `vox-tensor`/Burn patterns).
- Integrate Canary / Qwen3-ASR via `sherpa-rs` + ONNX Runtime. NeMo supports `model.export("model.onnx")` natively.
- Use TensorRT EP on RTX 4080 Super for production throughput; FP16 by default, INT8 only if profiling shows VRAM pressure.

### 3.3 Silero-VAD in Rust (Concrete)

```rust
// Cargo.toml
[dependencies]
silero-vad-rs = "0.3"
ort = { version = "1.17", features = ["cuda"] }

// Usage
let model = SileroVAD::new("models/silero_vad.onnx")?;
let mut vad = VADIterator::new(model, 0.5, 16_000, 100, 30);
// In audio capture loop:
loop {
    let chunk: Vec<f32> = mic.read_512_samples()?; // 32 ms @ 16 kHz
    if let Some(speech_event) = vad.process_chunk(&chunk)? {
        // queue chunk into speech_buffer
    }
}
```

Cost: <1 ms per 32 ms chunk on CPU. Zero GPU required for VAD stage.

archived_date: 2026-04-18
---

## 4. Code-Domain WER: Baseline vs. Adapted

This is the critical question. Synthesized estimates from 2025 domain adaptation studies:

| Scenario | Est. WER (English prose) | Est. WER (Rust code identifiers) | Notes |
|---|---|---|---|
| Whisper Large-v3 (raw) | 6.8% | **25–40%** | Catastrophic on snake_case, macros |
| Whisper-Turbo (raw) | 7.2% | **28–42%** | Similar; slightly worse |
| Canary-Qwen (raw) | 5.6% | **18–28%** | LLM decoder helps significantly |
| Qwen3-ASR-1.7B (raw) | ~5.7% | **15–25%** | Qwen3 base knows code |
| Whisper Large-v3 + LoRA (code corpus) | ~7% | **8–14%** | LoRA on decoder only; 10–20% relative gain |
| Canary-Qwen + code hotword biasing | ~5.6% | **10–18%** | Hotword prefix tree biasing |
| Qwen3-ASR-1.7B fully adapted | — | **6–10% (estimated)** | Best realistic target |
| + MENS Qwen3-0.6B post-correction | — | **4–8% (estimated)** | LLM corrector uses surrounding code context |

> **Estimated achievable WER for Vox speech-to-code (~4–8%):** This assumes (a) Qwen3-ASR-1.7B as the backbone, (b) runtime hotword biasing injecting identifiers declared in the current open file, and (c) a Qwen3-0.6B post-correction pass fine-tuned on (ASR-output, corrected-code) pairs from the Vox corpus.

**Why WER on code is so high without adaptation:**
- `unwrap_or_else` sounds like "unwrap or else" → 3 words vs 1
- `snake_case` case-folding by default destroys identifiers
- Library names (`tokio`, `anyhow`, `serde`) lack pronunciation priors
- Punctuation (`::`, `->`, `?`) is completely ignored by standard ASR
- Rust keywords (`impl`, `pub(crate)`, `dyn`) have rare phonetic patterns

---

## 5. Fine-Tuning / Training Pathway

### 5.1 LoRA Adapter on Whisper or Qwen3-ASR

**Language:** Python (training); Rust (deployment inference only).

```
1. Generate synthetic audio corpus (Piper TTS, local + free):
   - Read Vox codebase Rust files as "spoken text"
   - Normalize: "pub fn" → "pub fn" (preserve case for decoder)
   - Add speed perturbation ±10%, room-impulse-response augmentation
   - Target: ~50–100 h synthetic + any real developer voice recordings

2. HuggingFace PEFT LoRA config:
   model = WhisperForConditionalGeneration.from_pretrained("openai/whisper-large-v3")
   lora_config = LoraConfig(r=32, lora_alpha=64, 
                             target_modules=["q_proj","v_proj"],
                             lora_dropout=0.05)
   model = get_peft_model(model, lora_config)
   # Train decoder-only; freeze encoder entirely

3. Evaluate on holdout Vox dictation sessions:
   - Metric: per-identifier WER (strict, no normalization of case)
   - Also: syntactic validity rate (does rustfmt accept the output?)

4. Export: merge LoRA weights → .safetensors → convert to ONNX/CTranslate2
```

### 5.2 Domain Adapter for Qwen3-ASR (Preferred Path)

Qwen3-ASR-1.7B has a dual-module architecture: AuT audio encoder (~300 M params) + Qwen3-1.7B LLM decoder. The LLM decoder **already understands Rust syntax** from pretraining. This makes the adaptation *much cheaper*:
- Fine-tune **only the LLM decoder** with LoRA using text-only code correction data (ASR output → correct code) — no audio needed.
- Train on a corpus of (Whisper-misrecognition, correct Vox code) pairs.
- RTX 4080 Super (16 GB) can comfortably run 4-bit QLoRA on 1.7B decoder.

### 5.3 Integration with MENS Training Pipeline

Since Vox already uses Burn + QLoRA for MENS domain adapters:

```
MENS Training Pipeline (existing)
  └── Corpus: Rust source, Markdown, Synthetic
  └── Domain adapters: vox-lang, rust-expert, agents

NEW: asr-voice-adapter domain
  └── Corpus: (spoken-command-audio, code-text) pairs
       ├── Source A: Piper-synthesized Vox files
       ├── Source B: Developer session recordings (opt-in telemetry)
       └── Source C: Zero-shot Qwen3 text correction pairs
  └── Model: Qwen3-ASR-1.7B decoder LoRA (merged at inference)
  └── Evaluation: dictation WER on Vox codebase holdout
```

The ASR domain adapter lives in `crates/vox-populi/src/domains/asr_voice/` and is selected by `vox populi train --domain asr-voice`.

archived_date: 2026-04-18
---

## 6. Hotword / Context Biasing at Runtime

The single biggest practical gain in code-domain ASR is **injecting context from the open file** at inference time. Two techniques:

### 6.1 Shallow Fusion (n-gram)

Build a unigram/bigram language model from the symbols declared in the current open file (variables, function names, types). Merge its log-probability scores with the ASR beam search at decoding time.

- Works with Whisper via `faster-whisper`'s `initial_prompt` or via custom CTC/Beam hook.
- Trivially extractable from `rust-analyzer` LSP symbol table.
- Cost: negligible.

### 6.2 Tree-Constrained Pointer Generator (TCPGen)

An auxiliary neural module that maintains a prefix tree of the hotword list and dynamically adjusts token probabilities during attention-based decoding. Reported 15–30% relative WER improvement on rare-term benchmarks.

- Requires mild model surgery; more applicable to Canary than Whisper.
- Can be implemented as a second inference head; ONNX-exportable.

**Recommended practical approach for Vox v1:**

```rust
// vox-voice/src/context_biaser.rs
pub struct ContextBiaser {
    /// Symbols from rust-analyzer LSP hover/symbols response
    symbols: Vec<String>,
    boost_score: f32, // typically 1.5–2.5 log-prob bonus
}
impl ContextBiaser {
    pub fn build_initial_prompt(&self) -> String {
        // For Whisper: prepend symbol list as text prompt
        // Guides decoder attention toward known identifiers
        self.symbols.join(" ")
    }
}
```

---

## 7. Post-Processing Stack (LLM Correction)

### 7.1 Pipeline

```
ASR Raw Output (Qwen3-ASR or Whisper)
     │
     ▼
[1] Punctuation & Capitalization Restorer
     → Qwen3-0.6B LoRA fine-tuned on code-ASR pairs
     → Adds :: . () {} ; ? at correct positions
     │
     ▼
[2] Identifier Normalizer
     → Regex + LSP cross-reference: "get item" → getItem / get_item
     → Heuristic: if camelCase match exists in symbol table → prefer
     │
     ▼
[3] Code Validator (optional)
     → rustfmt --check / tsc --noEmit on buffer substring
     → Flag low-confidence segments if invalid parse
     │
     ▼
[4] MENS Input Channel
     → Passes structured TranscriptResult to MENS orchestrator
     → Includes n_best list, word timestamps, confidence score
```

**Hallucination guard:** The Qwen3-0.6B corrector must only modify tokens from the ASR n-best hypotheses list. If it tries to generate tokens not in any hypothesis, revert to the top-1 ASR output. This prevents over-correction.

### 7.2 Metrics Beyond WER

For code dictation, WER is insufficient. Track:

| Metric | Definition | Target |
|---|---|---|
| **Identifier Accuracy Rate (IAR)** | % identifiers transcribed exactly correct | >85% |
| **Syntactic Validity Rate (SVR)** | % utterances that `rustfmt`-parse cleanly | >70% |
| **Symbol Match Rate (SMR)** | % output tokens that match active LSP symbol table | >78% |
| **TTFT (streaming)** | Time to first readable token | <300 ms |
| **End-of-Utterance Latency (EUL)** | Total latency to final corrected text | <1 500 ms |

archived_date: 2026-04-18
---

## 8. Strategic Options Summary

Three viable architectures, ordered by investment:

### Option A — Whisper + Candle + QLoRA Adapter (Lowest Effort)

**WER estimate:** 8–14% on code identifiers

- Use existing `candle-whisper` bindings in the Vox ML ecosystem.
- Add Silero-VAD crate for speech segmentation.
- Train QLoRA adapter on Piper-synthesized Vox codebase audio.
- Add `initial_prompt` context biasing from open file symbols.
- Pass output to MENS with a lightweight Qwen3-0.6B text correction.
- All Rust at inference time (Candle + ort).

**Time to ship:** 2–4 weeks

### Option B — Qwen3-ASR-1.7B + sherpa-rs/ONNX + Full Stack (Recommended)

**WER estimate:** 4–8% on code identifiers

- Export Qwen3-ASR-1.7B to ONNX via official Qwen toolchains.
- Integrate via `sherpa-rs` crate with CUDA EP on RTX 4080 Super.
- Fine-tune LLM decoder via text-only LoRA (no audio needed for adaptation).
- Deploy two-pass streaming: Parakeet-TDT for UI echo (2 000× RTF), Qwen3-ASR for final MENS input.
- Full post-processing stack (Section 7).

**Time to ship:** 4–8 weeks

### Option C — Custom Speech-to-Code Model (Highest Accuracy, Highest Effort)

**WER estimate:** 2–5% on code identifiers (theoretically)

- Train a purpose-built model: FastConformer encoder + code LLM decoder (e.g., Qwen3-Coder).
- Train with NeMo on a dataset of developer sessions (real audio) + Piper synthetic.
- Requires 200–500 h of gpu-training time on RTX 4080 Super or rented cloud GPU (Vast.ai A100).
- Enables Vox-MENS to receive ASR embeddings *directly* rather than text, bypassing the text bottleneck.
- Eventually: a single model that accepts audio → produces Vox language AST directly.

**Time to ship:** 3–6 months

---

## 9. Integration Points with Existing Vox Codebase

| Where | What changes |
|---|---|
| `crates/vox-populi/src/domains/` | Add `asr_voice` domain with QLoRA recipe |
| `crates/vox-voice/` | **New crate** — owns VAD, ASR backends, post-processor |
| `crates/vox-cli/src/commands/` | Add `vox voice start` / `vox voice calibrate` / `vox voice status` |
| `crates/vox-clavis/src/lib.rs` | No new secrets if fully local; add `VOX_DEEPGRAM_API_KEY` only for optional cloud fallback |
| `contracts/operations/` | Add `voice-retention.v1.yaml` for audio session retention policy |
| `docs/src/reference/cli.md` | Document `vox voice` subsystem |
| `crates/vox-db/` | Schema addition: `voice_sessions` table (audio hash, WER estimate, correction log) |

archived_date: 2026-04-18
---

## 10. Recommended Immediate Action

Based on all research, the recommended path for 2026 is:

1. **Ship Option A (Whisper/Candle) as v0** — to get something working and build the evaluation harness.
2. **Collect real dictation data** — developer voice sessions with opt-in recording, stored per `workspace-artifact-retention.v1.yaml`.
3. **Fine-tune Qwen3-ASR-1.7B on code corpus** (Option B decoder LoRA) — takes ~1–2 GPU-days on the 4080 Super.
4. **Instrument WER tracking in `vox-db`** — every dictation session logs estimated identifier error rate.
5. **Plan Option C** as a 2026 H2 stretch goal once Option B ships and data volume justifies custom training.

---

*Sources: Hugging Face Open ASR Leaderboard (April 2026), NVIDIA NeMo docs, Qwen3-ASR tech report (arXiv:2601.21337), sherpa-onnx / sherpa-rs crates.io, silero-vad-rs docs.rs, WER domain-adaptation studies (INTERSPEECH 2024–2025), and 25 targeted web searches conducted April 2026.*

