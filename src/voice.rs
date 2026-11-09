//! Voice recording, playback, and WAV encoding for native Zalo voice messaging.
//!
//! Provides `VoiceRecorder` for microphone capture via `cpal`, real-time RMS
//! waveform analysis, pure-Rust 16-bit PCM WAV encoding, and in-memory audio preview playback.

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

/// Target sample rate for speech encoding (16 kHz mono).
pub const VOICE_SAMPLE_RATE: u32 = 16_000;

/// Pure-Rust 16-bit mono PCM WAV encoder.
/// Produces standard 44-byte RIFF header followed by signed 16-bit little-endian samples.
pub fn pcm_to_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let num_samples = samples.len();
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = bits_per_sample / 8;
    let byte_rate = sample_rate * u32::from(num_channels) * u32::from(bytes_per_sample);
    let block_align = num_channels * bytes_per_sample;
    let subchunk2_size = (num_samples * usize::from(bytes_per_sample)) as u32;
    let chunk_size = 36 + subchunk2_size;

    let mut wav = Vec::with_capacity(44 + subchunk2_size as usize);

    // RIFF Chunk Descriptor
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&chunk_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // "fmt " sub-chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size = 16 for PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = 1 (PCM)
    wav.extend_from_slice(&num_channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // "data" sub-chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&subchunk2_size.to_le_bytes());

    for &sample in samples {
        // Clamp and scale to 16-bit signed integer
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0).round() as i16;
        wav.extend_from_slice(&sample_i16.to_le_bytes());
    }

    wav
}

/// Calculate RMS (Root Mean Square) volume level normalized to [0.0, 1.0].
pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    let mean = sum_sq / (samples.len() as f32);
    (mean.sqrt() * 3.0).clamp(0.05, 1.0)
}

struct RecorderInner {
    samples: Vec<f32>,
    waveform_levels: Vec<f32>,
    window_samples: Vec<f32>,
    start_time: Instant,
    sample_rate: u32,
}

/// Active voice recording session.
pub struct VoiceRecorder {
    _stream: Option<cpal::Stream>,
    inner: Arc<Mutex<RecorderInner>>,
    active: Arc<AtomicBool>,
}

impl VoiceRecorder {
    /// Start recording from the default input device (microphone).
    pub fn start() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("Không tìm thấy thiết bị micro để ghi âm")?;

        let supported_config = device
            .default_input_config()
            .context("Không hỗ trợ cấu hình âm thanh mặc định của micro")?;

        let sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels() as usize;

        let inner = Arc::new(Mutex::new(RecorderInner {
            samples: Vec::new(),
            waveform_levels: Vec::new(),
            window_samples: Vec::new(),
            start_time: Instant::now(),
            sample_rate,
        }));

        let inner_clone = inner.clone();
        let active = Arc::new(AtomicBool::new(true));
        let active_clone = active.clone();

        let window_size = (sample_rate / 25) as usize; // ~40 ms window for waveform bars

