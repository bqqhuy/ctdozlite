//! Windows Media Foundation capture and H.264 codecs. No external executable.
//! COM interfaces stay on media threads; the UI only receives owned image data.
use anyhow::{Context, Result};
use std::{
    mem::ManuallyDrop,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender},
        Arc,
    },
    time::Duration,
};
use windows::{
    core::{Interface, GUID},
    Win32::{Media::MediaFoundation::*, System::Com::*},
};

fn media_components() -> Result<()> {
    static READY: std::sync::OnceLock<std::result::Result<(), String>> = std::sync::OnceLock::new();
    let result = READY.get_or_init(|| {
        #[link(name = "kernel32")]
        extern "system" {
            fn LoadLibraryExW(
                name: *const u16,
                file: *mut std::ffi::c_void,
                flags: u32,
            ) -> *mut std::ffi::c_void;
        }
        for name in ["mfplat.dll", "mfreadwrite.dll", "mf.dll"] {
            let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            // System32 only, with modules held for process lifetime to back delay imports.
            if unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), 0x800) }.is_null() {
                return Err(format!(
                    "Thiếu {name}; Windows N cần cài Media Feature Pack rồi khởi động lại ứng dụng"
                ));
            }
        }
        Ok(())
    });
    result
        .as_ref()
        .map(|_| ())
        .map_err(|reason| anyhow::anyhow!("{reason}"))
}
struct Runtime;
impl Runtime {
    fn new() -> Result<Self> {
        media_components()?;
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            if let Err(e) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                CoUninitialize();
                return Err(e.into());
            }
        }
        Ok(Self)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}
unsafe fn attributes() -> Result<IMFAttributes> {
    let mut attrs = None;
    MFCreateAttributes(&mut attrs, 4)?;
    attrs.context("MFCreateAttributes returned null")
}
unsafe fn devices() -> Result<Vec<(String, IMFActivate)>> {
    let attrs = attributes()?;
    attrs.SetGUID(
        &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
        &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    )?;
    let mut ptr = std::ptr::null_mut();
    let mut count = 0;
    MFEnumDeviceSources(&attrs, &mut ptr, &mut count)?;
    let mut result = Vec::new();
    // MF owns the array allocation; each entry owns one COM reference.
    if !ptr.is_null() {
        for item in std::slice::from_raw_parts_mut(ptr, count as usize) {
            if let Some(device) = item.take() {
                let len = device
                    .GetStringLength(&MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
                    .unwrap_or(0);
                let mut name = vec![0; len as usize + 1];
                if device
                    .GetString(&MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, &mut name, None)
                    .is_ok()
                {
                    result.push((String::from_utf16_lossy(&name[..len as usize]), device));
                }
            }
        }
        CoTaskMemFree(Some(ptr.cast()));
    }
    Ok(result)
}
pub fn camera_names() -> Result<Vec<String>> {
    // CPAL initializes its enumeration thread as STA. Keep Media Foundation in
    // a fresh MTA rather than failing with RPC_E_CHANGED_MODE after listing mics.
    std::thread::spawn(|| -> Result<Vec<String>> {
        let _runtime = Runtime::new()?;
        // SAFETY: this fresh media thread owns the MTA runtime and COM activations.
        unsafe {
            let mut names: Vec<String> = devices()?.into_iter().map(|(name, _)| name).collect();
            names.sort_by_key(|name| crate::video_frame::camera_preference(name));
            Ok(names)
        }
    })
    .join()
    .map_err(|_| anyhow::anyhow!("Camera enumeration thread failed"))?
}

unsafe fn media_type(subtype: &GUID, size: Option<(u32, u32)>) -> Result<IMFMediaType> {
    let mt = MFCreateMediaType()?;
    mt.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
    mt.SetGUID(&MF_MT_SUBTYPE, subtype)?;
    if let Some((w, h)) = size {
        mt.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64)?;
        mt.SetUINT64(&MF_MT_FRAME_RATE, (15u64 << 32) | 1)?;
        mt.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        mt.SetUINT32(&MF_MT_INTERLACE_MODE, 2)?; // progressive
    }
    Ok(mt)
}
unsafe fn dimensions(mt: &IMFMediaType) -> Result<(usize, usize)> {
    let size = mt.GetUINT64(&MF_MT_FRAME_SIZE)?;
    let (w, h) = ((size >> 32) as usize, (size & 0xffff_ffff) as usize);
    anyhow::ensure!(
        w > 0
            && h > 0
            && w <= 1920
            && h <= 1920
            && w * h <= 1920 * 1080
            && w % 2 == 0
            && h % 2 == 0,
        "Unsupported video size {w}x{h}"
    );
    Ok((w, h))
}
unsafe fn sample(bytes: &[u8], time: i64) -> Result<IMFSample> {
    anyhow::ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "Video packet exceeds limit"
    );
    let buffer = MFCreateMemoryBuffer(bytes.len() as u32)?;
    let mut ptr = std::ptr::null_mut();
    buffer.Lock(&mut ptr, None, None)?;
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
    buffer.Unlock()?;
    buffer.SetCurrentLength(bytes.len() as u32)?;
    let sample = MFCreateSample()?;
    sample.AddBuffer(&buffer)?;
    sample.SetSampleTime(time)?;
    sample.SetSampleDuration(10_000_000 / 15)?;
    Ok(sample)
}
unsafe fn bytes(sample: &IMFSample) -> Result<Vec<u8>> {
    let buffer = sample.ConvertToContiguousBuffer()?;
    let mut ptr = std::ptr::null_mut();
    let mut len = 0;
    buffer.Lock(&mut ptr, None, Some(&mut len))?;
    let result = if len == 0 {
        Ok(Vec::new())
    } else if len <= 16 * 1024 * 1024 {
        Ok(std::slice::from_raw_parts(ptr, len as usize).to_vec())
    } else {
        Err(anyhow::anyhow!("Decoded video exceeds limit"))
    };
    buffer.Unlock()?;
    result
}

