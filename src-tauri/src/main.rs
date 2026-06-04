// Prevents an extra console window from appearing alongside the GUI on Windows
// in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    cowork_app_lib::run();
}
