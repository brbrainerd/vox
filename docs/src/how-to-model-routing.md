# Model Routing & Provider Cascade

Vox uses a **three-layer provider cascade** to give you the best AI models available, defaulting to completely free access with no credit card required.

## Provider Cascade

```
┌─────────────────────────────────────────────────┐
│              Model Selection                     │
├─────────────────────────────────────────────────┤
│  Layer 1: Google AI Studio (direct)             │
│  ├── gemini-2.0-flash-lite   (fast, free)       │
│  ├── gemini-2.5-flash-preview (balanced, free)  │
│  └── gemini-2.5-pro          (best, free)       │
│                                                  │
│  Layer 2: OpenRouter (requires free API key)     │
│  ├── mistral/devstral-2-2512:free               │
│  ├── qwen/qwen3-coder:free                     │
│  ├── meta-llama/llama-4-scout:free              │
│  └── moonshotai/kimi-k2:free                   │
│                                                  │
│  Layer 3: OpenRouter Paid (premium)              │
│  ├── deepseek/deepseek-v3.2  ($0.27/M tokens)  │
│  ├── anthropic/claude-sonnet-4.5                │
│  ├── openai/gpt-5                              │
│  └── openai/o3                                 │
│                                                  │
│  Layer 0: Ollama (always available, zero-auth)   │
│  └── any locally pulled model                   │
└─────────────────────────────────────────────────┘
```

## How Model Selection Works

### `vox chat` (CLI)

1. Check for Google AI Studio key → use Gemini models directly
2. Check for OpenRouter key → use `:free` models, escalate to paid if needed
3. Check for Ollama → fall back to local inference
4. No keys at all → prompt user to set up (10 seconds, free)

### `vox-orchestrator` (Multi-Agent)

The `ModelRegistry` in `vox-orchestrator/src/models.rs` routes based on:

- **Task type:** CodeGen → Gemini Flash / Claude Sonnet 4.5; Debugging → O3; Research → GPT-5
- **Complexity:** Low complexity (≤3) → cheapest model; High → best-for-task
- **Cost preference:** Economy prefers free/cheap models; Performance selects premium

```rust
let registry = ModelRegistry::new();

// Best free model for coding tasks
let model = registry.best_free_for(TaskCategory::CodeGen);

// Best model regardless of cost
let model = registry.best_for(TaskCategory::Debugging, 8, CostPreference::Performance);
```

## Escalation Chain

If a model fails (rate limit, error), `vox chat` escalates automatically:

| Provider | Attempt 1 | Attempt 2 | Attempt 3+ |
|---|---|---|---|
| Google | gemini-2.0-flash-lite | gemini-2.5-flash-preview | gemini-2.5-pro |
| OpenRouter | default model | next-cheapest with same strength | most-capable free model |

## Key Management

Keys are managed via the unified `vox login` system:

```bash
vox login --registry google YOUR_KEY      # Google AI Studio
vox login --registry openrouter YOUR_KEY  # OpenRouter

# Keys stored in ~/.vox/auth.json
# Also reads from env vars: GEMINI_API_KEY, OPENROUTER_API_KEY
```

## Cost Tracking

When using paid models, Vox tracks costs in VoxDB:

```bash
vox db stats   # See total spend, per-model breakdown
```

Cost data is stored as memory entries and can be queried via `VoxDb::recall_memory()`.
