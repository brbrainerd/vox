use serial_test::serial;
use std::time::Duration;
use tokio::net::TcpListener;
use vox_populi::http_client::PopuliHttpClient;
use vox_populi::transport::{PopuliHttpAuth, PopuliTransportState, populi_http_app_with_auth};

static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
#[allow(clippy::await_holding_lock)] // Lock intentionally held across awaits to serialize env-mutating tests.
async fn verify_dispatch_results_persistence_across_restart() {
    let _guard = ENV_MUTEX.lock().expect("env lock");
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let store_path = tmp_dir.path().join("dispatch-store.json");

    unsafe {
        std::env::set_var("VOX_MESH_DISPATCH_STORE_PATH", store_path.to_str().unwrap());
    }

    let dispatch_id = "test-dispatch-123".to_string();

    // Pass 1: Simulate node having executed a dispatch and saved it to the file.
    // Instead of using the private `dispatch_results` map, we mock the file data for a restart scenario.
    let payload = serde_json::json!({
        dispatch_id.clone(): {
            "success": true,
            "output": "wait result retrieved after restart",
            "is_truncated": false,
            "error_message": null,
            "node_id": "test-node"
        }
    });
    std::fs::write(&store_path, serde_json::to_string_pretty(&payload).unwrap())
        .expect("setup dispatch persistence store");

    assert!(
        store_path.exists(),
        "Dispatch store should exist after Pass 1"
    );

    // Pass 2: Restart node via new_for_serve(), verify Wait operations retrieve the result.
    {
        let state = PopuliTransportState::new_for_serve();
        let app = populi_http_app_with_auth(state, PopuliHttpAuth::Open);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}", port);

        let server_task = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });

        let client = PopuliHttpClient::new_with_timeout(&base_url, Duration::from_secs(2));

        // Let's poll for the dispatch wait
        let response = client
            .dispatch_result_poll(&dispatch_id)
            .await
            .expect("should retrieve result via wait");

        assert!(response.success);
        assert_eq!(response.output, "wait result retrieved after restart");
        assert!(!response.is_truncated);

        server_task.abort();
    }

    unsafe {
        std::env::remove_var("VOX_MESH_DISPATCH_STORE_PATH");
    }
}
