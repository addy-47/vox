use std::{
    ffi::c_void,
    sync::OnceLock,
};

use jni::{
    objects::{GlobalRef, JObject},
    sys::{jobject, JNIEnv},
};

static ANDROID_APP_ACTIVITY: OnceLock<GlobalRef> = OnceLock::new();

/// Initializes the native `ndk_context` from the Android `MainActivity` JNI entry point.
///
/// Passes the JavaVM pointer and a retained JNI GlobalRef of the MainActivity to
/// `ndk_context::initialize_android_context`.
///
/// # Safety
///
/// Must be invoked over JNI with a valid `JNIEnv` pointer and a live `MainActivity` jobject
/// provided by the Android runtime.
#[no_mangle]
pub unsafe extern "C" fn Java_com_addy_Vox_MainActivity_initAndroidContext(
    env: *mut JNIEnv,
    activity: jobject,
) {
    if env.is_null() || activity.is_null() {
        log::error!("[AndroidContext] Null JNIEnv or Activity received in initAndroidContext");
        return;
    }

    let jni_env = match jni::JNIEnv::from_raw(env) {
        Ok(e) => e,
        Err(e) => {
            log::error!(
                "[AndroidContext] Failed to create JNIEnv from raw pointer: {}",
                e
            );
            return;
        }
    };

    let global_activity = match jni_env.new_global_ref(JObject::from_raw(activity)) {
        Ok(g) => g,
        Err(e) => {
            log::error!(
                "[AndroidContext] Failed to create global ref for MainActivity: {}",
                e
            );
            return;
        }
    };

    let vm = match jni_env.get_java_vm() {
        Ok(vm) => vm,
        Err(e) => {
            log::error!("[AndroidContext] Failed to obtain JavaVM: {}", e);
            return;
        }
    };

    let vm_ptr = vm.get_java_vm_pointer() as *mut c_void;
    let activity_ptr = global_activity.as_obj().as_raw() as *mut c_void;

    if let Err(e) = ANDROID_APP_ACTIVITY.set(global_activity) {
        log::warn!(
            "[AndroidContext] MainActivity global ref was already retained: {:?}",
            e.as_obj()
        );
    }

    // ndk-context is already initialized by tao (TauriActivity) during super.onCreate()
    // with the application context; calling initialize_android_context again here would
    // trip its `previous.is_none()` assertion and abort the process. Do NOT re-init.
    let _ = (vm_ptr, activity_ptr);
    log::info!(
        "[AndroidContext] ndk-context already owned by tao; activity ref retained only"
    );
}
