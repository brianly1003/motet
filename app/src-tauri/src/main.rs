//! Desktop library shell over the local pipeline. No recording starts from app launch.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod companion;
mod controller;
mod desktop;
mod library;
mod notifications;
mod permissions;
mod recording;
mod tray;
mod window_native;

fn main() {
    desktop::run();
}
