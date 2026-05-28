//! Offline checks for shared `reqwest` presets (`vox-reqwest-defaults`).

use vox_http_client::client_builder;

#[test]
fn client_builder_preset_builds_offline() {
    // Default headers (including user-agent) are applied when the request is sent,
    // not on `Request::headers()` for a freshly built request.
    let _client = client_builder()
        .build()
        .expect("ClientBuilder from vox_http_client should build offline");
}

#[test]
fn fallback_client_constructor_does_not_panic() {
    let _c = vox_http_client::client();
}
