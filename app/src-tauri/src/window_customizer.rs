use tauri::{plugin::Plugin, Runtime, Webview};

/// Disables native WebKitGTK pinch-to-zoom on Linux.
#[derive(Default)]
pub struct PinchZoomDisablePlugin;

impl<R: Runtime> Plugin<R> for PinchZoomDisablePlugin {
    fn name(&self) -> &'static str {
        "vox-pinch-zoom-disable"
    }

    fn webview_created(&mut self, webview: Webview<R>) {
        #[cfg(target_os = "linux")]
        if let Err(e) = webview.with_webview(|_webview| {
            use gtk::glib::ObjectExt;
            use gtk::prelude::*;
            use webkit2gtk::glib::gobject_ffi;
            use webkit2gtk::WebViewExt;

            let web_view = _webview.inner();

            // Helper to destroy signal handlers attached to 'wk-view-zoom-gesture'
            fn destroy_zoom_gesture(web_view: &webkit2gtk::WebView) -> bool {
                unsafe {
                    if let Some(data) = web_view.data::<gtk::GestureZoom>("wk-view-zoom-gesture") {
                        gobject_ffi::g_signal_handlers_destroy(data.as_ptr().cast());
                        log::info!("[PinchZoom] Destroyed 'wk-view-zoom-gesture' signal handlers.");
                        return true;
                    }
                }
                false
            }

            // 1. Destroy immediately if gesture data already exists
            let initial_destroyed = destroy_zoom_gesture(&web_view);

            // 2. Destroy on GTK widget map signal (when widget is realized/mapped to screen)
            web_view.connect_map(|web_view| {
                destroy_zoom_gesture(web_view);
            });

            // 3. Guard rail: Listen to notify::zoom-level changes and lock zoom_level to 1.0
            web_view.connect_notify(Some("zoom-level"), |web_view, _| {
                let current_zoom = web_view.zoom_level();
                if (current_zoom - 1.0).abs() > 0.001 {
                    log::info!(
                        "[PinchZoom] Intercepted zoom level change ({:.2} -> 1.0), forcing reset.",
                        current_zoom
                    );
                    web_view.set_zoom_level(1.0);
                }
            });

            if !initial_destroyed {
                log::info!(
                    "[PinchZoom] Registered GTK map signal and notify::zoom-level guard rail for pinch-to-zoom protection."
                );
            }
        }) {
            log::warn!("[PinchZoom] Failed to access webview: {}", e);
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn webview_created(&mut self, _webview: Webview<R>) {}
}
