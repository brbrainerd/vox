//! Offline smoke: middleware [`ClientWithMiddleware`] builds on top of preset inner client.

use std::time::Duration;

#[test]
fn populi_middleware_client_builds_with_and_without_retry() {
    let inner = vox_http_client::client_builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("inner reqwest client");

    let _no_retry = vox_http_client::populi_control_plane_client(inner.clone(), false);
    let _with_retry = vox_http_client::populi_control_plane_client(inner, true);
}
