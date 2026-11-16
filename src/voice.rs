//! Voice recording, playback, and WAV encoding for native Zalo voice messaging.
//!
//! Provides `VoiceRecorder` for microphone capture via `cpal`, real-time RMS
//! waveform analysis, pure-Rust 16-bit PCM WAV encoding, and in-memory audio preview playback.

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

/// Target sample rate for speech encoding (16 kHz mono).
pub const VOICE_SAMPLE_RATE: u32 = 16_000;
/// Local recording duration budget, not a claim about Zalo's server limit.
pub const VOICE_MAX_DURATION: Duration = Duration::from_secs(5 * 60);
/// Maximum captured mono PCM storage before the device worker finishes recording.
pub const VOICE_MAX_BYTES: usize = 64 * 1024 * 1024;
const VOICE_MAX_SAMPLES: usize = VOICE_MAX_BYTES / std::mem::size_of::<f32>();

fn recording_limit_reached(duration: Duration, sample_count: usize) -> bool {
    duration >= VOICE_MAX_DURATION || sample_count >= VOICE_MAX_SAMPLES
}

fn invalidate_recording_generation(active_generation: &AtomicU64, generation: u64) {
    let _ = active_generation.compare_exchange(
        generation,
        generation.wrapping_add(1),
        Ordering::AcqRel,
        Ordering::Acquire,
    );
}

fn preview_generation_is_active(active_generation: &AtomicU64, generation: u64) -> bool {
    active_generation.load(Ordering::Acquire) == generation
}

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
    limit_reached: bool,
}

