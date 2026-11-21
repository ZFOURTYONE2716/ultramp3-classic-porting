#![windows_subsystem = "windows"]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    ultramp3_windows::run_native()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
