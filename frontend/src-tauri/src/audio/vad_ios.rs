//! Pass-through VAD for iOS.
//!
//! The Silero VAD used on desktop pulls in ONNX Runtime (`ort`), which has no
//! prebuilt iOS binaries and would have to be compiled from source. Rather than
//! carry that, iOS uses this stub: it exposes the same API but treats all audio
//! as speech, so every chunk reaches Whisper.
//!
//! The cost is inference load — desktop VAD filters roughly 70% of audio before
//! transcription, so iOS does correspondingly more work per minute of
//! recording. Transcripts are unaffected.

use anyhow::Result;

/// Mirrors `vad::SpeechSegment`.
#[derive(Debug, Clone)]
pub struct SpeechSegment {
    pub samples: Vec<f32>,
    pub start_timestamp_ms: f64,
    pub end_timestamp_ms: f64,
    pub confidence: f32,
}

/// Mirrors `vad::ContinuousVadProcessor`, without any speech detection.
pub struct ContinuousVadProcessor {
    sample_rate: u32,
    processed_samples: usize,
}

impl ContinuousVadProcessor {
    pub fn new(input_sample_rate: u32, _redemption_time_ms: u32) -> Result<Self> {
        log::info!(
            "iOS pass-through VAD active at {} Hz; all audio is forwarded for transcription",
            input_sample_rate
        );
        Ok(Self {
            sample_rate: input_sample_rate.max(1),
            processed_samples: 0,
        })
    }

    /// Emits the incoming audio as a single segment, timestamped from the
    /// running sample count so downstream timing stays consistent.
    pub fn process_audio(&mut self, samples: &[f32]) -> Result<Vec<SpeechSegment>> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        let ms_per_sample = 1000.0 / self.sample_rate as f64;
        let start_timestamp_ms = self.processed_samples as f64 * ms_per_sample;
        self.processed_samples += samples.len();
        let end_timestamp_ms = self.processed_samples as f64 * ms_per_sample;

        Ok(vec![SpeechSegment {
            samples: samples.to_vec(),
            start_timestamp_ms,
            end_timestamp_ms,
            confidence: 1.0,
        }])
    }

    /// Nothing is buffered, so there is never anything to flush.
    pub fn flush(&mut self) -> Result<Vec<SpeechSegment>> {
        Ok(Vec::new())
    }
}

/// Mirrors `vad::extract_speech_16k`; returns the input unchanged.
pub fn extract_speech_16k(samples_mono_16k: &[f32]) -> Result<Vec<f32>> {
    Ok(samples_mono_16k.to_vec())
}
