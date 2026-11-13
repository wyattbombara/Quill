#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod app;
mod commands;
use quill_chat::{error, model, services};

fn main() {
    app::run();
}
