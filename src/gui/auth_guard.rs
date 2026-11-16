//! Windows Hello biometric authentication lock guard (WP16-4).
//!
//! Provides app-level locking with biometric (fingerprint, face) or PIN authentication
//! on Windows 11. Auto-locks after 5 minutes of inactivity.
//!
//! Evidence: Windows Security Credentials UI (UserConsentVerifier API)
//! - `windows::Security::Credentials::UI::UserConsentVerifier`
//! - Native-device acceptance is tracked separately from fixture tests.

#[cfg(any(windows, test))]
use anyhow::{anyhow, Context};
use anyhow::{bail, Result};
#[cfg(any(windows, test))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Application lock guard — manages lock state and idle timeout.
pub struct AppLockGuard {
    /// Whether the app is currently locked.
    locked: bool,
    /// Last interaction timestamp.
    last_interaction: Instant,
    /// Idle threshold (5 minutes).
    idle_timeout: Duration,
    /// Whether biometric auth is available on this system.
    biometric_available: bool,
    availability: Option<(std::sync::mpsc::Receiver<bool>, Instant)>,
}

impl AppLockGuard {
    /// Create a new lock guard (app starts unlocked).
    pub fn new() -> Self {
        Self::with_initial_lock(false)
    }

    /// Create a lock guard with explicit initial lock state (M-02).
    /// If app lock is enabled and session was restored from disk on startup,
    /// it initializes locked to prevent app lock bypass on restart.
    pub fn with_initial_lock(locked: bool) -> Self {
        Self {
            locked,
            last_interaction: Instant::now(),
            idle_timeout: Duration::from_secs(5 * 60), // 5 minutes
            biometric_available: false,
            availability: Some((Self::start_availability_check(), Instant::now())),
        }
    }

    #[cfg(not(test))]
    fn start_availability_check() -> std::sync::mpsc::Receiver<bool> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("hello-availability".into())
            .spawn(move || {
                #[cfg(windows)]
                let available = {
                    use windows::Security::Credentials::UI::{
                        UserConsentVerifier, UserConsentVerifierAvailability,
                    };
                    UserConsentVerifier::CheckAvailabilityAsync()
                        .and_then(|operation| operation.get())
                        .is_ok_and(|status| status == UserConsentVerifierAvailability::Available)
                };
                #[cfg(not(windows))]
                let available = false;
                let _ = sender.send(available);
            });
        receiver
    }

    #[cfg(test)]
    fn start_availability_check() -> std::sync::mpsc::Receiver<bool> {
        let (sender, receiver) = std::sync::mpsc::channel();
        sender.send(false).unwrap();
        receiver
    }

    /// Poll the background availability probe without blocking GUI creation or rendering.
    pub fn poll_availability(&mut self) {
        if let Some((receiver, start)) = &self.availability {
            match receiver.try_recv() {
                Ok(available) => {
                    self.biometric_available = available;
                    self.availability = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.availability = None,
                Err(std::sync::mpsc::TryRecvError::Empty)
                    if start.elapsed() >= Duration::from_secs(5) =>
                {
                    self.availability = None
                }
                Err(_) => {}
            }
        }
    }

    /// Update last interaction timestamp (called on user input).
    /// Does NOT unlock the app; unlock requires successful Windows Hello auth.
    pub fn touch(&mut self) {
        self.last_interaction = Instant::now();
    }

    /// Mark the app as unlocked after successful authentication.
    pub fn unlock(&mut self) {
        self.locked = false;
        self.last_interaction = Instant::now();
    }

    /// Check if the app should auto-lock due to idle timeout.
    pub fn should_lock(&self) -> bool {
        self.biometric_available
            && !self.locked
            && self.last_interaction.elapsed() >= self.idle_timeout
    }

    /// Mark the app as locked.
    pub fn lock(&mut self) {
        self.locked = true;
    }

    /// Check if currently locked.
    pub fn is_locked(&self) -> bool {
        self.locked
    }
}

impl Default for AppLockGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
const HELLO_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(windows)]
static HELLO_ADMISSION: HelloAdmission = HelloAdmission::new();

#[cfg(any(windows, test))]
struct HelloAdmission(AtomicBool);