// Camera/decoder surfaces may have a hardware pitch that differs from the
// media type's default stride. IMF2DBuffer supplies a packed contiguous copy.
unsafe fn nv12_sample(
    sample: &IMFSample,
    w: usize,
    h: usize,
    default_stride: usize,
) -> Result<(Vec<u8>, usize)> {
    let buffer = sample.ConvertToContiguousBuffer()?;
    if let Ok(planar) = buffer.cast::<IMF2DBuffer>() {
        let size = planar.GetContiguousLength()? as usize;
        anyhow::ensure!(size <= 16 * 1024 * 1024, "Camera surface exceeds limit");
        let mut packed = vec![0; size];
        planar.ContiguousCopyTo(&mut packed)?;
        crate::video_frame::validate_layout(w, h, w, packed.len())?;
        Ok((packed, w))
    } else {
        let data = bytes(sample)?;
        let stride = if data.len() == w * h * 3 / 2 {
            w
        } else {
            default_stride
        };
        crate::video_frame::validate_layout(w, h, stride, data.len())?;
        Ok((data, stride))
    }
}

struct Codec {
    transform: IMFTransform,
    decoding: bool,
}
impl Codec {
    unsafe fn encoder(w: u32, h: u32) -> Result<Self> {
        let transform: IMFTransform =
            CoCreateInstance(&CMSH264EncoderMFT, None, CLSCTX_INPROC_SERVER)?;
        let output = media_type(&MFVideoFormat_H264, Some((w, h)))?;
        output.SetUINT32(&MF_MT_AVG_BITRATE, 350_000)?;
        output.SetUINT32(&MF_MT_MPEG2_PROFILE, 66)?; // baseline, no B frames
        transform.SetOutputType(0, &output, 0)?;
        transform.SetInputType(0, &media_type(&MFVideoFormat_NV12, Some((w, h)))?, 0)?;
        let codec = Self {
            transform,
            decoding: false,
        };
        codec.start()?;
        Ok(codec)
    }
    unsafe fn decoder() -> Result<Self> {
        let transform: IMFTransform =
            CoCreateInstance(&CMSH264DecoderMFT, None, CLSCTX_INPROC_SERVER)?;
        transform.SetInputType(0, &media_type(&MFVideoFormat_H264, None)?, 0)?;
        let codec = Self {
            transform,
            decoding: true,
        };
        // Decoder output becomes known after SPS/PPS arrive.
        if codec.select_output().is_ok() {
            codec.start()?;
        }
        Ok(codec)
    }
    unsafe fn start(&self) -> Result<()> {
        self.transform
            .ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
        self.transform
            .ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        Ok(())
    }
    unsafe fn select_output(&self) -> Result<()> {
        for i in 0..64 {
            let mt = self.transform.GetOutputAvailableType(0, i)?;
            if mt.GetGUID(&MF_MT_SUBTYPE)? == MFVideoFormat_NV12 {
                dimensions(&mt)?;
                self.transform.SetOutputType(0, &mt, 0)?;
                return Ok(());
            }
        }
        anyhow::bail!("H.264 decoder has no NV12 output")
    }
    unsafe fn input(&self, sample: &IMFSample) -> Result<()> {
        self.transform.ProcessInput(0, sample, 0)?;
        Ok(())
    }
    unsafe fn output(&self) -> Result<Option<IMFSample>> {
        for _ in 0..3 {
            let info = self.transform.GetOutputStreamInfo(0)?;
            anyhow::ensure!(
                info.cbSize <= 16 * 1024 * 1024,
                "Codec output exceeds limit"
            );
            let allocated = if info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0 {
                None
            } else {
                let sample = MFCreateSample()?;
                sample.AddBuffer(&MFCreateMemoryBuffer(info.cbSize.max(1024 * 1024))?)?;
                Some(sample)
            };
            let mut output = [MFT_OUTPUT_DATA_BUFFER {
                dwStreamID: 0,
                pSample: ManuallyDrop::new(allocated),
                dwStatus: 0,
                pEvents: ManuallyDrop::new(None),
            }];
            let mut status = 0;
            let result = self.transform.ProcessOutput(0, &mut output, &mut status);
            let sample = ManuallyDrop::take(&mut output[0].pSample);
            drop(ManuallyDrop::take(&mut output[0].pEvents));
            match result {
                Ok(()) => return Ok(sample),
                Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(None),
                Err(e) if self.decoding && e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                    self.select_output()?
                }
                Err(e) => return Err(e.into()),
            }
        }
        anyhow::bail!("Repeated decoder format changes")
    }
}

