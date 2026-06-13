## vox-actor-runtime — Semantic Behavior Map

**Summary.** From 145 extracted `Behavior` claims (all `EXTRACTED` confidence), this map dedups to ~27 distinct symbols across 14 files. Coverage is strongest in scheduling (`durable_scheduler.rs`), LLM result parsing (`llm_result.rs`), activity retry (`activity.rs`), and auth (`auth.rs`), each of which carries explicit error and/or invariant proofs. Coverage thins sharply on the runtime's stateful and security-adjacent surfaces — rate limiting, the process registry, the retrieval context budget, the provider router, and the network-fed JSON parsers — which are proven only on the happy/matching path despite having obvious failure, empty, or conflict modes.

---

### activity.rs
- **`ActivityOptions` (builder)** — `with_retries/with_timeout_secs/with_initial_backoff/with_activity_id` set their fields. *Happy only.*
- **`ActivityOptions::parse_duration`** — parses `s/ms/m/h` and bare numbers; returns `None` on invalid input. *Has error path.*
- **`execute_activity`** — retries then succeeds; exhausts retries → `ActivityResult::Failed(RetriesExhausted)`; makes exactly `1 + retries` attempts. *Happy + error + invariant.*
- **`ActivityError::RetriesExhausted`** — attempts = `1 + retries`, carries last error. *Invariant.*

### auth.rs
- **`parse_bearer_token`** — extracts token from `Bearer <token>`; `None` when prefix missing or input `None`. *Happy + edge/error.*
- **`authorize_request`** — true on api_key match / bearer match; false on either mismatch. *Happy + error.*

### durable_scheduler.rs (best-covered module)
- **`ScheduleSpec::parse`** — `@hourly/@daily/@weekly` → variants; 5-field cron → `Cron`; `None` for garbage and incomplete (`0 0 0`) exprs. *Happy + error.*
- **`startup_fires`** — `Skip`→0, `RunNow`→1 (invariant across missed=1/99), `CatchUp`→missed count. *Happy + invariant.*
- **`missed_buckets_since`** — `(now-last)/period`; 0 when last_run is in the future. *Happy + edge.*
- **`ScheduleSpec::next_delay_from`** — canonical periods (Hourly=3600s, Daily=86400s). *Happy.*

### llm_result.rs
- **`LlmResult::parse_from`** — deserializes valid JSON (string + numeric fields); strips ```json fences and parses inner JSON. *Happy (multiple inputs), no malformed-JSON rejection proof.*
- **`LlmResult::unwrap_or_default`** — returns type default on `Err`. *Edge.*
- **`LlmResult::map`** — transforms `Ok`, preserves variant. *Happy.*
- **`maybe_strip_markdown_json_fences`** — strips bare and `json`-tagged fences, tolerates whitespace, idempotent on non-fenced. *Happy + idempotence invariant.*
- **`LlmError` (Display)** — ParseError/ApiError include message; ActivityFailed contains "activity failed". *Happy.*

### mailbox.rs
- **`MessagePayload` clone** — zero-copy shared buffer (Bytes/Arc). *Invariant.*
- **`MessagePayload::json_value` / `deserialize_json`** — construct from `serde_json::Value`, round-trip with fields preserved. *Happy.*
- **wire format** — preserves `event` field and `args` array (positional deserialize) across roundtrip. *Happy.*

### scheduler.rs
- **`ProcessHandle::send`** — delivers payload text to spawned actor. *Happy.*
- **`ProcessHandle::call`** — receives correctly-formatted reply. *Happy.*

### state_machine.rs
- **`ReactiveStateMachine::state`** — returns initial state. *Happy.*
- **`ReactiveStateMachine::send`** — applies reducer, updates and returns new state. *Happy.*
- **clone** — clones share state via Arc; changes visible across clones. *Invariant.*

### rate_limit.rs
- **`RateLimiter::allow`** — true for first two calls, false on third (same key); true under env-default config. *Happy/deny-edge only.*

### registry.rs
- **`ProcessRegistry::lookup`** — `Some` for registered Pid; name re-register replaces prior Pid (old→`None`, new→`Some`, size stays 1). *Happy only.*

### resilient_http.rs
- **`ResilientHttpClient::backoff_duration`** — exponential 50/100/200ms for attempts 1/2/3. *Happy.*

### retrieval.rs
- **`apply_context_budget`** — truncates to char budget (`abcdef`→`abc`), keeps chunk count, sets provenance `truncated`. *Happy only.*

### model_resolution.rs
- **`resolve_chat_provider_route`** — returns `ManualOpenAiCompatible` when `manual_base_url` set, preserving base_url and model. *Happy only.*

### inference_env.rs
- **`parse_ollama_tags_models`** — extracts `name` from each model object → `Vec<String>`. *Happy only.*
- **`parse_hf_hub_models_array`** — parses array, maps `modelId`→`id`, preserves `downloads`/`pipeline_tag`. *Happy only.*

### feedback.rs
- **`FeedbackCollector::log / thumbs_up / get_training_data`** — log returns positive interaction_id, thumbs_up returns positive feedback_id, get_training_data round-trips prompt/response. *Happy + round-trip invariant only.*

### llm/cascade.rs
- **`cascade_for_research_stage`** — Planner + default input yields a candidate list including provider `ollama`. *Happy only.*

---

## Semantic gaps

Symbols proven **only** on the happy/matching path whose contract clearly has a failure, empty, or conflict mode. Ordered by actionability:

1. **`RateLimiter::allow` (security throttle).** Proven to deny after a threshold, but never proven to (a) isolate counts per key, (b) reset/refill after a window, or (c) recover. A rate limiter tested only on a single key's deny edge can pass while silently leaking cross-key state or never refilling.
2. **`ProcessRegistry::lookup`/register (integrity).** Name-replace is proven, but there is no unregister path, no lookup-miss on a never-registered Pid, and no concurrent-registration conflict test. Mutator with no failure/conflict proof.
3. **`apply_context_budget` (integrity/provenance).** Single-string truncation proven; zero/negative budget, already-fits no-op, and multi-chunk eviction order are untested. Provenance `truncated` flag is only ever observed true.
4. **`resolve_chat_provider_route` (router/validator).** Only the `manual_base_url` "manual wins" branch is proven. The precedence/fallback logic when manual is absent, and conflict resolution among competing providers, are unproven — the routing decision this function exists to make is essentially untested.
5. **`parse_ollama_tags_models` / `parse_hf_hub_models_array` (network-fed parsers).** Only well-formed JSON is proven. No missing-field, empty-array, or malformed-JSON rejection path — these consume untrusted remote responses.
6. **`FeedbackCollector` log/thumbs_up/get_training_data (mutator + store).** Only the happy round-trip is proven. No empty-store `get_training_data`, no `thumbs_up` on an unknown interaction_id, no persistence-failure handling.
7. **`ResilientHttpClient::backoff_duration`.** Exponential growth proven but no cap/ceiling or overflow at high attempt counts — the "resilient" guarantee (bounded backoff) is unverified.
8. **`ProcessHandle::send`/`call`.** Delivery and echo proven, but dead-actor / closed-mailbox / call-timeout failure paths are absent.
9. **`ReactiveStateMachine::send`.** Reducer application proven; reducer panic/error propagation and concurrent-send ordering under Arc-shared state are unproven.
10. **`cascade_for_research_stage`.** Only membership ("includes ollama") for one stage is proven — no ordering, exclusion, or empty-cascade-when-nothing-qualifies behavior.
