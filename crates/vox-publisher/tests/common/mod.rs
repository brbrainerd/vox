//! Shared helpers for integration tests (`tests/*.rs` re-export via `mod common`).

use std::net::SocketAddr;

/// Wait until something accepts TCP connections on `addr` (mock HTTP server readiness).
pub async fn wait_for_local_server(addr: SocketAddr, label: &str) {
    let deadline = tokio::time::Instant::now() + vox_config::timeouts::D_5S;
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("{label}: local server did not accept connections on {addr}");
        }
        tokio::time::sleep(vox_config::timeouts::D_10MS).await;
    }
}
