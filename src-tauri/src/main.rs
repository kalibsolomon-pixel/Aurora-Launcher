// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if aurora_launcher_lib::run_artwork_decoder_if_requested() {
        return;
    }
    aurora_launcher_lib::run()
}
