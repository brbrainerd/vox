//! Subtitle generation subsystem (.srt).

/// Fallback ffmpeg audio extraction.
pub mod ffmpeg_extract;
/// Subtitle generator formatting utilities (SRT).
pub mod srt;

pub use srt::format_srt;
pub use srt::generate_srt_file;