        let err_fn = |err| {
            crate::logger::log_error("voice_recorder", &format!("Lỗi luồng ghi âm: {err}"));
        };

        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &supported_config.into(),
                move |data: &[f32], _: &_| {
                    if !active_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Ok(mut lock) = inner_clone.lock() {
                        for chunk in data.chunks_exact(channels) {
                            let mono = chunk.iter().sum::<f32>() / (channels as f32);
                            lock.samples.push(mono);
                            lock.window_samples.push(mono);
                            if lock.window_samples.len() >= window_size {
                                let rms = calculate_rms(&lock.window_samples);
                                lock.waveform_levels.push(rms);
                                if lock.waveform_levels.len() > 60 {
                                    lock.waveform_levels.remove(0);
                                }
                                lock.window_samples.clear();
                            }
                        }
                    }
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &supported_config.into(),
                move |data: &[i16], _: &_| {
                    if !active_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Ok(mut lock) = inner_clone.lock() {
                        for chunk in data.chunks_exact(channels) {
                            let mono: f32 =
                                chunk.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>()
                                    / (channels as f32);
                            lock.samples.push(mono);
                            lock.window_samples.push(mono);
                            if lock.window_samples.len() >= window_size {
                                let rms = calculate_rms(&lock.window_samples);
                                lock.waveform_levels.push(rms);
                                if lock.waveform_levels.len() > 60 {
                                    lock.waveform_levels.remove(0);
                                }
                                lock.window_samples.clear();
                            }
                        }
                    }
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_input_stream(
                &supported_config.into(),
                move |data: &[u16], _: &_| {
                    if !active_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Ok(mut lock) = inner_clone.lock() {
                        for chunk in data.chunks_exact(channels) {
                            let mono: f32 =
                                chunk.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>()
                                    / (channels as f32);
                            lock.samples.push(mono);
                            lock.window_samples.push(mono);
                            if lock.window_samples.len() >= window_size {
                                let rms = calculate_rms(&lock.window_samples);
                                lock.waveform_levels.push(rms);
                                if lock.waveform_levels.len() > 60 {
                                    lock.waveform_levels.remove(0);
                                }
                                lock.window_samples.clear();
                            }
                        }
                    }
                },
                err_fn,
                None,
            )?,
            other => anyhow::bail!("Định dạng mẫu micro không được hỗ trợ: {other:?}"),
        };

        stream
            .play()
            .context("Không thể khởi động luồng thu âm micro")?;

        Ok(Self {
            _stream: Some(stream),
            inner,
            active,
        })
    }

    /// Retrieve the current recording duration.
    pub fn duration(&self) -> Duration {
        if let Ok(lock) = self.inner.lock() {
            lock.start_time.elapsed()
        } else {
            Duration::ZERO
        }
    }

    /// Retrieve the recent waveform amplitude bars (normalized [0.0, 1.0]).
    pub fn waveform(&self) -> Vec<f32> {
        if let Ok(lock) = self.inner.lock() {
            lock.waveform_levels.clone()
        } else {
            Vec::new()
        }
    }

    /// Stop recording, release device, and return recorded samples resampled to 16 kHz.
    pub fn finish(self) -> (Vec<f32>, u32, Duration) {
        self.active.store(false, Ordering::Relaxed);
        let (samples, sample_rate, duration) = if let Ok(lock) = self.inner.lock() {
            let dur = lock.start_time.elapsed();
            (lock.samples.clone(), lock.sample_rate, dur)
        } else {
            (Vec::new(), VOICE_SAMPLE_RATE, Duration::ZERO)
        };
        (samples, sample_rate, duration)
    }

    /// Discard recording and stop stream.
    pub fn cancel(self) {
        self.active.store(false, Ordering::Relaxed);
    }
}

/// Linear resampler converting from arbitrary input sample rate to 16 kHz.
pub fn resample_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32> {
    if source_rate == VOICE_SAMPLE_RATE || source_rate == 0 || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = VOICE_SAMPLE_RATE as f64 / source_rate as f64;
    let target_len = (samples.len() as f64 * ratio).round() as usize;
    let mut output = Vec::with_capacity(target_len);

    for i in 0..target_len {
        let src_idx = i as f64 / ratio;
        let left_idx = src_idx.floor() as usize;
        let right_idx = (left_idx + 1).min(samples.len() - 1);
        let frac = (src_idx - left_idx as f64) as f32;

        let sample = if left_idx < samples.len() {
            samples[left_idx] * (1.0_f32 - frac) + samples[right_idx] * frac
        } else {
            0.0
        };
        output.push(sample);
    }

    output
}

/// Simple preview player for recorded audio buffers.
pub struct VoicePreviewPlayer {
    _stream: Option<cpal::Stream>,
    playing: Arc<AtomicBool>,
    playback_pos: Arc<Mutex<usize>>,
    total_samples: usize,
}

impl VoicePreviewPlayer {
    pub fn play(samples: Vec<f32>, sample_rate: u32) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("Không tìm thấy thiết bị loa để phát âm thanh")?;

