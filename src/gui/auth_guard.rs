//! Windows Hello biometric authentication lock guard (WP16-4).
//!
//! Provides app-level locking with biometric (fingerprint, face) or PIN authentication
//! on Windows 11. Auto-locks after 5 minutes of inactivity.
//!
//! Evidence: Windows Security Credentials UI (UserConsentVerifier API)
//! - `windows::Security::Credentials::UI::UserConsentVerifier`
//! - Tested on Windows 11 ARM64

use std::time::{Duration, Instant};
use anyhow::{Context, Result};

/// Application lock guard — manages lock state and idle timeout.
#[allow(dead_code)]
pub struct AppLockGuard {
    /// Whether the app is currently locked.
    locked: bool,
    /// Last interaction timestamp.
    last_interaction: Instant,
    /// Idle threshold (5 minutes).
    idle_timeout: Duration,
    /// Whether biometric auth is available on this system.
    biometric_available: bool,
}

#[allow(dead_code)]
impl AppLockGuard {
    /// Create a new lock guard (app starts unlocked, no initial lock required).
    pub fn new() -> Self {
        Self {
            locked: false,
            last_interaction: Instant::now(),
            idle_timeout: Duration::from_secs(5 * 60), // 5 minutes
            biometric_available: Self::check_biometric_available(),
        }
    }

    /// Check if Windows Hello is available on this system.
    #[cfg(windows)]
    fn check_biometric_available() -> bool {
        use windows::Security::Credentials::UI::UserConsentVerifier;

        // Check availability synchronously via blocking task
        std::thread::spawn(|| {
            match UserConsentVerifier::CheckAvailabilityAsync() {
                Ok(async_op) => {
                    // Poll the async operation synchronously
                    match async_op.get() {
                        Ok(availability) => {
                            use windows::Security::Credentials::UI::UserConsentVerifierAvailability;
                            matches!(
                                availability,
                                UserConsentVerifierAvailability::Available
                            )
                        }
                        Err(_) => false,
                    }
                }
                Err(_) => false,
            }
        }).join().unwrap_or(false)

    }

    #[cfg(not(windows))]
    fn check_biometric_available() -> bool {
        false
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
        !self.locked && self.last_interaction.elapsed() >= self.idle_timeout
    }

    /// Mark the app as locked.
    pub fn lock(&mut self) {
        self.locked = true;
    }

    /// Check if currently locked.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Request biometric authentication and unlock if successful.
    /// Returns `true` if unlock succeeded, `false` if authentication failed or was cancelled.
    pub async fn authenticate(&mut self) -> Result<bool> {
        if !self.biometric_available {
            return Ok(false);
        }

        let result = request_windows_hello().await?;
        if result {
            self.unlock();
        }
        Ok(result)
    }

    /// Return the remaining idle time before auto-lock.
    pub fn time_until_lock(&self) -> Duration {
        if self.locked {
            Duration::ZERO
        } else {
            self.idle_timeout.saturating_sub(self.last_interaction.elapsed())
        }
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
    use windows::Security::Credentials::UI::UserConsentVerifier;
    use windows::core::HSTRING;

    let task = tokio::task::spawn_blocking(move || -> Result<bool> {
        eprintln!("[AUTH_GUARD] Requesting Windows Hello authentication...");

        let message = HSTRING::from("ZaloLite");
        match UserConsentVerifier::RequestVerificationAsync(&message) {
            Ok(async_op) => {
                // Block on the WinRT IAsyncOperation inside the blocking pool.
                match async_op.get() {
                    Ok(result) => {
                        use windows::Security::Credentials::UI::UserConsentVerificationResult;
                        let success =
                            matches!(result, UserConsentVerificationResult::Verified);
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
        guard.idle_timeout = Duration::from_millis(50);
        assert!(!guard.should_lock());
        thread::sleep(Duration::from_millis(100));
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
        assert!(guard.is_locked(), "touch must not bypass Windows Hello auth");
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
        guard.idle_timeout = Duration::from_millis(50);
        thread::sleep(Duration::from_millis(30));
        guard.touch();
        assert!(!guard.should_lock());
        thread::sleep(Duration::from_millis(70));
        assert!(guard.should_lock());
    }
}
