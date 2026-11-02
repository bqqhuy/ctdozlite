//! Local bounded phase diagnostics. Never write message bodies, file paths or payloads.
use std::sync::{Mutex, OnceLock};
static PHASES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
pub fn phase(name: &'static str) {
    if let Ok(mut p) = PHASES.lock() {
        if p.len() >= 24 {
            p.remove(0);
        }
        p.push(name);
    }
}
pub fn install() {
    let _ = ROOT.set(crate::storage::root().join("diagnostics"));
    #[cfg(windows)]
    native::install();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(root) = ROOT.get() {
            // A fatal diagnostic is best effort; do not recurse into the panic hook.
            if std::fs::create_dir_all(root).is_ok() {
                let source = info
                    .location()
                    .map(|l| {
                        format!(
                            "{}:{}",
                            std::path::Path::new(l.file())
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                            l.line()
                        )
                    })
                    .unwrap_or_default();
                let phases = PHASES.try_lock().map(|p| p.join(" > ")).unwrap_or_default();
                let report = format!(
                    "version={} arch={} time={} source={} phases={}\n",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::ARCH,
                    crate::login::unix_now(),
                    source,
                    phases
                );
                let _ = std::fs::write(root.join("last-panic.txt"), report);
            }
        }
        previous(info);
    }));
}

#[cfg(windows)]
mod native {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, sync::OnceLock};
    use windows::Win32::System::Diagnostics::Debug::{
        SetUnhandledExceptionFilter, EXCEPTION_POINTERS,
    };
    static PATH: OnceLock<Vec<u16>> = OnceLock::new();
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *const c_void,
            creation: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn WriteFile(
            handle: *mut c_void,
            buffer: *const u8,
            length: u32,
            written: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }
    unsafe extern "system" fn report(exception: *const EXCEPTION_POINTERS) -> i32 {
        // No allocations, locks, payloads or stack dumps in a failing process.
        let Some(path) = PATH.get() else {
            return 0;
        };
        if exception.is_null() || (*exception).ExceptionRecord.is_null() {
            return 0;
        }
        let record = &*(*exception).ExceptionRecord;
        let mut buffer = [b' '; 96];
        let prefix = b"native exception code=0x";
        buffer[..prefix.len()].copy_from_slice(prefix);
        let hex = b"0123456789abcdef";
        let code = record.ExceptionCode.0 as u32;
        for n in 0..8 {
            buffer[prefix.len() + n] = hex[((code >> (28 - n * 4)) & 15) as usize];
        }
        let offset = (record.ExceptionAddress as usize)
            .wrapping_sub(GetModuleHandleW(std::ptr::null()) as usize) as u64;
        let label = b" main-image-relative=0x";
        let start = prefix.len() + 8;
        buffer[start..start + label.len()].copy_from_slice(label);
        for n in 0..16 {
            buffer[start + label.len() + n] = hex[((offset >> (60 - n * 4)) & 15) as usize];
        }
        let end = start + label.len() + 16;
        buffer[end] = b'\n';
        let handle = CreateFileW(
            path.as_ptr(),
            0x40000000,
            1,
            std::ptr::null(),
            2,
            0x80,
            std::ptr::null_mut(),
        );
        if !handle.is_null() && handle as isize != -1 {
            let mut written = 0;
            WriteFile(
                handle,
                buffer.as_ptr(),
                (end + 1) as u32,
                &mut written,
                std::ptr::null_mut(),
            );
            CloseHandle(handle);
        }
        0 // Continue to Windows Error Reporting; access violations cannot be unwound safely.
    }
    pub fn install() {
        let directory = crate::storage::root().join("diagnostics");
        if std::fs::create_dir_all(&directory).is_err() {
            return;
        }
        let path = directory.join("last-native-error.txt");
        let _ = PATH.set(path.as_os_str().encode_wide().chain(Some(0)).collect());
        // SAFETY: a process-lifetime function and NUL-terminated path outlive every callback.
        unsafe {
            SetUnhandledExceptionFilter(Some(report));
        }
    }
}