        let supported_config = device
            .default_output_config()
            .context("Không hỗ trợ cấu hình loa mặc định")?;

        let out_sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels() as usize;

        // Resample to output device rate if needed
        let play_samples = if out_sample_rate != sample_rate {
            let ratio = out_sample_rate as f64 / sample_rate as f64;
            let target_len = (samples.len() as f64 * ratio).round() as usize;
            let mut out = Vec::with_capacity(target_len);
            for i in 0..target_len {
                let src_idx = i as f64 / ratio;
                let left = src_idx.floor() as usize;
                let right = (left + 1).min(samples.len().saturating_sub(1));
                let frac = (src_idx - left as f64) as f32;
                let val = if left < samples.len() {
                    samples[left] * (1.0_f32 - frac) + samples[right] * frac
                } else {
                    0.0
                };
                out.push(val);
            }
            out
        } else {
            samples
        };

        let total_samples = play_samples.len();
        let playback_pos = Arc::new(Mutex::new(0usize));
        let pos_clone = playback_pos.clone();
        let playing = Arc::new(AtomicBool::new(true));
        let playing_clone = playing.clone();

        let samples_arc = Arc::new(play_samples);
        let samples_clone = samples_arc.clone();

        let err_fn = |err| {
            crate::logger::log_error("voice_player", &format!("Lỗi loa phát: {err}"));
        };

        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &supported_config.into(),
                move |data: &mut [f32], _: &_| {
                    if !playing_clone.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        return;
                    }
                    if let Ok(mut pos) = pos_clone.lock() {
                        for frame in data.chunks_exact_mut(channels) {
                            if *pos < samples_clone.len() {
                                let s = samples_clone[*pos];
                                frame.fill(s);
                                *pos += 1;
                            } else {
                                frame.fill(0.0);
                                playing_clone.store(false, Ordering::Relaxed);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )?,
            other => anyhow::bail!("Định dạng mẫu phát âm thanh không hỗ trợ: {other:?}"),
        };

        stream.play().context("Không thể phát luồng âm thanh")?;

        Ok(Self {
            _stream: Some(stream),
            playing,
            playback_pos,
            total_samples,
        })
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    pub fn progress(&self) -> f32 {
        if self.total_samples == 0 {
            return 0.0;
        }
        if let Ok(pos) = self.playback_pos.lock() {
            (*pos as f32 / self.total_samples as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn stop(&self) {
        self.playing.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_encoder_creates_valid_riff_header() {
        let dummy_samples = vec![0.0f32, 0.5, -0.5, 0.25, -0.25];
        let wav = pcm_to_wav(&dummy_samples, 16_000);

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");

        // Subchunk1Size = 16
        assert_eq!(u32::from_le_bytes(wav[16..20].try_into().unwrap()), 16);
        // AudioFormat = 1 (PCM)
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1);
        // NumChannels = 1
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
        // SampleRate = 16000
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        // BitsPerSample = 16
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);

        // Data size = 5 samples * 2 bytes = 10 bytes
        let data_size = u32::from_le_bytes(wav[40..44].try_into().unwrap());
        assert_eq!(data_size, 10);
        assert_eq!(wav.len(), 44 + 10);
    }

    #[test]
    fn calculate_rms_clamped_and_proportional() {
        assert_eq!(calculate_rms(&[]), 0.0);
        let silent = vec![0.0f32; 100];
        assert_eq!(calculate_rms(&silent), 0.05);

        let loud = vec![0.8f32; 100];
        let loud_rms = calculate_rms(&loud);
        assert!(loud_rms > 0.5 && loud_rms <= 1.0);
    }

    #[test]
    fn resample_linear_preserves_length_ratio() {
        let samples = vec![0.1f32; 48_000]; // 1 second at 48 kHz
        let resampled = resample_to_16k(&samples, 48_000);
        assert_eq!(resampled.len(), 16_000); // 1 second at 16 kHz
    }
}
