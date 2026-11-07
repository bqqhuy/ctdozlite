//! Checked DIB decoding independent of borrowed Win32 clipboard handles.
use anyhow::{ensure, Context, Result};

const MAX_PIXELS: usize = 16 * 1024 * 1024;
pub const MAX_RAW: usize = 80 * 1024 * 1024;
fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let b = bytes
        .get(offset..offset + 4)
        .context("Clipboard header bị cắt")?;
    Ok(u32::from_le_bytes(b.try_into()?))
}
fn channel(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 255;
    }
    let shift = mask.trailing_zeros();
    let maximum = mask >> shift;
    ((((value & mask) >> shift) as u64 * 255 + maximum as u64 / 2) / maximum as u64) as u8
}
pub fn decode_dib(bytes: &[u8]) -> Result<image::RgbaImage> {
    ensure!(
        bytes.len() <= MAX_RAW && bytes.len() >= 40,
        "Clipboard bitmap quá lớn hoặc không hợp lệ"
    );
    let header = word(bytes, 0)? as usize;
    ensure!(
        matches!(header, 40 | 52 | 56 | 108 | 124) && header <= bytes.len(),
        "Không hỗ trợ header clipboard"
    );
    let width = word(bytes, 4)? as i32;
    let signed_height = word(bytes, 8)? as i32;
    ensure!(
        width > 0 && width <= 8192 && signed_height != 0 && signed_height.unsigned_abs() <= 8192,
        "Kích thước clipboard không hợp lệ"
    );
    let width = width as usize;
    let height = signed_height.unsigned_abs() as usize;
    ensure!(
        width.checked_mul(height).is_some_and(|n| n <= MAX_PIXELS),
        "Ảnh clipboard quá nhiều điểm ảnh"
    );
    let planes = u16::from_le_bytes(bytes[12..14].try_into()?);
    let bits = u16::from_le_bytes(bytes[14..16].try_into()?);
    let compression = word(bytes, 16)?;
    ensure!(
        planes == 1 && matches!(bits, 24 | 32) && matches!(compression, 0 | 3 | 6),
        "Không hỗ trợ định dạng clipboard"
    );
    ensure!(bits == 32 || compression == 0, "Không hỗ trợ bitmap nén");
    let masks = if compression == 0 {
        [
            0xff0000,
            0xff00,
            0xff,
            if bits == 32 { 0xff000000 } else { 0 },
        ]
    } else {
        [
            word(bytes, 40)?,
            word(bytes, 44)?,
            word(bytes, 48)?,
            if compression == 6 || header >= 56 {
                word(bytes, 52)?
            } else {
                0
            },
        ]
    };
    for (i, mask) in masks.iter().enumerate() {
        ensure!(
            (i == 3 || *mask != 0)
                && (*mask == 0
                    || (mask >> mask.trailing_zeros())
                        .wrapping_add(1)
                        .is_power_of_two()),
            "Masque clipboard không hợp lệ"
        );
        for other in masks.iter().take(i) {
            ensure!(mask & other == 0, "Masque clipboard chồng nhau");
        }
    }
    let offset = header
        + if header == 40 && compression != 0 {
            if compression == 6 {
                16
            } else {
                12
            }
        } else {
            0
        };
    let bpp = bits as usize / 8;
    let stride = width
        .checked_mul(bpp)
        .and_then(|n| n.checked_add(3))
        .context("Clipboard stride tràn")?
        & !3;
    let end = stride
        .checked_mul(height)
        .and_then(|n| n.checked_add(offset))
        .context("Clipboard bitmap tràn")?;
    ensure!(end <= bytes.len(), "Clipboard bitmap bị cắt");
    let opaque_rgb = compression == 0
        && (bpp == 3
            || !(0..height)
                .any(|y| (0..width).any(|x| bytes[offset + y * stride + x * bpp + 3] != 0)));
    let mut image = image::RgbaImage::new(width as u32, height as u32);
    for y in 0..height {
        let source = if signed_height < 0 { y } else { height - 1 - y };
        for x in 0..width {
            let p = offset + source * stride + x * bpp;
            let value = if bpp == 4 {
                word(bytes, p)?
            } else {
                u32::from_le_bytes([bytes[p], bytes[p + 1], bytes[p + 2], 0])
            };
            image.put_pixel(
                x as u32,
                y as u32,
                image::Rgba([
                    channel(value, masks[0]),
                    channel(value, masks[1]),
                    channel(value, masks[2]),
                    if opaque_rgb {
                        255
                    } else {
                        channel(value, masks[3])
                    },
                ]),
            );
        }
    }
    Ok(image)
}