#[cfg(any(windows, test))]
impl HelloAdmission {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn acquire(&self) -> Result<HelloLease<'_>> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| anyhow!("Đang có yêu cầu Windows Hello khác xử lý."))?;
        Ok(HelloLease(self))
    }
}

#[cfg(any(windows, test))]
struct HelloLease<'a>(&'a HelloAdmission);

#[cfg(any(windows, test))]
impl Drop for HelloLease<'_> {
    fn drop(&mut self) {
        self.0 .0.store(false, Ordering::Release);
    }
}

/// Perform a desktop-owned Windows Hello request and return whether verification succeeded.
///
/// # Errors
/// Returns an error for an invalid window handle, a concurrent request, a timed-out request,
/// or a failed native operation. A timeout does not cancel WinRT; admission stays held until
/// the native operation actually completes.
pub async fn request_windows_hello(hwnd: isize) -> Result<bool> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;

        let native_hwnd = HWND(hwnd);
        validate_hwnd(hwnd, is_valid_window)?;
        request_with_backend(&HELLO_ADMISSION, HELLO_REQUEST_TIMEOUT, move || {
            run_desktop_hello(native_hwnd)
        })
        .await
    }

    #[cfg(not(windows))]
    {
        let _ = hwnd;
        bail!("Windows Hello chỉ khả dụng trên Windows.");
    }
}

#[cfg(windows)]
fn is_valid_window(hwnd: isize) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow};

    let native_hwnd = windows::Win32::Foundation::HWND(hwnd);
    let mut process_id = 0;
    // SAFETY: These calls inspect the HWND and return its owning process; they do not retain it.
    unsafe {
        IsWindow(native_hwnd).as_bool()
            && GetWindowThreadProcessId(native_hwnd, Some(&mut process_id as *mut u32)) != 0
            && process_id == std::process::id()
    }
}

#[cfg(any(windows, test))]
fn validate_hwnd(isize_hwnd: isize, is_window: impl FnOnce(isize) -> bool) -> Result<()> {
    if isize_hwnd == 0 || !is_window(isize_hwnd) {
        bail!("Không thể xác thực Windows Hello vì cửa sổ ứng dụng không hợp lệ.");
    }
    Ok(())
}

#[cfg(any(windows, test))]
async fn request_with_backend<F>(
    admission: &'static HelloAdmission,
    timeout: Duration,
    operation: F,
) -> Result<bool>
where
    F: FnOnce() -> Result<bool> + Send + 'static,
{
    let lease = admission.acquire()?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("windows-hello-request".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
                .unwrap_or_else(|_| Err(anyhow!("Windows Hello gặp lỗi nội bộ.")));
            drop(lease);
            let _ = sender.send(result);
        })
        .context("Không thể khởi chạy yêu cầu Windows Hello.")?;

    match tokio::time::timeout(timeout, receiver).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => bail!("Yêu cầu Windows Hello đã kết thúc bất thường."),
        Err(_) => bail!("Yêu cầu Windows Hello quá thời hạn 30 giây; hãy thử lại sau."),
    }
}

