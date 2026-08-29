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
            use webkit2gtk::WebViewExt;

            let web_view = _webview.inner();

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

            log::info!(
                "[PinchZoom] Registered notify::zoom-level guard rail for pinch-to-zoom protection."
            );
        }) {
            log::warn!("[PinchZoom] Failed to access webview: {}", e);
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn webview_created(&mut self, _webview: Webview<R>) {}
}
