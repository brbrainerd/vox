use vox_gamify::ai::{FreeAiClient, FreeAiProvider};

#[tokio::test]
async fn test_free_ai_client_deterministic() {
    let client = FreeAiClient::new(vec![FreeAiProvider::Deterministic]);
    assert_eq!(client.providers().len(), 1);

    // Test the mock generation logic since network calls shouldn't be made in unit tests
    let response = client.generate("test prompt").await;
    assert!(response.is_ok());

    let text = response.unwrap();
    assert!(text.contains("offline mode"));
}
