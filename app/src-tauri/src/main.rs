// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Suppress noisy ONNX Runtime initialization logs globally
    std::env::set_var("ORT_LOGGING_LEVEL", "WARNING");
    vox_lib::run()
}
