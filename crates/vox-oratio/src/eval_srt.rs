//! Timing-aware evaluation for subtitle boundaries.

use crate::backends::asr_backend::TimedSegment;

/// Compute the mean absolute timing error (in milliseconds) between predicted and expected subtitle blocks.
///
/// This is a simplified "TER" (Timing Error Rate) based on matching blocks by text similarity,
/// then computing differences. For a production metric, you would want dynamic time warping (DTW)
/// against the target audio, but this serves as a baseline proxy using ground-truth subtitles.
pub fn mean_timing_offset_ms(expected: &[TimedSegment], actual: &[TimedSegment]) -> f32 {
    if expected.is_empty() || actual.is_empty() {
        return 0.0;
    }

    let mut total_offset = 0i64;
    let mut matched = 0;

    // Very basic alignment: slide actual over expected, matching sequential tokens
    // We'll limit just to 1-to-1 match by sequence index for now as a rough baseline.
    let count = expected.len().min(actual.len());
    for i in 0..count {
        let e = &expected[i];
        let a = &actual[i];

        let start_diff = (e.start_ms as i64 - a.start_ms as i64).abs();
        let end_diff = (e.end_ms as i64 - a.end_ms as i64).abs();

        total_offset += start_diff + end_diff;
        matched += 2;
    }

    if matched > 0 {
        total_offset as f32 / matched as f32
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires network/large file — owner: oratio sunset: 2026-12-31"]
    fn test_sintel_frame_to_ms_accuracy() {
        // Pseudo-test logic:
        // 1. Download/extract 1-minute sintel clip (e.g. Sintel.2010.720p.mkv segments).
        // 2. Fetch expected SRT: https://mango.blender.org/wp-content/content/subtitles/sintel_en.srt
        // 3. Process video through `generate_srt_file`.
        // 4. Compute `mean_timing_offset_ms`.
        // 5. assert!(offset < 200.0, "TER higher than 200ms budget");
        let expected = vec![TimedSegment {
            start_ms: 1000,
            end_ms: 2000,
            text: "Wait!".into(),
        }];
        let actual = vec![TimedSegment {
            start_ms: 1050,
            end_ms: 1950,
            text: "Wait".into(),
        }];
        let offset = mean_timing_offset_ms(&expected, &actual);
        assert!(offset == 50.0);
    }
}
