//! Runtime backend selection for Oratio STT.
//!
//! Priority: `VOX_ORATIO_BACKEND` env → feature flags → Candle Whisper fallback.

use crate::backends::asr_backend::AsrBackend;

#[cfg(feature = "stt-candle")]
use crate::backends::candle_whisper::CandleWhisperBackend;

/// Instantiate the configured STT backend.
///
/// # Env
/// - `VOX_ORATIO_BACKEND=auto` (default) — picks Sherpa if compiled in, else Candle
/// - `VOX_ORATIO_BACKEND=whisper` — always Candle Whisper
/// - `VOX_ORATIO_BACKEND=sherpa` — always Sherpa (returns error if feature not compiled)
pub fn create_backend() -> anyhow::Result<Box<dyn AsrBackend>> {
    let backend_env = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioBackend)
        .expose()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "auto".to_string());
    let backend_env = backend_env.trim().to_ascii_lowercase();

    match backend_env.as_str() {
        "auto" | "" => {
            #[cfg(feature = "stt-sherpa")]
            {
                return Ok(Box::new(
                    crate::backends::sherpa_onnx::SherpaOnnxBackend::new()?,
                ));
            }
            #[cfg(all(feature = "stt-candle", not(feature = "stt-sherpa")))]
            {
                Ok(Box::new(CandleWhisperBackend))
            }
            #[cfg(not(any(feature = "stt-candle", feature = "stt-sherpa")))]
            anyhow::bail!(
                "No STT backend compiled in. Enable `stt-candle` or `stt-sherpa` feature."
            );
        }
        "whisper" | "candle" => {
            #[cfg(feature = "stt-candle")]
            return Ok(Box::new(CandleWhisperBackend));
            #[cfg(not(feature = "stt-candle"))]
            anyhow::bail!("Backend 'whisper' selected but `stt-candle` feature not compiled.");
        }
        "sherpa" => {
            #[cfg(feature = "stt-sherpa")]
            return Ok(Box::new(
                crate::backends::sherpa_onnx::SherpaOnnxBackend::new()?,
            ));
            #[cfg(not(feature = "stt-sherpa"))]
            anyhow::bail!("Backend 'sherpa' selected but `stt-sherpa` feature not compiled.");
        }
        other => anyhow::bail!("Unknown VOX_ORATIO_BACKEND value: {other:?}"),
    }
}