/// Rasterize Office vector clipboard data after all borrowed clipboard handles are released.
#[cfg(windows)]
fn render_emf(bytes: &[u8]) -> Result<image::RgbaImage> {
    use windows::Win32::{
        Foundation::{HANDLE, RECT},
        Graphics::Gdi::*,
    };
    struct Resources {
        emf: HENHMETAFILE,
        dc: HDC,
        bitmap: HBITMAP,
        old: HGDIOBJ,
    }
    impl Drop for Resources {
        fn drop(&mut self) {
            // SAFETY: these resources were created and selected only on this worker thread.
            unsafe {
                if self.old.0 != 0 {
                    SelectObject(self.dc, self.old);
                }
                if self.bitmap.0 != 0 {
                    DeleteObject(self.bitmap);
                }
                if self.dc.0 != 0 {
                    DeleteDC(self.dc);
                }
                if self.emf.0 != 0 {
                    DeleteEnhMetaFile(self.emf);
                }
            }
        }
    }
    ensure!(bytes.len() <= MAX_RAW, "Ảnh vector clipboard quá lớn");
    // SAFETY: the owned byte slice outlives decoding; all GDI handles are RAII-owned.
    unsafe {
        let mut r = Resources {
            emf: SetEnhMetaFileBits(bytes),
            dc: HDC(0),
            bitmap: HBITMAP(0),
            old: HGDIOBJ(0),
        };
        ensure!(r.emf.0 != 0, "Không đọc được ảnh vector clipboard");
        let mut header = ENHMETAHEADER::default();
        ensure!(
            GetEnhMetaFileHeader(
                r.emf,
                std::mem::size_of::<ENHMETAHEADER>() as u32,
                Some(&mut header)
            ) > 0,
            "Thiếu kích thước ảnh vector"
        );
        // Office frame coordinates are hundredths of millimetres. 144 DPI keeps cell text sharp.
        let dimension = |start: i32, end: i32, fallback: i64| {
            let frame = end as i64 - start as i64;
            if frame > 0 {
                (frame * 144 + 1270) / 2540
            } else {
                fallback
            }
        };
        let width = dimension(
            header.rclFrame.left,
            header.rclFrame.right,
            header.rclBounds.right as i64 - header.rclBounds.left as i64,
        );
        let height = dimension(
            header.rclFrame.top,
            header.rclFrame.bottom,
            header.rclBounds.bottom as i64 - header.rclBounds.top as i64,
        );
        ensure!(
            (1..=8192).contains(&width)
                && (1..=8192).contains(&height)
                && width * height <= MAX_PIXELS as i64,
            "Vùng Excel quá lớn; hãy sao chép vùng nhỏ hơn"
        );
        r.dc = CreateCompatibleDC(HDC(0));
        ensure!(r.dc.0 != 0, "Không tạo được vùng vẽ clipboard");
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = std::ptr::null_mut();
        r.bitmap = CreateDIBSection(r.dc, &info, DIB_RGB_COLORS, &mut pixels, HANDLE(0), 0)?;
        ensure!(!pixels.is_null(), "Không tạo được bitmap clipboard");
        r.old = SelectObject(r.dc, r.bitmap);
        let rect = RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        // Initialize opaque white because spreadsheet vector formats generally have no background fill.
        let count = width as usize * height as usize * 4;
        std::ptr::write_bytes(pixels.cast::<u8>(), 255, count);
        ensure!(
            PlayEnhMetaFile(r.dc, r.emf, &rect).as_bool(),
            "Không vẽ được vùng Excel"
        );
        GdiFlush();
        let bgra = std::slice::from_raw_parts(pixels.cast::<u8>(), count);
        let mut rgba = Vec::with_capacity(count);
        for pixel in bgra.as_chunks::<4>().0 {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }
        image::RgbaImage::from_raw(width as u32, height as u32, rgba)
            .context("Bitmap clipboard không hợp lệ")
    }
}