/// Convert limited-range BT.601 NV12 to RGBA, respecting the negotiated stride.
fn nv12_rgba(input: &[u8], w: usize, h: usize, stride: usize) -> Result<Vec<u8>> {
    anyhow::ensure!(
        stride >= w && stride <= 8192 && input.len() >= stride * h * 3 / 2,
        "Truncated NV12 frame"
    );
    let mut rgba = vec![0; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let yy = (input[y * stride + x] as i32 - 16).max(0);
            let uv = stride * h + (y / 2) * stride + (x & !1);
            let u = input[uv] as i32 - 128;
            let v = input[uv + 1] as i32 - 128;
            let p = (y * w + x) * 4;
            rgba[p] = ((298 * yy + 409 * v + 128) >> 8).clamp(0, 255) as u8;
            rgba[p + 1] = ((298 * yy - 100 * u - 208 * v + 128) >> 8).clamp(0, 255) as u8;
            rgba[p + 2] = ((298 * yy + 516 * u + 128) >> 8).clamp(0, 255) as u8;
            rgba[p + 3] = 255;
        }
    }
    Ok(rgba)
}
#[allow(clippy::too_many_arguments)]
fn show_frame(
    data: &[u8],
    w: usize,
    h: usize,
    stride: usize,
    key: &str,
    tx: &std::sync::mpsc::Sender<crate::gui::GuiEvent>,
    ctx: &eframe::egui::Context,
    generation: u64,
) -> Result<()> {
    if !crate::video_preview::claim(generation, key) {
        return Ok(());
    }
    let result = (|| -> Result<()> {
        let rgba = nv12_rgba(data, w, h, stride)?;
        tx.send(crate::gui::GuiEvent::VideoFrameReady {
            generation,
            key: key.into(),
            image: eframe::egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba),
        })?;
        ctx.request_repaint();
        Ok(())
    })();
    if result.is_err() {
        crate::video_preview::consumed(generation, key);
    }
    result
}
pub fn readiness() -> Result<()> {
    let _runtime = Runtime::new()?;
    unsafe {
        let _encoder = Codec::encoder(320, 240)?;
        let _decoder = Codec::decoder()?;
    }
    Ok(())
}
pub fn decode(
    stop: Arc<AtomicBool>,
    rx: Receiver<Vec<u8>>,
    tx: std::sync::mpsc::Sender<crate::gui::GuiEvent>,
    ctx: eframe::egui::Context,
    synced: Arc<AtomicBool>,
    generation: u64,
) -> Result<()> {
    let _runtime = Runtime::new()?;
    unsafe {
        let mut codec = Codec::decoder()?;
        let mut timestamp = 0;
        while !stop.load(Ordering::Relaxed) {
            let packet = match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(p) => p,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            if packet.is_empty() {
                continue;
            }
            let result = (|| -> Result<()> {
                codec.input(&sample(&packet, timestamp)?)?;
                timestamp += 10_000_000 / 15;
                while let Some(output) = codec.output()? {
                    let mt = codec.transform.GetOutputCurrentType(0)?;
                    let (w, h) = dimensions(&mt)?;
                    let stride = mt.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(w as u32) as usize;
                    if crate::tray::previews_visible() {
                        let (raw, stride) = nv12_sample(&output, w, h, stride)?;
                        show_frame(
                            &raw,
                            w,
                            h,
                            stride,
                            "call_remote_video",
                            &tx,
                            &ctx,
                            generation,
                        )?;
                    }
                    synced.store(true, Ordering::Relaxed);
                }
                Ok(())
            })();
            if let Err(e) = result {
                synced.store(false, Ordering::Relaxed);
                crate::voip_log::line("VDEC", format!("Native decoder reset: {e}"));
                codec = Codec::decoder()?;
            }
        }
    }
    Ok(())
}
// Media Foundation media sources are free-threaded. Both capture and watcher
// enter the process MTA; no STA interface is moved across apartments.
struct MtaSource(IMFMediaSource);
unsafe impl Send for MtaSource {}
impl MtaSource {
    unsafe fn shutdown(&self) {
        let _ = self.0.Shutdown();
    }
}
struct Source(IMFMediaSource);
impl Drop for Source {
    fn drop(&mut self) {
        unsafe {
            let _ = self.0.Shutdown();
        }
    }
}

