# Semantic Behavior Map — `vox-plugin-webhook`

Deterministically synthesized from 21 distinct proven-behavior claims (of 21 extracted) across 7 symbols. 3 symbols have an explicit error-path proof; **3 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `WebhookHandler::handle`  (error, happy; EXTRACTED)
- [happy] successfully processes valid payloads and returns an event with the correct source field  (crates/vox-plugin-webhook/src/webhook/handler.rs)
- [happy] successfully processes valid payloads and returns an event with the correct event_type field  (crates/vox-plugin-webhook/src/webhook/handler.rs)
- [error] rejects payloads from sources not in the allowlist  (crates/vox-plugin-webhook/src/webhook/handler.rs)
- [error] rejects payloads without a signature when a secret is configured  (crates/vox-plugin-webhook/src/webhook/handler.rs)
- [happy] accepts payloads with a valid HMAC-SHA256 signature  (crates/vox-plugin-webhook/src/webhook/handler.rs)
- [happy] Without a configured secret, accepts and parses payloads from any source  (crates/vox-plugin-webhook/src/webhook/handler.rs)
- [error] With allow_source filtering configured, rejects payloads from sources not in the allowlist  (crates/vox-plugin-webhook/src/webhook/handler.rs)
- [error] With a secret configured, rejects payloads lacking a signature  (crates/vox-plugin-webhook/src/webhook/handler.rs)
- [happy] With a secret configured, accepts payloads with a valid HMAC-SHA256 signature  (crates/vox-plugin-webhook/src/webhook/handler.rs)

### `OrchestratorInboxItem::from_webhook`  (edge, happy; EXTRACTED)
- [happy] Maps GitHub push events to InboxItemKind::GitPush  (crates/vox-plugin-webhook/src/webhook/bridge.rs)
- [happy] Maps GitLab merge_request events to InboxItemKind::PullRequest  (crates/vox-plugin-webhook/src/webhook/bridge.rs)
- [happy] Maps Discord interaction_create events to InboxItemKind::ChannelMessage  (crates/vox-plugin-webhook/src/webhook/bridge.rs)
- [edge] Maps unknown event sources to InboxItemKind::ExternalEvent  (crates/vox-plugin-webhook/src/webhook/bridge.rs)

### `verify_payload`  (error, happy; EXTRACTED)
- [happy] successfully verifies a payload signed with sign_payload  (crates/vox-plugin-webhook/src/webhook/signing.rs)
- [error] returns WebhookError::MissingTimestamp when timestamp header is absent for Slack payloads  (crates/vox-plugin-webhook/src/webhook/signing.rs)
- [error] returns WebhookError::MissingTimestamp when timestamp is empty for Slack payloads  (crates/vox-plugin-webhook/src/webhook/signing.rs)
- [error] returns WebhookError::TimestampOutOfWindow when timestamp is not numeric for Slack payloads  (crates/vox-plugin-webhook/src/webhook/signing.rs)

### `ChannelKind`  (happy; EXTRACTED)
- [happy] ChannelKind variants format as human-readable display strings  (crates/vox-plugin-webhook/src/webhook/channel.rs)

### `ChannelManager::register`  (happy; EXTRACTED)
- [happy] Registered channels appear in list() output with correct id  (crates/vox-plugin-webhook/src/webhook/channel.rs)

### `ChannelManager::send`  (error; EXTRACTED)
- [error] WebSocket send operations that fail return an error result instead of silently dropping  (crates/vox-plugin-webhook/src/webhook/channel.rs)

### `ChannelManager::unregister`  (happy; EXTRACTED)
- [happy] Unregistered channels cannot be retrieved via get()  (crates/vox-plugin-webhook/src/webhook/channel.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`ChannelKind`** — only: _ChannelKind variants format as human-readable display strings_
- **`ChannelManager::register`** — only: _Registered channels appear in list() output with correct id_
- **`ChannelManager::unregister`** — only: _Unregistered channels cannot be retrieved via get()_
