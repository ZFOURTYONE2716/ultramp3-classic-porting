#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Stopped,
    Playing,
    Paused,
}
#[derive(Clone, Default)]
pub struct SongInfo {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub sample_rate: u32,
    pub average_kbps: Option<u64>,
    pub stereo: bool,
}
#[cfg(not(target_arch = "wasm32"))]
pub use crate::audio_native::Track;
#[cfg(target_arch = "wasm32")]
pub use crate::web::Track;