fn append_sample(
    inner: &mut RecorderInner,
    sample: f32,
    window_size: usize,
    active: &AtomicBool,
) -> bool {
    if recording_limit_reached(inner.start_time.elapsed(), inner.samples.len()) {
        inner.limit_reached = true;
        active.store(false, Ordering::Relaxed);
        return false;
    }
    inner.samples.push(sample);
    inner.window_samples.push(sample);
    if inner.window_samples.len() >= window_size {
        let rms = calculate_rms(&inner.window_samples);
        inner.waveform_levels.push(rms);
        if inner.waveform_levels.len() > 60 {
            inner.waveform_levels.remove(0);
        }
        inner.window_samples.clear();
    }
    if recording_limit_reached(Duration::ZERO, inner.samples.len()) {
        inner.limit_reached = true;
        active.store(false, Ordering::Relaxed);
        return false;
    }
    true
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
            limit_reached: false,
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
                            if !append_sample(&mut lock, mono, window_size, &active_clone) {
                                break;
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
                            if !append_sample(&mut lock, mono, window_size, &active_clone) {
                                break;
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
                            if !append_sample(&mut lock, mono, window_size, &active_clone) {
                                break;
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

    fn snapshot(&self) -> (Duration, Vec<f32>, usize, bool) {
        if let Ok(lock) = self.inner.lock() {
            let duration = lock.start_time.elapsed();
            (
                duration,
                lock.waveform_levels.clone(),
                lock.samples.len(),
                lock.limit_reached || duration >= VOICE_MAX_DURATION,
            )
        } else {
            (Duration::ZERO, Vec::new(), 0, false)
        }
    }

    /// Stop recording, release the device, and move out the bounded mono sample buffer.
    pub fn finish(mut self) -> (Vec<f32>, u32, Duration, bool) {
        self.active.store(false, Ordering::Relaxed);
        self._stream.take();
        let (samples, sample_rate, duration, limit_reached) =
            if let Ok(mut lock) = self.inner.lock() {
                let dur = lock.start_time.elapsed();
                (
                    std::mem::take(&mut lock.samples),
                    lock.sample_rate,
                    dur,
                    lock.limit_reached || dur >= VOICE_MAX_DURATION,
                )
            } else {
                (Vec::new(), VOICE_SAMPLE_RATE, Duration::ZERO, false)
            };
        (samples, sample_rate, duration, limit_reached)
    }

    /// Discard recording and stop stream.
    pub fn cancel(mut self) {
        self.active.store(false, Ordering::Relaxed);
        self._stream.take();
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
    playback_pos: Arc<Mutex<f64>>,
    total_samples: usize,
}

impl VoicePreviewPlayer {
    pub fn play(samples: Arc<[f32]>, sample_rate: u32) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("Không tìm thấy thiết bị loa để phát âm thanh")?;

        let supported_config = device
            .default_output_config()
            .context("Không hỗ trợ cấu hình loa mặc định")?;

        let out_sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels() as usize;

        anyhow::ensure!(
            sample_rate > 0 && out_sample_rate > 0,
            "Tần số mẫu âm thanh không hợp lệ"
        );
        let total_samples = samples.len();
        let source_step = sample_rate as f64 / out_sample_rate as f64;
        let playback_pos = Arc::new(Mutex::new(0.0f64));
        let pos_clone = playback_pos.clone();
        let playing = Arc::new(AtomicBool::new(true));
        let playing_clone = playing.clone();

        let samples_clone = samples;

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
                            if *pos < samples_clone.len() as f64 {
                                let left = (*pos).floor() as usize;
                                let right = (left + 1).min(samples_clone.len().saturating_sub(1));
                                let frac = (*pos - left as f64) as f32;
                                let s = samples_clone[left] * (1.0 - frac)
                                    + samples_clone[right] * frac;
                                frame.fill(s);
                                *pos += source_step;
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

#[derive(Clone, Default)]
pub(crate) struct VoiceSnapshot {
    pub(crate) recording: bool,
    pub(crate) duration: Duration,
    pub(crate) waveform: Vec<f32>,
    pub(crate) recorded_bytes: usize,
    pub(crate) preview_playing: bool,
}

pub(crate) enum VoiceEvent {
    Finished {
        generation: u64,
        samples: Arc<[f32]>,
        sample_rate: u32,
        duration: Duration,
        limit_reached: bool,
        send_after_finish: bool,
    },
    Failed {
        generation: u64,
        message: String,
    },
    PreviewFailed {
        generation: u64,
        message: String,
    },
}

enum VoiceCommand {
    Start {
        generation: u64,
    },
    Finish {
        generation: u64,
        send_after_finish: bool,
    },
    Cancel {
        generation: u64,
    },
    PlayPreview {
        generation: u64,
        samples: Arc<[f32]>,
        sample_rate: u32,
    },
    StopPreview {
        generation: u64,
    },
    Shutdown,
}

/// Device-owning worker; CPAL streams remain on the dedicated thread because some host backends
/// do not permit moving their stream handles between threads.
pub(crate) struct VoiceWorker {
    commands: SyncSender<VoiceCommand>,
    events: Receiver<VoiceEvent>,
    snapshot: Arc<Mutex<VoiceSnapshot>>,
    active_generation: Arc<AtomicU64>,
}

impl VoiceWorker {
    pub(crate) fn new(ctx: eframe::egui::Context) -> Result<Self> {
        let (command_tx, command_rx) = mpsc::sync_channel(4);
        let (event_tx, event_rx) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(VoiceSnapshot::default()));
        let thread_snapshot = Arc::clone(&snapshot);
        let active_generation = Arc::new(AtomicU64::new(0));
        let thread_generation = Arc::clone(&active_generation);
        std::thread::Builder::new()
            .name("voice-device".into())
            .spawn(move || {
                voice_worker_loop(
                    command_rx,
                    event_tx,
                    thread_snapshot,
                    thread_generation,
                    ctx,
                )
            })
            .context("Không khởi chạy được bộ xử lý âm thanh")?;
        Ok(Self {
            commands: command_tx,
            events: event_rx,
            snapshot,
            active_generation,
        })
    }

    pub(crate) fn start(&self, generation: u64) -> Result<()> {
        self.active_generation.store(generation, Ordering::Release);
        self.send(VoiceCommand::Start { generation })
    }

    pub(crate) fn finish(&self, generation: u64, send_after_finish: bool) -> Result<()> {
        self.send(VoiceCommand::Finish {
            generation,
            send_after_finish,
        })
    }

    pub(crate) fn cancel(&self, generation: u64) -> Result<()> {
        invalidate_recording_generation(&self.active_generation, generation);
        let _ = self.commands.try_send(VoiceCommand::Cancel { generation });
        Ok(())
    }

    pub(crate) fn play_preview(
        &self,
        generation: u64,
        samples: Arc<[f32]>,
        sample_rate: u32,
    ) -> Result<()> {
        self.send(VoiceCommand::PlayPreview {
            generation,
            samples,
            sample_rate,
        })
    }

    pub(crate) fn stop_preview(&self, generation: u64) -> Result<()> {
        self.send(VoiceCommand::StopPreview { generation })
    }

    pub(crate) fn snapshot(&self) -> VoiceSnapshot {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default()
    }

    pub(crate) fn try_events(&self) -> impl Iterator<Item = VoiceEvent> + '_ {
        self.events.try_iter()
    }

    fn send(&self, command: VoiceCommand) -> Result<()> {
        self.commands
            .try_send(command)
            .map_err(|_| anyhow::anyhow!("Bộ xử lý âm thanh đang bận hoặc đã dừng"))
    }
}

impl Drop for VoiceWorker {
    fn drop(&mut self) {
        let _ = self.commands.try_send(VoiceCommand::Shutdown);
    }
}

fn voice_worker_loop(
    commands: Receiver<VoiceCommand>,
    events: mpsc::Sender<VoiceEvent>,
    snapshot: Arc<Mutex<VoiceSnapshot>>,
    active_generation: Arc<AtomicU64>,
    ctx: eframe::egui::Context,
) {
    let mut recorder = None;
    let mut recording_generation = None;
    let mut preview: Option<VoicePreviewPlayer> = None;
    let mut preview_generation = None;
    loop {
        let received = if recorder.is_none() && preview.is_none() {
            commands
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        } else {
            commands.recv_timeout(Duration::from_millis(40))
        };
        match received {
            Ok(VoiceCommand::Start { generation })
                if recorder.is_none()
                    && active_generation.load(Ordering::Acquire) == generation =>
            {
                match VoiceRecorder::start() {
                    Ok(active) => {
                        recorder = Some(active);
                        recording_generation = Some(generation);
                        if let Ok(mut current) = snapshot.lock() {
                            *current = VoiceSnapshot {
                                recording: true,
                                ..Default::default()
                            };
                        }
                        ctx.request_repaint();
                    }
                    Err(error) => {
                        let _ = events.send(VoiceEvent::Failed {
                            generation,
                            message: error.to_string(),
                        });
                        ctx.request_repaint();
                    }
                }
            }
            Ok(VoiceCommand::Finish {
                generation,
                send_after_finish,
            }) => {
                if recording_generation == Some(generation) {
                    if let Some(active) = recorder.take() {
                        let (samples, sample_rate, duration, limit_reached) = active.finish();
                        recording_generation = None;
                        if let Ok(mut current) = snapshot.lock() {
                            *current = VoiceSnapshot::default();
                        }
                        let _ = events.send(VoiceEvent::Finished {
                            generation,
                            samples: Arc::from(samples),
                            sample_rate,
                            duration,
                            limit_reached,
                            send_after_finish,
                        });
                        ctx.request_repaint();
                    }
                }
            }
            Ok(VoiceCommand::Cancel { generation }) => {
                let mut cancelled_recording = false;
                if recording_generation == Some(generation) {
                    if let Some(active) = recorder.take() {
                        active.cancel();
                    }
                    recording_generation = None;
                    cancelled_recording = true;
                }
                if preview_generation == Some(generation) {
                    if let Some(active) = preview.take() {
                        active.stop();
                    }
                    preview_generation = None;
                    if let Ok(mut current) = snapshot.lock() {
                        current.preview_playing = false;
                    }
                }
                if cancelled_recording {
                    if let Ok(mut current) = snapshot.lock() {
                        *current = VoiceSnapshot::default();
                    }
                }
                ctx.request_repaint();
            }
            Ok(VoiceCommand::PlayPreview {
                generation,
                samples,
                sample_rate,
            }) => {
                if preview_generation_is_active(&active_generation, generation) {
                    if let Some(active) = preview.take() {
                        active.stop();
                    }
                    preview_generation = None;
                    match VoicePreviewPlayer::play(samples, sample_rate) {
                        Ok(active) => {
                            preview = Some(active);
                            preview_generation = Some(generation);
                        }
                        Err(error) => {
                            let _ = events.send(VoiceEvent::PreviewFailed {
                                generation,
                                message: error.to_string(),
                            });
                        }
                    }
                    ctx.request_repaint();
                }
            }
            Ok(VoiceCommand::StopPreview { generation }) => {
                if preview_generation == Some(generation) {
                    if let Some(active) = preview.take() {
                        active.stop();
                    }
                    preview_generation = None;
                    if let Ok(mut current) = snapshot.lock() {
                        current.preview_playing = false;
                    }
                }
                ctx.request_repaint();
            }
            Ok(VoiceCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            // Never ignore a Start silently: the UI waits for an outcome. A stale
            // generation's Failed is discarded by the UI's own binding check.
            Ok(VoiceCommand::Start { generation }) => {
                let _ = events.send(VoiceEvent::Failed {
                    generation,
                    message: "Micro đang bận với bản ghi khác; hãy thử lại".into(),
                });
                ctx.request_repaint();
            }
        }

        if recording_generation
            .is_some_and(|generation| !preview_generation_is_active(&active_generation, generation))
        {
            if let Some(active) = recorder.take() {
                active.cancel();
            }
            recording_generation = None;
            if let Ok(mut current) = snapshot.lock() {
                *current = VoiceSnapshot::default();
            }
            ctx.request_repaint();
        }

        if preview_generation
            .is_some_and(|generation| !preview_generation_is_active(&active_generation, generation))
        {
            if let Some(active) = preview.take() {
                active.stop();
            }
            preview_generation = None;
            if let Ok(mut current) = snapshot.lock() {
                current.preview_playing = false;
            }
            ctx.request_repaint();
        }

        if preview.as_ref().is_some_and(|active| !active.is_playing()) {
            preview.take();
            preview_generation = None;
            if let Ok(mut current) = snapshot.lock() {
                current.preview_playing = false;
            }
            ctx.request_repaint();
        }

        if let Some(active) = recorder.as_ref() {
            let (duration, waveform, sample_count, limit_reached) = active.snapshot();
            if limit_reached {
                if let Some(active) = recorder.take() {
                    let (samples, sample_rate, duration, limit_reached) = active.finish();
                    if let Ok(mut current) = snapshot.lock() {
                        *current = VoiceSnapshot::default();
                    }
                    let _ = events.send(VoiceEvent::Finished {
                        generation: recording_generation.unwrap_or_default(),
                        samples: Arc::from(samples),
                        sample_rate,
                        duration,
                        limit_reached,
                        send_after_finish: false,
                    });
                    recording_generation = None;
                    ctx.request_repaint();
                }
            } else if let Ok(mut current) = snapshot.lock() {
                current.recording = true;
                current.duration = duration;
                current.waveform = waveform;
                current.recorded_bytes = sample_count * std::mem::size_of::<f32>();
                current.preview_playing =
                    preview.as_ref().is_some_and(VoicePreviewPlayer::is_playing);
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        } else if let Ok(mut current) = snapshot.lock() {
            current.preview_playing = preview.as_ref().is_some_and(VoicePreviewPlayer::is_playing);
        }
    }

    if let Some(active) = recorder.take() {
        active.cancel();
    }
    if let Some(active) = preview.take() {
        active.stop();
    }
    if let Ok(mut current) = snapshot.lock() {
        *current = VoiceSnapshot::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_limit_is_bounded_by_duration_and_sample_storage() {
        assert!(!recording_limit_reached(
            VOICE_MAX_DURATION - Duration::from_millis(1),
            0
        ));
        assert!(recording_limit_reached(VOICE_MAX_DURATION, 0));
        assert!(!recording_limit_reached(
            Duration::ZERO,
            VOICE_MAX_SAMPLES - 1
        ));
        assert!(recording_limit_reached(Duration::ZERO, VOICE_MAX_SAMPLES));
        assert_eq!(VOICE_MAX_BYTES, 64 * 1024 * 1024);
    }

    #[test]
    fn cancellation_generation_invalidates_recording_when_command_queue_is_full() {
        let (commands, receiver) = mpsc::sync_channel(1);
        assert!(commands
            .try_send(VoiceCommand::Start { generation: 0 })
            .is_ok());
        assert!(commands
            .try_send(VoiceCommand::Start { generation: 1 })
            .is_err());
        let active_generation = AtomicU64::new(0);

        invalidate_recording_generation(&active_generation, 0);

        assert_eq!(active_generation.load(Ordering::Acquire), 1);
        assert!(matches!(
            receiver.try_recv(),
            Ok(VoiceCommand::Start { generation: 0 })
        ));
    }

    #[test]
    fn queued_stale_preview_is_rejected_after_cancel_cannot_enter_full_queue() {
        let (commands, receiver) = mpsc::sync_channel(1);
        assert!(commands
            .try_send(VoiceCommand::PlayPreview {
                generation: 4,
                samples: Arc::from(vec![0.25, -0.25]),
                sample_rate: VOICE_SAMPLE_RATE,
            })
            .is_ok());
        assert!(commands
            .try_send(VoiceCommand::Cancel { generation: 4 })
            .is_err());
        let active_generation = AtomicU64::new(4);

        invalidate_recording_generation(&active_generation, 4);

        let Ok(VoiceCommand::PlayPreview { generation, .. }) = receiver.try_recv() else {
            panic!("expected the queued preview command");
        };
        assert!(!preview_generation_is_active(
            &active_generation,
            generation
        ));
    }

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
