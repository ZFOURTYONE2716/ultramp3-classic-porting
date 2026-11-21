pub const FFT_SIZE: usize = 512;
pub type Frame = [[i16; 2]; FFT_SIZE];
// 0x100054c0–0x100054d0 requests 22,050 Hz from the Symbian audio stream.
// This affects only the visualization. Playback uses the original source rate.
pub const SCOPE_RATE: u32 = 22_050;

#[cfg(not(target_arch = "wasm32"))]
pub use crate::scope_native::{LatestFrame, Tap};
