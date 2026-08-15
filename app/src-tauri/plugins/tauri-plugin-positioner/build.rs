const COMMANDS: &[&str] = &[
    "move_window",
    "move_window_constrained",
    "set_tray_icon_state",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
