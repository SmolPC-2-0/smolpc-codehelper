#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(mobile, tauri::mobile_entry_point)]

fn main() {
    // call run() from src-tauri/src/lib.rs
    ui_lib::run();
}
