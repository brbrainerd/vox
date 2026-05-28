//! Generated provider enums + chat route helpers.

use vox_orchestrator_types::{
    ChatProviderRouteKind, ChatRouteBackend, ProviderType, backend_telemetry_labels,
    route_backend_for_chat_route,
};

#[test]
fn provider_type_default_backend_matches_contract() {
    assert_eq!(
        ProviderType::OpenRouter.default_backend(),
        ChatRouteBackend::OpenRouter
    );
    assert_eq!(
        ProviderType::Ollama.default_backend(),
        ChatRouteBackend::Ollama
    );
}

#[test]
fn route_backend_for_chat_route_normalizes_openrouter_and_gemini_hint() {
    let openrouter = ChatProviderRouteKind::OpenRouter {
        model: "openrouter/auto".into(),
    };
    assert_eq!(
        route_backend_for_chat_route(&openrouter),
        ChatRouteBackend::OpenRouter
    );

    let gemini = ChatProviderRouteKind::ManualOpenAiCompatible {
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        model: "gemini-pro".into(),
        bearer: None,
    };
    assert_eq!(
        route_backend_for_chat_route(&gemini),
        ChatRouteBackend::GeminiDirect
    );
}

#[test]
fn backend_telemetry_labels_cover_openrouter_and_ollama() {
    let (p, s) = backend_telemetry_labels(ChatRouteBackend::OpenRouter);
    assert_eq!(p, "openrouter");
    assert_eq!(s, "openrouter");
    let (p2, s2) = backend_telemetry_labels(ChatRouteBackend::Ollama);
    assert_eq!(p2, "mens");
    assert_eq!(s2, "populi_local");
}
