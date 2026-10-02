//! Windowing stand-in for the Silero VAD on iOS.
//!
//! The Silero VAD used on desktop pulls in ONNX Runtime (`ort`), which has no
//! prebuilt iOS binaries and would have to be compiled from source. Rather than
//! carry that, iOS uses this module: it exposes the same API as `vad`, but it
//! cannot tell speech from silence, so it hands Whisper fixed-length windows of
//! everything it hears (minus windows that are essentially silent).
//!
//! Two parts of the `vad` contract matter and are easy to get wrong:
//!
//! 1. **Segments must come out at 16 kHz.** `AudioPipeline` feeds this
//!    processor the mixed 48 kHz stream and then stamps whatever comes back as
//!    `sample_rate: 16000` without looking. The desktop VAD resamples
//!    internally, so returning the input untouched made Whisper read 48 kHz
//!    audio as 16 kHz -- everything transcribed at 3x speed as garbage.
//! 2. **Segments must be worth a Whisper call.** The pipeline mixes in ~50 ms
//!    windows, so emitting one segment per call meant ~20 inferences a second,
//!    each padded internally to Whisper's 30 s mel window. Audio is accumulated
//!    into `WINDOW_MS` chunks instead.
//!
//! The cost of having no real VAD is inference load: desktop filters roughly
//! 70% of audio before transcription, while iOS transcribes every non-silent
//! window.

use anyhow::Result;

/// Whisper's native input rate; matches `VAD_SAMPLE_RATE` in `vad`.
const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// How much audio to gather before handing a window to Whisper.
///
/// This is the latency/quality dial. Shorter windows surface transcripts sooner
/// but cut words at the boundaries and give Whisper less context to work with;
/// longer windows read better but lag. Without a VAD there is no speech pause
/// to cut on, so the boundary is arbitrary either way.
const WINDOW_MS: u64 = 5_000;

/// Windows whose RMS falls below this are dropped instead of transcribed.
///
/// Microphone audio reaches the pipeline normalized to -23 LUFS (RMS ~0.07 for
/// speech), and the normalizer derives its gain from cumulative loudness rather
/// than per-window, so a silent window stays quiet instead of being boosted.
/// This threshold sits ~30 dB below speech: low enough to keep quiet talking,
/// high enough to stop Whisper hallucinating ("thank you for watching") over
/// silence, which is otherwise guaranteed with no VAD in front of it.
const SILENCE_RMS: f32 = 0.002;

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
    /// Rate of the audio arriving from the pipeline (48 kHz in practice).
    input_sample_rate: u32,
    /// `WINDOW_MS` expressed in input-rate samples.
    window_samples: usize,
    /// Input-rate audio awaiting a full window.
    buffer: Vec<f32>,
    /// Input-rate samples already windowed, including dropped silent ones, so
    /// timestamps stay pinned to the real position in the recording.
    consumed_samples: usize,
}

impl ContinuousVadProcessor {
    pub fn new(input_sample_rate: u32, _redemption_time_ms: u32) -> Result<Self> {
        let input_sample_rate = input_sample_rate.max(1);
        let window_samples =
            ((input_sample_rate as u64 * WINDOW_MS) / 1000).max(1) as usize;

        log::info!(
            "iOS windowing VAD active: {} Hz in, {} ms windows ({} samples) resampled to {} Hz; \
             all non-silent audio is forwarded for transcription",
            input_sample_rate,
            WINDOW_MS,
            window_samples,
            WHISPER_SAMPLE_RATE
        );

        Ok(Self {
            input_sample_rate,
            window_samples,
            buffer: Vec::with_capacity(window_samples * 2),
            consumed_samples: 0,
        })
    }

    /// Accumulates audio and emits a 16 kHz segment per completed window.
    pub fn process_audio(&mut self, samples: &[f32]) -> Result<Vec<SpeechSegment>> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        self.buffer.extend_from_slice(samples);

        let mut segments = Vec::new();
        while self.buffer.len() >= self.window_samples {
            let window: Vec<f32> = self.buffer.drain(..self.window_samples).collect();
            if let Some(segment) = self.build_segment(&window)? {
                segments.push(segment);
            }
        }

        Ok(segments)
    }

    /// Emits whatever is left as a final short window.
    pub fn flush(&mut self) -> Result<Vec<SpeechSegment>> {
        if self.buffer.is_empty() {
            return Ok(Vec::new());
        }

        let window = std::mem::take(&mut self.buffer);
        Ok(self.build_segment(&window)?.into_iter().collect())
    }

    /// Resamples one window to 16 kHz, dropping it if it is silent.
    ///
    /// Each window is resampled on its own rather than through a resampler that
    /// carries state across calls, which leaves a small discontinuity at window
    /// edges. That only affects the transcription path -- the saved recording is
    /// mixed separately and never passes through here.
    fn build_segment(&mut self, window: &[f32]) -> Result<Option<SpeechSegment>> {
        let start_timestamp_ms = self.samples_to_ms(self.consumed_samples);
        self.consumed_samples += window.len();
        let end_timestamp_ms = self.samples_to_ms(self.consumed_samples);

        let rms = (window.iter().map(|s| s * s).sum::<f32>() / window.len() as f32).sqrt();
        if rms < SILENCE_RMS {
            log::debug!(
                "iOS VAD: dropping silent window {:.0}-{:.0} ms (RMS {:.5} < {:.5})",
                start_timestamp_ms,
                end_timestamp_ms,
                rms,
                SILENCE_RMS
            );
            return Ok(None);
        }

        let samples = if self.input_sample_rate == WHISPER_SAMPLE_RATE {
            window.to_vec()
        } else {
            // Deliberately the fallible `resample`, not `resample_audio`: the
            // latter returns the input unchanged on failure, which is precisely
            // the silent 48-kHz-labelled-16-kHz bug this module used to have.
            super::audio_processing::resample(
                window,
                self.input_sample_rate,
                WHISPER_SAMPLE_RATE,
            )?
        };

        Ok(Some(SpeechSegment {
            samples,
            start_timestamp_ms,
            end_timestamp_ms,
            confidence: 1.0,
        }))
    }

    fn samples_to_ms(&self, samples: usize) -> f64 {
        samples as f64 * 1000.0 / self.input_sample_rate as f64
    }
}

/// Mirrors `vad::extract_speech_16k`; returns the input unchanged.
pub fn extract_speech_16k(samples_mono_16k: &[f32]) -> Result<Vec<f32>> {
    Ok(samples_mono_16k.to_vec())
}