// SourceReader conversion can resize NV12 without requiring a camera-native
// 320x240@15 mode. If that fails, decode a bounded advertised native mode and
// normalize its padded NV12 planes ourselves before the encoder.
unsafe fn select_camera_output(reader: &IMFSourceReader, stream: u32) -> Result<()> {
    let requested = media_type(&MFVideoFormat_NV12, Some((320, 240)))?;
    requested.DeleteItem(&MF_MT_FRAME_RATE)?;
    if reader.SetCurrentMediaType(stream, None, &requested).is_ok() {
        return Ok(());
    }
    let mut candidates = Vec::new();
    for index in 0..128 {
        let Ok(native) = reader.GetNativeMediaType(stream, index) else {
            break;
        };
        if let Ok((w, h)) = dimensions(&native) {
            let rate = native.GetUINT64(&MF_MT_FRAME_RATE).unwrap_or(0);
            let (num, den) = (rate >> 32, rate & 0xffff_ffff);
            let slow = den == 0 || num < 15 * den;
            candidates.push(((slow, (w * h).abs_diff(320 * 240)), native));
        }
    }
    candidates.sort_by_key(|(score, _)| *score);
    for (_, native) in candidates {
        if reader.SetCurrentMediaType(stream, None, &native).is_err() {
            continue;
        }
        if reader
            .SetCurrentMediaType(stream, None, &media_type(&MFVideoFormat_NV12, None)?)
            .is_ok()
        {
            dimensions(&reader.GetCurrentMediaType(stream)?)?;
            return Ok(());
        }
    }
    anyhow::bail!("Camera không có định dạng NV12/chuyển đổi phù hợp; thử camera khác hoặc cập nhật driver Huawei")
}

