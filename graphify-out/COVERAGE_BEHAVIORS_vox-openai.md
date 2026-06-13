# Semantic Behavior Map — `vox-openai`

## Summary

This map synthesizes 9 EXTRACTED behavior claims over 7 symbols spanning two concerns: SSE streaming assembly (`crates/vox-openai/src/sse.rs`) and chat-completion wire types (`crates/vox-openai/tests/*`). No claims were dropped as duplicates — the paired `ChatCompletionRequest` and `ChatCompletionResponse` claims describe distinct facets (multipart content vs. base key set; tool_calls vs. minimal body). Every single claim is happy-path (`kind: "happy"`). Only `sse_data_line_delta()` carries any negative-case proof. The remaining six symbols are exercised exclusively on well-formed, complete input, which is significant because three of them (`Utf8LineBuffer`, `chat_completion_delta_content`, the `usage` decoder) have explicit None/empty/lossy-fallback branches in their contracts that no test reaches.

## Per-symbol behaviors

### `Utf8LineBuffer` (`src/sse.rs`)
- **Proven (happy):** Accumulates bytes across successive `push_lossy_bytes` calls and emits complete lines at `\n` boundaries, stripping trailing `\r`. A line begun in one chunk and finished in the next is emitted once, intact (`lines_split_across_chunks`).
- **Error path:** none.
- **Edge/invariant:** none. The split tested is ASCII at a byte boundary — the multibyte-codepoint-split case (the module's stated purpose, "lossy UTF-8 assembly") is unproven. `flush_trailing()` has zero coverage.

### `chat_completion_delta_content()` (`src/sse.rs`)
- **Proven (happy):** Extracts `choices[0].delta.content` as a `String` from well-formed OpenAI streaming JSON (`delta_extracts_content`).
- **Error path:** none. Contract returns `None` on parse failure, empty `choices`, or absent `content` — untested.
- **Edge/invariant:** none.

### `sse_data_line_delta()` (`src/sse.rs`)
- **Proven (happy):** Extracts non-empty delta content from `data: {json}` lines (`sse_line_done_and_data_prefix`).
- **Proven (negative):** Returns `None` for empty lines, non-`data:`-prefixed lines (e.g. `event: ping`), and the `[DONE]` sentinel. This is the only symbol in the crate with rejection coverage.
- **Edge/invariant:** partial. The `.filter(|s| !s.is_empty())` branch — a `data:` line whose JSON yields empty content — is not exercised.

### `ChatCompletionRequest` (`tests/`)
- **Proven (happy):** Serializes with `model`, `stream`, `max_tokens`, `messages` keys (`chat_completion_request_serializes_expected_keys`); serializes multipart `content` as an array of typed parts with `text`/`image_url` fields (`chat_completion_request_serializes_multipart_with_image_url`).
- **Error path:** none (serialization).
- **Edge/invariant:** none — no omitted-optional-field or round-trip stability proof.

### `ChatCompletionResponse` (`tests/`)
- **Proven (happy):** Deserializes a minimal success body (choices array, message content, usage) (`chat_completion_response_deserializes_minimal_success_body`); deserializes `tool_calls` with `function.name` and `function.arguments` (`chat_completion_response_parses_tool_calls_and_usage_extras`).
- **Error path:** none — no missing-required-field or malformed-payload proof.
- **Edge/invariant:** none.

### `ChatCompletionResponse.usage` (`tests/`)
- **Proven (happy):** Deserializes optional `cost`, `total_cost`, and cache token fields when present (`chat_completion_response_parses_tool_calls_and_usage_extras`).
- **Error path:** none.
- **Edge/invariant:** none — the absent-optional-field (None/default) path is unproven despite the fields being optional.

## Semantic gaps

These symbols are proven **only** on the happy path yet have contracts with explicit failure/empty/lossy modes. Most actionable first:

1. **`Utf8LineBuffer` UTF-8 split — `push_lossy_bytes()`.** The module exists specifically to do *lossy UTF-8 assembly across arbitrary chunk boundaries*, but the only split test uses ASCII. A multibyte codepoint (e.g. an emoji or accented char) split across two `push_lossy_bytes` calls is the core risk and is completely unverified. This is the crate's reason for not using `eventsource-stream`; it should have a dedicated test.
2. **`Utf8LineBuffer::flush_trailing()` — no coverage at all.** Both branches (emit a non-empty trailing fragment that lacked a `\n`; no-op + clear on empty tail) are unproven. A stream that ends without a final newline silently relies on untested code.
3. **`chat_completion_delta_content()` — malformed/missing-field None paths.** Three distinct `None` branches (JSON parse error, empty `choices`, missing `delta.content`) are unexercised. Real provider streams (OpenRouter/HF framing differences) make these likely in production.
4. **`sse_data_line_delta()` empty-content filter.** The `.filter(|s| !s.is_empty())` drop is the one gap in an otherwise well-covered symbol — a `data:` frame carrying empty `delta.content` should yield `None`.
5. **`ChatCompletionResponse` malformed-body decoding.** A deserializer that only sees well-formed bodies has no proof of its behavior on missing required fields or malformed `tool_call.arguments` — the realistic failure surface for a wire-protocol type.
6. **`ChatCompletionResponse.usage` absent-optional path.** Optional cost/cache fields are proven present but never proven absent; a response omitting them is the common case and is untested.
7. **`ChatCompletionRequest` round-trip / omitted-optional invariant.** Serialization shape is asserted, but there is no proof that optional fields are omitted (not emitted as null) or that a request survives a serialize→deserialize round trip.