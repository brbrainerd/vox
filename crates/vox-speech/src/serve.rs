#[cfg(feature = "serve")]
use axum::{Json, Router, extract::Multipart, routing::post};
#[cfg(feature = "serve")]
use std::net::SocketAddr;

#[cfg(feature = "serve")]
use crate::backends::candle_whisper::transcribe_pcm_internal;

#[cfg(feature = "serve")]
pub async fn run_serve_worker(port: u16) -> anyhow::Result<()> {
    tracing::info!("Starting local Oratio worker on port {}", port);

    let app = Router::new().route("/transcribe", post(transcribe_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(feature = "serve")]
async fn transcribe_handler(
    mut multipart: Multipart,
) -> Result<Json<crate::backends::asr_backend::AsrOutput>, axum::http::StatusCode> {
    let mut file_data = Vec::new();
    let mut _sample_rate = 16000;
    let mut _language = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field
                .bytes()
                .await
                .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
            file_data.extend_from_slice(&data);
        } else if name == "sample_rate" {
            let text = field
                .text()
                .await
                .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
            if let Ok(sr) = text.parse::<u32>() {
                _sample_rate = sr;
            }
        } else if name == "language" {
            let text = field
                .text()
                .await
                .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
            if !text.is_empty() {
                _language = Some(text);
            }
        }
    }

    if file_data.is_empty() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    // Convert raw LE bytes to f32 PCM
    let mut pcm_data = Vec::with_capacity(file_data.len() / 4);
    for chunk in file_data.chunks_exact(4) {
        let val = f32::from_le_bytes(chunk.try_into().unwrap());
        pcm_data.push(val);
    }

    let (raw_text, segments) = tokio::task::spawn_blocking(move || {
        transcribe_pcm_internal(&pcm_data, _language.as_deref())
    })
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(crate::backends::asr_backend::AsrOutput {
        raw_text,
        confidence: 0.85,
        n_best: Vec::new(),
        segments,
    }))
}
