//! Operating-system randomness for protocol nonces. No time/PID-derived seeds.
use anyhow::Result;
pub fn fill(bytes: &mut [u8]) -> Result<()> {
    #[cfg(windows)]
    {
        #[link(name = "bcrypt")]
        extern "system" {
            fn BCryptGenRandom(
                algorithm: *mut std::ffi::c_void,
                buffer: *mut u8,
                len: u32,
                flags: u32,
            ) -> i32;
        }
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                bytes.len().try_into()?,
                2,
            )
        };
        anyhow::ensure!(status >= 0, "BCryptGenRandom failed: {status}");
    }
    #[cfg(not(windows))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")?.read_exact(bytes)?;
    }
    Ok(())
}
