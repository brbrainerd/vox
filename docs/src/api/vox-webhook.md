# Crate API: vox-webhook

## Module: `vox-webhook\src\channel.rs`

Channel abstraction — Discord, Slack, WebSocket, and custom integrations.


### `enum ChannelKind`

Channel kind discriminant.


### `struct ChannelEvent`

A message event sent through a channel.


### `struct Channel`

Configuration for a registered channel.


### `struct ChannelManager`

Manages registered channels and dispatches events.


## Module: `vox-webhook\src\delivery.rs`

Outbound webhook delivery with retry, backoff, and delivery receipts.


### `struct OutboundWebhook`

An outbound webhook configuration.


### `struct WebhookDeliveryResult`

The result of a webhook delivery attempt.


### `struct WebhookDelivery`

Delivers payloads to outbound webhook endpoints with retry logic.


## Module: `vox-webhook\src\handler.rs`

Inbound webhook handler — parses, verifies and routes incoming webhook events.


### `struct InboundPayload`

A normalized inbound webhook payload.


### `struct WebhookEvent`

A parsed webhook event ready for dispatch.


### `struct WebhookHandler`

Processes an inbound webhook payload:
1. Optionally verifies signature
2. Returns a `WebhookEvent` ready for dispatch


## Module: `vox-webhook\src\lib.rs`

# vox-webhook — HTTP Webhook Gateway
Provides an inbound webhook receiver, outbound delivery with retry/signing,
and a `Channel` abstraction for Discord/Slack/WebSocket integrations.


### `enum WebhookError`

Errors from the webhook system.


## Module: `vox-webhook\src\router.rs`

Axum HTTP router for the inbound webhook gateway.

Exposes:
- POST `/webhooks/:source` — receive an inbound webhook
- GET  `/webhooks/health` — health check
- GET  `/webhooks/channels` — list registered channels


### `struct WebhookState`

Shared state for the webhook router.


### `fn build_router`

Build the Axum `Router` for the webhook gateway.


### `fn serve`

Start the webhook server on the given bind address.


## Module: `vox-webhook\src\signing.rs`

Webhook signature generation and verification using HMAC-SHA3-256.


### `struct WebhookSignature`

A webhook signature — an HMAC-SHA3-256 hex digest.


### `fn sign_payload`

Sign a payload with a secret key using HMAC-SHA3-256.


### `fn verify_payload`

Verify a payload against a signature string (e.g. "sha3=abc123...").