#[cfg(windows)]
fn run_desktop_hello(hwnd: windows::Win32::Foundation::HWND) -> Result<bool> {
    use windows::core::HSTRING;
    use windows::Security::Credentials::UI::UserConsentVerificationResult;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
    use windows::Win32::System::WinRT::{
        RoGetActivationFactory, RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED,
    };

    // SAFETY: This dedicated native thread has no prior WinRT apartment initialization.
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
        .context("Không thể khởi tạo WinRT apartment cho Windows Hello.")?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: This guard is created only after successful RoInitialize on this thread.
            unsafe { RoUninitialize() };
        }
    }
    let _apartment = Apartment;

    let class_name = HSTRING::from("Windows.Security.Credentials.UI.UserConsentVerifier");
    // SAFETY: WinRT is initialized on this thread and the requested runtime class is the
    // documented desktop verifier interop class.
    let interop: IUserConsentVerifierInterop = unsafe { RoGetActivationFactory(&class_name) }
        .context("Không thể khởi tạo giao diện Windows Hello trên desktop.")?;
    let message = HSTRING::from("Xác minh danh tính để mở khóa ZaloLite");
    validate_hwnd(hwnd.0, is_valid_window)?;
    // SAFETY: HWND was revalidated on this native thread. WinRT manages the consent UI after
    // invocation, and this call retains no Rust borrow or pointer into the GUI owner. The GUI
    // generation layer must ignore any completion that arrives after the window closes.
    let operation: windows::Foundation::IAsyncOperation<UserConsentVerificationResult> =
        unsafe { interop.RequestVerificationForWindowAsync(hwnd, &message) }
            .context("Không thể hiển thị yêu cầu Windows Hello.")?;
    let result = operation
        .get()
        .context("Yêu cầu Windows Hello không hoàn tất thành công.")?;
    Ok(matches!(result, UserConsentVerificationResult::Verified))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn lock_guard_starts_unlocked() {
        let guard = AppLockGuard::new();
        assert!(!guard.is_locked());
    }

    #[test]
    fn m02_lock_guard_with_initial_lock_starts_locked() {
        let guard = AppLockGuard::with_initial_lock(true);
        assert!(guard.is_locked());
        let guard_unlocked = AppLockGuard::with_initial_lock(false);
        assert!(!guard_unlocked.is_locked());
    }

    #[test]
    fn touch_updates_interaction_time() {
        let mut guard = AppLockGuard::new();
        let initial = guard.last_interaction;
        thread::sleep(Duration::from_millis(100));
        guard.touch();
        assert!(guard.last_interaction > initial);
    }

    #[test]
    fn should_lock_after_timeout() {
        let mut guard = AppLockGuard::new();
        guard.biometric_available = true;
        guard.idle_timeout = Duration::from_millis(100);
        guard.touch();
        assert!(!guard.should_lock());
        thread::sleep(Duration::from_millis(150));
        assert!(guard.should_lock());
    }

    #[test]
    fn lock_prevents_auto_lock() {
        let mut guard = AppLockGuard::new();
        guard.lock();
        assert!(guard.is_locked());
        assert!(!guard.should_lock());
    }

    #[test]
    fn touch_does_not_unlock_locked_app() {
        let mut guard = AppLockGuard::new();
        guard.lock();
        guard.touch();
        assert!(
            guard.is_locked(),
            "touch must not bypass Windows Hello auth"
        );
    }

    #[test]
    fn unlock_after_lock_resets_state() {
        let mut guard = AppLockGuard::new();
        guard.lock();
        assert!(guard.is_locked());
        guard.unlock();
        assert!(!guard.is_locked());
        assert!(!guard.should_lock(), "unlock must reset the idle timer");
    }

    #[test]
    fn touch_resets_idle_timer() {
        let mut guard = AppLockGuard::new();
        guard.biometric_available = true;
        guard.idle_timeout = Duration::from_millis(50);
        thread::sleep(Duration::from_millis(30));
        guard.touch();
        assert!(!guard.should_lock());
        thread::sleep(Duration::from_millis(70));
        assert!(guard.should_lock());
    }

    #[test]
    fn admission_rejects_overlapping_native_operations() {
        let admission = HelloAdmission::new();
        let lease = admission.acquire().unwrap();
        assert!(admission.acquire().is_err());
        drop(lease);
        assert!(admission.acquire().is_ok());
    }

    #[tokio::test]
    async fn timeout_keeps_admission_until_native_operation_finishes() {
        let admission = Box::leak(Box::new(HelloAdmission::new()));
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let timed_out = request_with_backend(admission, Duration::from_millis(10), move || {
            started_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
            Ok(true)
        })
        .await;
        assert!(timed_out.is_err());
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(admission.acquire().is_err());
        finish_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        while admission.0.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(!admission.0.load(Ordering::Acquire));
    }

    #[test]
    fn invalid_window_handles_fail_closed_without_native_calls() {
        let mut native_calls = 0;
        assert!(validate_hwnd(0, |_| {
            native_calls += 1;
            true
        })
        .is_err());
        assert_eq!(native_calls, 0);
        assert!(validate_hwnd(123, |_| false).is_err());
    }
}
