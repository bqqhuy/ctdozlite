//! Windows Hello biometric authentication lock guard (WP16-4).
//!
//! Provides app-level locking with biometric (fingerprint, face) or PIN authentication
//! on Windows 11. Auto-locks after 5 minutes of inactivity.
//!
//! Evidence: Windows Security Credentials UI (UserConsentVerifier API)
//! - `windows::Security::Credentials::UI::UserConsentVerifier`
//! - Native-device acceptance is tracked separately from fixture tests.

#[cfg(windows)]
use anyhow::Context;
use anyhow::Result;
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
    /// Create a new lock guard (app starts unlocked, no initial lock required).
    pub fn new() -> Self {
        Self {
            locked: false,
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

/// Perform the actual Windows Hello authentication request, independent of any
/// lock-guard state. Returns `true` only when the user verified successfully.
#[cfg(windows)]
pub async fn request_windows_hello() -> Result<bool> {
    use windows::core::HSTRING;
    use windows::Security::Credentials::UI::UserConsentVerifier;

    let task = tokio::task::spawn_blocking(move || -> Result<bool> {
        eprintln!("[AUTH_GUARD] Requesting Windows Hello authentication...");

        let message = HSTRING::from("ZaloLite");
        match UserConsentVerifier::RequestVerificationAsync(&message) {
            Ok(async_op) => {
                // Block on the WinRT IAsyncOperation inside the blocking pool.
                match async_op.get() {
                    Ok(result) => {
                        use windows::Security::Credentials::UI::UserConsentVerificationResult;
                        let success = matches!(result, UserConsentVerificationResult::Verified);
                        eprintln!(
                            "[AUTH_GUARD] Authentication result: {}",
                            if success { "verified" } else { "failed" }
                        );
                        Ok(success)
                    }
                    Err(e) => {
                        eprintln!("[AUTH_GUARD] Authentication failed: {e:?}");
                        Ok(false)
                    }
                }
            }
            Err(e) => {
                eprintln!("[AUTH_GUARD] Failed to request authentication: {e:?}");
                Ok(false)
            }
        }
    });

    task.await.context("Authentication task failed")?
}

/// Non-Windows stub: authentication is never available.
#[cfg(not(windows))]
pub async fn request_windows_hello() -> Result<bool> {
    Ok(false)
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
}
