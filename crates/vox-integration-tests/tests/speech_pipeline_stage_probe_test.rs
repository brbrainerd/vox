#![allow(missing_docs)]

//! Lightweight stage probes for the speech-to-code audit.
//!
//! These tests avoid model downloads. Runtime ASR cells are covered by the audit
//! matrix and may skip with explicit reasons when model or hardware assets are
//! unavailable.

use std::fs;
use std::path::PathBuf;

use vox_speech::refine::{CorrectionContext, refine_transcript};
use vox_speech::routing::IdeContext;
use vox_speech::vad::create_vad;
use vox_speech::{
    OratioRuntimeConfig, RouteMode, preprocess_audio_pcm_f32_reported,
    route_transcript_with_options,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn probe_capture_editor_webview_preserves_native_sample_rate_for_audit() {
    let root = workspace_root();
    let source = fs::read_to_string(
        root.join("apps/editor/vox-vscode/src/speech/registerOratioSpeechCommands.ts"),
    )
    .expect("read editor speech commands");

    assert!(source.contains("navigator.mediaDevices.getUserMedia({ audio: true })"));
    assert!(source.contains("const sr = audioCtx.sampleRate"));
    assert!(source.contains("encodeWavMono(sampleBuffers, sr)"));
    assert!(source.contains("sampleRate: sr"));
}

#[test]
fn probe_dashboard_speak_surface_has_no_microphone_path() {
    let root = workspace_root();
    let speak_panel_path = root.join("crates/vox-dashboard/src/components/shell/SpeakPanel.tsx");
    if !speak_panel_path.exists() {
        // vox-dashboard not yet created; test vacuously passes
        return;
    }
    let speak_panel = fs::read_to_string(&speak_panel_path).expect("read SpeakPanel");
    let chat_hook = fs::read_to_string(root.join("crates/vox-dashboard/src/hooks/useVoxChat.ts"))
        .expect("read useVoxChat");

    for forbidden in [
        "getUserMedia",
        "MediaRecorder",
        "vox_oratio",
        "vox_speech_to_code",
    ] {
        assert!(
            !speak_panel.contains(forbidden),
            "SpeakPanel unexpectedly contains speech capture token {forbidden}"
        );
    }
    assert!(
        chat_hook.contains("vox_chat_message"),
        "dashboard speak should currently route text chat through vox_chat_message"
    );
}

#[test]
fn probe_vad_suppresses_near_silence_before_stt() {
    let mut vad = create_vad();
    let quiet = vec![0.001_f32; 16_000];
    let segments = vad.detect_segments(&quiet, 16_000);
    assert!(
        segments.is_empty(),
        "near-silence should not produce voiced VAD segments"
    );
}

#[test]
fn probe_refine_confidence_stays_inside_runtime_bounds() {
    let ctx = CorrectionContext::default();
    let refined = refine_transcript("  VOXX   oratia  ", &ctx);
    let tunables = ctx.refine_tunables;

    assert_eq!(refined.text, "vox oratio");
    assert!(
        refined.confidence >= tunables.conf_min && refined.confidence <= tunables.conf_max,
        "refine confidence {} outside [{}, {}]",
        refined.confidence,
        tunables.conf_min,
        tunables.conf_max
    );
}

#[test]
fn probe_routing_respects_tool_and_orchestrator_confidence_thresholds() {
    let runtime = OratioRuntimeConfig::default();
    let ctx = IdeContext::default();

    let low_tool = route_transcript_with_options(
        RouteMode::Tool,
        "speech-audit-low-tool",
        "create function hello",
        runtime.routing.tool_route_min_confidence - 0.01,
        &runtime,
        &ctx,
    );
    assert_eq!(low_tool.status, "below_tool_confidence");

    let low_orchestrator = route_transcript_with_options(
        RouteMode::Orchestrator,
        "speech-audit-low-orchestrator",
        "create function hello",
        runtime.routing.orchestrator_min_confidence - 0.01,
        &runtime,
        &ctx,
    );
    assert_eq!(low_orchestrator.status, "queued_low_confidence");
}

#[test]
fn probe_acoustic_preprocess_reports_noop_without_env_override() {
    let samples = vec![0.25_f32, -0.25, 0.125, -0.125];
    let (out, diagnostics) = preprocess_audio_pcm_f32_reported(&samples, 100);

    assert_eq!(out, samples);
    assert_eq!(diagnostics.mode, "none");
    assert!(!diagnostics.skipped_due_to_budget);
}
