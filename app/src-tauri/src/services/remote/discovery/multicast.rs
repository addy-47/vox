//! Android Wi-Fi Multicast Lock for mDNS packet reception.
//!
//! On Android, the Wi-Fi radio filters multicast packets by default to preserve
//! battery. To browse mDNS services (`_vox._tcp.local`), the app must hold a
//! `WifiManager.MulticastLock` with `CHANGE_WIFI_MULTICAST_STATE` permission.
//!
//! Workspace rule: `android-style-guide.md` (no unhandled JNI panics, proper keep rules).

#[cfg(target_os = "android")]
use jni::objects::JObject;

// ─── Constants & Type Aliases ────────────────────────────────────────────────

// ─── Data Structures ─────────────────────────────────────────────────────────

/// RAII drop guard for Android Wi-Fi MulticastLock.
pub struct MulticastLockGuard {
    #[cfg(target_os = "android")]
    acquired: bool,
}

// ─── Trait Implementations ───────────────────────────────────────────────────

impl Drop for MulticastLockGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "android")]
        if self.acquired {
            let _ = std::panic::catch_unwind(|| {
                let ctx = ndk_context::android_context();
                let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.ok()?;
                let mut env = vm.attach_current_thread().ok()?;
                let res = env.call_static_method(
                    "com/addy/Vox/MulticastLockManager",
                    "release",
                    "()V",
                    &[],
                );
                if res.is_err() {
                    let _ = env.exception_clear();
                }
                log::info!("[Remote::Multicast] MulticastLock released");
                Some(())
            });
        }
    }
}

// ─── Inherent Implementations ────────────────────────────────────────────────

impl MulticastLockGuard {
    /// Acquires the Android MulticastLock if running on Android.
    /// Returns a guard that releases the lock on drop.
    #[cfg(target_os = "android")]
    pub fn acquire() -> Self {
        let acquired = std::panic::catch_unwind(|| {
            let ctx = ndk_context::android_context();
            let vm = match unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) } {
                Ok(vm) => vm,
                Err(e) => {
                    log::warn!("[Remote::Multicast] Failed to get JavaVM from context: {e}");
                    return false;
                }
            };
            let mut env = match vm.attach_current_thread() {
                Ok(env) => env,
                Err(e) => {
                    log::warn!("[Remote::Multicast] Failed to attach current thread: {e}");
                    return false;
                }
            };
            let context = unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) };

            // Primary path: helper object in com.addy.Vox (handles exception safely in Kotlin)
            let kotlin_res = env.call_static_method(
                "com/addy/Vox/MulticastLockManager",
                "acquire",
                "(Landroid/content/Context;)Z",
                &[(&context).into()],
            );

            if let Ok(val) = kotlin_res {
                if let Ok(success) = val.z() {
                    log::info!("[Remote::Multicast] MulticastLockManager.acquire: {success}");
                    return success;
                }
            }

            let _ = env.exception_clear();
            log::warn!("[Remote::Multicast] MulticastLockManager.acquire failed");
            false
        })
        .unwrap_or(false);

        Self { acquired }
    }

    /// Desktop no-op constructor.
    #[cfg(not(target_os = "android"))]
    pub fn acquire() -> Self {
        Self {}
    }
}

// ─── Helper Functions ────────────────────────────────────────────────────────