// An automatic choice may encounter an IR/unsupported camera before the color
// camera. Explicit user selections never silently switch to another device.
unsafe fn open_camera(desired: &str) -> Result<(Source, IMFSourceReader)> {
    let mut all = devices()?;
    if desired.is_empty() {
        all.sort_by_key(|(name, _)| crate::video_frame::camera_preference(name));
    }
    let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    let mut last_error =
        anyhow::anyhow!("Không tìm thấy camera; kiểm tra quyền Camera cho desktop apps");
    for (_, device) in all
        .into_iter()
        .filter(|(name, _)| desired.is_empty() || name == desired)
    {
        let result = (|| -> Result<(Source, IMFSourceReader)> {
            let source = Source(device.ActivateObject::<IMFMediaSource>().context(
                "Không mở được camera; kiểm tra quyền Camera hoặc ứng dụng đang chiếm camera",
            )?);
            let attrs = attributes()?;
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;
            let reader = MFCreateSourceReaderFromMediaSource(&source.0, &attrs)?;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(stream, true)?;
            select_camera_output(&reader, stream)?;
            Ok((source, reader))
        })();
        match result {
            Ok(camera) => return Ok(camera),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

pub fn capture(
    stop: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    keyframe: Arc<AtomicBool>,
    au_tx: SyncSender<Vec<u8>>,
    tx: Option<std::sync::mpsc::Sender<crate::gui::GuiEvent>>,
    ctx: Option<eframe::egui::Context>,
    generation: u64,
) -> Result<()> {
    let _runtime = Runtime::new()?;
    while !stop.load(Ordering::Relaxed) {
        if !enabled.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(80));
            continue;
        }
        let desired = crate::media::selected_camera_device();
        unsafe {
            let (source, reader) = open_camera(&desired)?;
            let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            let mut encoder = Codec::encoder(320, 240)?;
            // Shutdown interrupts ReadSample even when a camera/driver stops producing.
            let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
            let watch_source = MtaSource(source.0.clone());
            let watch_stop = stop.clone();
            let watch_enabled = enabled.clone();
            let watch_name = desired.clone();
            let capture_start = std::time::Instant::now();
            let last_sample = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let watch_last_sample = last_sample.clone();
            let stalled = Arc::new(AtomicBool::new(false));
            let watch_stalled = stalled.clone();
            let watcher = std::thread::spawn(move || {
                let Ok(_runtime) = Runtime::new() else {
                    return;
                };
                loop {
                    if done_rx.recv_timeout(Duration::from_millis(100))
                        != Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                    {
                        break;
                    }
                    let elapsed = capture_start.elapsed().as_millis() as u64;
                    if elapsed.saturating_sub(watch_last_sample.load(Ordering::Relaxed)) > 5_000 {
                        watch_stalled.store(true, Ordering::Relaxed);
                        watch_source.shutdown();
                        break;
                    }
                    if watch_stop.load(Ordering::Relaxed)
                        || !watch_enabled.load(Ordering::Relaxed)
                        || crate::media::selected_camera_device() != watch_name
                    {
                        watch_source.shutdown();
                        break;
                    }
                }
            });
            let result = (|| -> Result<()> {
                let mut frame_number = 0i64;
                let mut cadence = crate::video_frame::CaptureCadence::default();
                while !stop.load(Ordering::Relaxed)
                    && enabled.load(Ordering::Relaxed)
                    && crate::media::selected_camera_device() == desired
                {
                    let mut sample = None;
                    let mut flags = 0;
                    let mut source_time = 0;
                    let read = reader.ReadSample(
                        stream,
                        0,
                        None,
                        Some(&mut flags),
                        Some(&mut source_time),
                        Some(&mut sample),
                    );
                    if stop.load(Ordering::Relaxed)
                        || !enabled.load(Ordering::Relaxed)
                        || crate::media::selected_camera_device() != desired
                    {
                        break;
                    }
                    read?;
                    anyhow::ensure!(
                        flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 == 0,
                        "Camera disconnected"
                    );
                    let Some(sample) = sample else {
                        continue;
                    };
                    last_sample.store(
                        capture_start.elapsed().as_millis() as u64,
                        Ordering::Relaxed,
                    );
                    if !cadence.accepts(source_time) {
                        continue;
                    }
                    let mt = reader.GetCurrentMediaType(stream)?;
                    let (w, h) = dimensions(&mt)?;
                    let stride = mt.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(w as u32) as usize;
                    let (raw, stride) = nv12_sample(&sample, w, h, stride)?;
                    let normalized = crate::video_frame::normalize_nv12(&raw, w, h, stride)?;
                    let encoded_sample =
                        self::sample(&normalized, frame_number * (10_000_000 / 15))?;
                    frame_number += 1;
                    // Reinitializing guarantees an IDR with SPS/PPS on peer request.
                    if keyframe.swap(false, Ordering::Relaxed) || frame_number % 30 == 0 {
                        encoder = Codec::encoder(320, 240)?;
                    }
                    if let (Some(tx), Some(ctx)) = (&tx, &ctx) {
                        show_frame(
                            &normalized,
                            320,
                            240,
                            320,
                            "call_local_video",
                            tx,
                            ctx,
                            generation,
                        )?;
                    }
                    encoder.input(&encoded_sample)?;
                    while let Some(output) = encoder.output()? {
                        let packet = bytes(&output)?;
                        match au_tx.try_send(packet) {
                            Ok(()) => (),
                            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                                keyframe.store(true, Ordering::Relaxed);
                            }
                            Err(_) => return Ok(()),
                        }
                    }
                }
                Ok(())
            })();
            drop(done_tx);
            let _ = watcher.join();
            anyhow::ensure!(
                !stalled.load(Ordering::Relaxed),
                "Camera không trả khung hình trong 5 giây; thử tắt/bật camera hoặc đổi thiết bị"
            );
            result?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_dimensional_nv12_surface_is_copied_without_hardware_padding() {
        let _runtime = Runtime::new().unwrap();
        // SAFETY: the test owns an MTA runtime and the sample/buffer references.
        unsafe {
            let (w, h) = (322usize, 242usize);
            let buffer =
                MFCreate2DMediaBuffer(w as u32, h as u32, u32::from_le_bytes(*b"NV12"), false)
                    .unwrap();
            let planar = buffer.cast::<IMF2DBuffer>().unwrap();
            let mut packed = vec![100; w * h * 3 / 2];
            packed[w * h..].fill(128);
            planar.ContiguousCopyFrom(&packed).unwrap();
            let sample = MFCreateSample().unwrap();
            sample.AddBuffer(&buffer).unwrap();
            let (actual, stride) = nv12_sample(&sample, w, h, 384).unwrap();
            assert_eq!(stride, w);
            assert_eq!(actual, packed);
        }
    }
    #[test]
    fn nv12_stride_and_truncation() {
        let black = vec![16, 16, 0, 0, 16, 16, 0, 0, 128, 128, 0, 0];
        assert_eq!(
            nv12_rgba(&black, 2, 2, 4).unwrap(),
            [0, 0, 0, 255].repeat(4)
        );
        assert!(nv12_rgba(&black[..8], 2, 2, 4).is_err());
    }
    #[test]
    fn camera_enumeration_isolated_from_sta_caller() -> Result<()> {
        std::thread::spawn(|| -> Result<()> {
            unsafe {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            }
            let result = camera_names();
            unsafe {
                CoUninitialize();
            }
            result.map(|_| ())
        })
        .join()
        .unwrap()
    }
    #[test]
    fn native_h264_roundtrip() -> Result<()> {
        let _runtime = Runtime::new()?;
        unsafe {
            let encoder = Codec::encoder(320, 240)?;
            let decoder = Codec::decoder()?;
            let mut raw = vec![128; 320 * 240 * 3 / 2];
            raw[..320 * 240].fill(90);
            let mut decoded = 0;
            for n in 0..30 {
                encoder.input(&sample(&raw, n * (10_000_000 / 15))?)?;
                while let Some(encoded) = encoder.output()? {
                    let packet = bytes(&encoded)?;
                    anyhow::ensure!(
                        packet.windows(3).any(|s| s == [0, 0, 1]),
                        "Encoder must produce Annex-B"
                    );
                    decoder.input(&sample(&packet, n * (10_000_000 / 15))?)?;
                    while let Some(frame) = decoder.output()? {
                        let (w, h) = dimensions(&decoder.transform.GetOutputCurrentType(0)?)?;
                        assert_eq!((w, h), (320, 240));
                        assert!(bytes(&frame)?.len() >= w * h * 3 / 2);
                        decoded += 1;
                    }
                }
            }
            anyhow::ensure!(decoded > 0, "Native H.264 pipeline produced no frames");
        }
        Ok(())
    }
}
