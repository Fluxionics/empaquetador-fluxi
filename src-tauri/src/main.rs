#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod packer;

use packer::{pack_to_exe, get_file_info};

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![pack_to_exe, get_file_info])
        .run(tauri::generate_context!())
        .expect("Error iniciando Fluxi");
}