#[cfg(windows)]
pub fn read_windows() -> Result<Option<image::RgbaImage>> {
    use std::ffi::c_void;
    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(hwnd: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut c_void;
        fn IsClipboardFormatAvailable(format: u32) -> i32;
        fn RegisterClipboardFormatW(name: *const u16) -> u32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalLock(handle: *mut c_void) -> *const u8;
        fn GlobalUnlock(handle: *mut c_void) -> i32;
        fn GlobalSize(handle: *mut c_void) -> usize;
    }
    #[link(name = "shell32")]
    extern "system" {
        fn DragQueryFileW(handle: *mut c_void, index: u32, name: *mut u16, length: u32) -> u32;
    }
    struct Clipboard;
    impl Drop for Clipboard {
        fn drop(&mut self) {
            // SAFETY: this guard owns this thread's successfully opened clipboard.
            unsafe {
                CloseClipboard();
            }
        }
    }
    struct Lock(*mut c_void);
    impl Drop for Lock {
        fn drop(&mut self) {
            // SAFETY: the guard owns exactly one successful GlobalLock.
            unsafe {
                GlobalUnlock(self.0);
            }
        }
    }
    enum Source {
        Png(Vec<u8>),
        Dib(Vec<u8>),
        Emf(Vec<u8>),
        File(std::path::PathBuf),
    }
    let source = {
        let mut opened = false;
        for _ in 0..8 {
            // SAFETY: no owner HWND is required for read-only clipboard access.
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        ensure!(opened, "Clipboard đang được ứng dụng khác sử dụng");
        let _clipboard = Clipboard;
        let mut source = None;
        for (format, png) in [
            ("PNG", true),
            ("image/png", true),
            ("DIBV5", false),
            ("DIB", false),
        ] {
            let wide: Vec<_> = format.encode_utf16().chain(Some(0)).collect();
            // SAFETY: static numeric IDs or a NUL-terminated owned UTF-16 buffer.
            let id = unsafe {
                match format {
                    "DIBV5" => 17,
                    "DIB" => 8,
                    _ => RegisterClipboardFormatW(wide.as_ptr()),
                }
            };
            if id == 0 || unsafe { IsClipboardFormatAvailable(id) } == 0 {
                continue;
            }
            // SAFETY: clipboard is open on this thread; borrowed memory is copied before closing.
            unsafe {
                let handle = GetClipboardData(id);
                if handle.is_null() {
                    continue;
                }
                let size = GlobalSize(handle);
                if size == 0
                    || size
                        > if png {
                            crate::limits::IMAGE_BYTES
                        } else {
                            MAX_RAW
                        }
                {
                    continue;
                }
                let ptr = GlobalLock(handle);
                if ptr.is_null() {
                    continue;
                }
                let _lock = Lock(handle);
                let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
                source = Some(if png {
                    Source::Png(bytes)
                } else {
                    Source::Dib(bytes)
                });
                break;
            }
        }
        // SAFETY: format inspection borrows no clipboard memory and runs on this reader thread.
        if source.is_none() && unsafe { IsClipboardFormatAvailable(14) } != 0 {
            use windows::Win32::Graphics::Gdi::{GetEnhMetaFileBits, HENHMETAFILE};
            // SAFETY: clipboard remains open; copy the vector bytes before releasing its handle.
            unsafe {
                let handle = GetClipboardData(14);
                if !handle.is_null() {
                    let emf = HENHMETAFILE(handle as isize);
                    let count = GetEnhMetaFileBits(emf, None) as usize;
                    if count > 0 && count <= MAX_RAW {
                        let mut bytes = vec![0; count];
                        if GetEnhMetaFileBits(emf, Some(&mut bytes)) as usize == count {
                            source = Some(Source::Emf(bytes));
                        }
                    }
                }
            }
        }
        if source.is_none() {
            // SAFETY: HDROP is queried only while the clipboard remains open; path is copied.
            unsafe {
                if IsClipboardFormatAvailable(15) != 0 {
                    let handle = GetClipboardData(15);
                    if !handle.is_null() {
                        let n = DragQueryFileW(handle, 0, std::ptr::null_mut(), 0);
                        if n > 0 && n < 32768 {
                            let mut name = vec![0; n as usize + 1];
                            let count = DragQueryFileW(handle, 0, name.as_mut_ptr(), n + 1);
                            if count == n {
                                source = Some(Source::File(std::path::PathBuf::from(
                                    String::from_utf16(&name[..n as usize])?,
                                )));
                            }
                        }
                    }
                }
            }
        }
        source
    };
    // All clipboard handles have been released before decoding or touching files.
    crate::crash_report::phase("clipboard-decode");
    let image = match source {
        None => return Ok(None),
        Some(Source::Png(bytes)) => crate::limits::decode_image(&bytes)?.to_rgba8(),
        Some(Source::Dib(bytes)) => decode_dib(&bytes)?,
        Some(Source::Emf(bytes)) => render_emf(&bytes)?,
        Some(Source::File(path)) => crate::limits::decode_image(&crate::limits::read_file(
            &path,
            crate::limits::IMAGE_BYTES,
        )?)?
        .to_rgba8(),
    };
    ensure!(
        (image.width() as usize)
            .checked_mul(image.height() as usize)
            .is_some_and(|n| n <= MAX_PIXELS),
        "Ảnh clipboard quá lớn"
    );
    Ok(Some(image))
}

/// Copy decoded viewer pixels as a top-down DIB, without exposing a signed CDN URL.
/// # Errors
/// Returns clipboard ownership/allocation failures. Must run off the UI thread.
#[cfg(windows)]
pub(crate) fn write_windows(image: &eframe::egui::ColorImage) -> Result<()> {
    use std::ffi::c_void;
    use windows::{
        core::w,
        Win32::{Foundation::HWND, UI::WindowsAndMessaging::*},
    };
    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(hwnd: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, memory: *mut c_void) -> *mut c_void;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(memory: *mut c_void) -> *const u8;
        fn GlobalUnlock(memory: *mut c_void) -> i32;
        fn GlobalFree(memory: *mut c_void) -> *mut c_void;
    }
    struct Owned {
        hwnd: HWND,
        memory: *mut c_void,
        opened: bool,
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: ownership transfers to Windows only after SetClipboardData succeeds.
            unsafe {
                if self.opened {
                    CloseClipboard();
                }
                if !self.memory.is_null() {
                    GlobalFree(self.memory);
                }
                if self.hwnd.0 != 0 {
                    let _ = DestroyWindow(self.hwnd);
                }
            }
        }
    }
    let [width, height] = image.size;
    ensure!(
        width > 0
            && height > 0
            && width <= 8192
            && height <= 8192
            && width
                .checked_mul(height)
                .is_some_and(|n| n <= MAX_PIXELS && n == image.pixels.len()),
        "Ảnh quá lớn để sao chép"
    );
    let bytes = 40 + image.pixels.len() * 4;
    // SAFETY: all handles stay on this worker, allocations are checked and sized before writing.
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        );
        ensure!(hwnd.0 != 0, "Không tạo được chủ clipboard");
        let mut owned = Owned {
            hwnd,
            memory: GlobalAlloc(2, bytes),
            opened: false,
        };
        ensure!(!owned.memory.is_null(), "Không cấp phát được clipboard");
        let ptr = GlobalLock(owned.memory);
        ensure!(!ptr.is_null(), "Không ghi được clipboard");
        let buffer = std::slice::from_raw_parts_mut(ptr as *mut u8, bytes);
        buffer.fill(0);
        buffer[0..4].copy_from_slice(&40u32.to_le_bytes());
        buffer[4..8].copy_from_slice(&(width as i32).to_le_bytes());
        buffer[8..12].copy_from_slice(&(-(height as i32)).to_le_bytes());
        buffer[12..14].copy_from_slice(&1u16.to_le_bytes());
        buffer[14..16].copy_from_slice(&32u16.to_le_bytes());
        for (out, pixel) in buffer[40..]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&image.pixels)
        {
            out.copy_from_slice(&[pixel.b(), pixel.g(), pixel.r(), 255]);
        }
        GlobalUnlock(owned.memory);
        for _ in 0..8 {
            if OpenClipboard(hwnd.0 as *mut c_void) != 0 {
                owned.opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        ensure!(owned.opened, "Clipboard đang bận");
        ensure!(EmptyClipboard() != 0, "Không mở được clipboard để ghi");
        ensure!(
            !SetClipboardData(8, owned.memory).is_null(),
            "Không sao chép được ảnh"
        );
        owned.memory = std::ptr::null_mut();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bitmap() -> Vec<u8> {
        let mut b = vec![0; 44];
        b[0..4].copy_from_slice(&40u32.to_le_bytes());
        b[4..8].copy_from_slice(&1i32.to_le_bytes());
        b[8..12].copy_from_slice(&(-1i32).to_le_bytes());
        b[12..14].copy_from_slice(&1u16.to_le_bytes());
        b[14..16].copy_from_slice(&32u16.to_le_bytes());
        b[40..44].copy_from_slice(&[2, 3, 4, 0]);
        b
    }
    #[test]
    fn top_down_rgb_without_alpha_is_opaque() {
        assert_eq!(
            decode_dib(&bitmap()).unwrap().get_pixel(0, 0).0,
            [4, 3, 2, 255]
        );
    }
    #[test]
    fn truncation_and_huge_dimensions_fail_before_allocation() {
        let mut b = bitmap();
        b.pop();
        assert!(decode_dib(&b).is_err());
        b = bitmap();
        b[8..12].copy_from_slice(&i32::MIN.to_le_bytes());
        assert!(decode_dib(&b).is_err());
    }
    #[test]
    fn invalid_compression_is_not_interpreted_as_pixels() {
        let mut b = bitmap();
        b[16..20].copy_from_slice(&1u32.to_le_bytes());
        assert!(decode_dib(&b).is_err());
    }
}

