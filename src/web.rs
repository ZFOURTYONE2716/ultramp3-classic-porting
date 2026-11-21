use crate::{
    audio::{Mode, SongInfo},
    scope::{Frame, FFT_SIZE, SCOPE_RATE},
    App, Entry,
};
use id3::TagLike;
use std::io::Cursor;
use wasm_bindgen::prelude::*;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioScheduledSourceNode, GainNode,
    HtmlCanvasElement,
};

const SETTINGS_KEY: &str = "ultramp3-web-settings";

pub fn settings() -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(SETTINGS_KEY)
        .ok()?
}

pub fn save_settings(settings: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(SETTINGS_KEY, settings);
    }
}

#[wasm_bindgen]
pub struct WebPlayer {
    runner: eframe::WebRunner,
}

#[wasm_bindgen]
impl WebPlayer {
    pub async fn start(canvas: HtmlCanvasElement) -> Result<WebPlayer, JsValue> {
        let runner = eframe::WebRunner::new();
        runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|_| {
                    let mut app = App {
                        selected: 2,
                        initial_skin: Some("Orbit"),
                        volume: 80,
                        ..Default::default()
                    };
                    if let Some(settings) = settings() {
                        app.apply_settings(&settings);
                    }
                    Ok(Box::new(app))
                }),
            )
            .await?;
        Ok(Self { runner })
    }

    pub fn add_audio(
        &self,
        context: AudioContext,
        buffer: AudioBuffer,
        name: String,
        bytes: &[u8],
    ) {
        let Some(mut app) = self.runner.app_mut::<App>() else {
            return;
        };
        let mut info = SongInfo {
            sample_rate: buffer.sample_rate() as u32,
            stereo: buffer.number_of_channels() > 1,
            ..Default::default()
        };
        if name.to_ascii_lowercase().ends_with(".mp3") {
            let duration = millis(buffer.duration());
            info.average_kbps = (bytes.len() as u64 * 8).checked_div(duration);
            if let Ok(tag) = id3::v1v2::read_from(Cursor::new(bytes)) {
                info.title = tag.title().map(str::to_owned);
                info.artist = tag.artist().map(str::to_owned);
                info.album = tag.album().map(str::to_owned);
                info.year = tag.year().map(|year| year.to_string());
                info.genre = tag.genre_parsed().map(|genre| genre.into_owned());
            }
        }
        app.queue.push(Entry {
            name,
            bytes: Some(bytes.len() as u64),
            marked: false,
            audio: DecodedAudio {
                context,
                buffer,
                info,
            },
        });
        app.status.clear();
        if app.current.is_none() {
            app.load(0);
        }
    }
}

pub struct DecodedAudio {
    context: AudioContext,
    buffer: AudioBuffer,
    info: SongInfo,
}

pub struct Track {
    context: AudioContext,
    buffer: AudioBuffer,
    gain: GainNode,
    source: Option<AudioBufferSourceNode>,
    start_time: f64,
    offset: f64,
    samples: [Vec<f32>; 2],
    pub wave: Option<Frame>,
    pub info: SongInfo,
    pub duration: u64,
    pub position: u64,
    pub mode: Mode,
}

impl Track {
    pub fn open(audio: &DecodedAudio, volume: u32) -> Result<Self, String> {
        let gain = audio.context.create_gain().map_err(audio_error)?;
        gain.connect_with_audio_node(&audio.context.destination())
            .map_err(audio_error)?;
        let ratio = (audio.buffer.sample_rate() / SCOPE_RATE as f32).max(1.);
        let mut track = Self {
            context: audio.context.clone(),
            buffer: audio.buffer.clone(),
            gain,
            source: None,
            start_time: 0.,
            offset: 0.,
            samples: std::array::from_fn(|_| vec![0.; (FFT_SIZE as f32 * ratio).ceil() as usize]),
            wave: None,
            info: audio.info.clone(),
            duration: millis(audio.buffer.duration()),
            position: 0,
            mode: Mode::Stopped,
        };
        track.volume(volume);
        track.play_from(0)?;
        Ok(track)
    }

    pub fn play_from(&mut self, position: u64) -> Result<(), String> {
        let offset = position.min(self.duration.saturating_sub(1)) as f64 / 1000.;
        let source = self.context.create_buffer_source().map_err(audio_error)?;
        source.set_buffer(Some(&self.buffer));
        source
            .connect_with_audio_node(&self.gain)
            .map_err(audio_error)?;
        let start = self.context.current_time();
        if let Err(error) = source.start_with_when_and_grain_offset(start, offset) {
            let _ = source.disconnect();
            return Err(audio_error(error));
        }
        self.release_source();
        self.source = Some(source);
        self.start_time = start;
        self.offset = offset;
        self.position = position.min(self.duration.saturating_sub(1));
        self.mode = Mode::Playing;
        self.wave = None;
        Ok(())
    }

    fn release_source(&mut self) {
        if let Some(source) = self.source.take() {
            let _ = source.unchecked_ref::<AudioScheduledSourceNode>().stop();
            let _ = source.disconnect();
        }
    }

    pub fn pause(&mut self) {
        self.position = millis(self.elapsed());
        self.release_source();
        self.mode = Mode::Paused;
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.release_source();
        self.mode = Mode::Stopped;
        self.seek(0)
    }

    pub fn seek(&mut self, position: u64) -> Result<(), String> {
        if self.mode == Mode::Playing {
            self.play_from(position)
        } else {
            self.position = position.min(self.duration.saturating_sub(1));
            self.wave = None;
            Ok(())
        }
    }

    pub fn volume(&self, volume: u32) {
        self.gain.gain().set_value(volume as f32 / 100.);
    }

    fn elapsed(&self) -> f64 {
        (self.offset + (self.context.current_time() - self.start_time).max(0.))
            .min(self.buffer.duration())
    }

    pub fn ended(&mut self) -> bool {
        if self.mode != Mode::Playing {
            return false;
        }
        let seconds = self.elapsed();
        self.position = millis(seconds);
        if seconds >= self.buffer.duration() {
            self.release_source();
            self.mode = Mode::Stopped;
            self.wave = None;
            return true;
        }
        self.update_wave(seconds);
        false
    }

    fn update_wave(&mut self, seconds: f64) {
        let rate = self.buffer.sample_rate() as f64;
        let end = (seconds * rate) as u32;
        let len = self.samples[0].len() as u32;
        if end < len {
            return;
        }
        let start = end - len;
        let channels = if self.info.stereo { 2 } else { 1 };
        for (channel, samples) in self.samples.iter_mut().take(channels).enumerate() {
            if self
                .buffer
                .copy_from_channel_with_start_in_channel(samples, channel as i32, start)
                .is_err()
            {
                self.wave = None;
                return;
            }
        }
        let ratio = (rate / SCOPE_RATE as f64).max(1.);
        self.wave = Some(std::array::from_fn(|index| {
            let sample = (index as f64 * ratio) as usize;
            let pcm = |channel: usize| {
                (self.samples[channel][sample] * 32768.).clamp(-32768., 32767.) as i16
            };
            [pcm(0), pcm(channels - 1)]
        }));
    }

    pub fn is_finished(&self) -> bool {
        self.position >= self.duration
    }
}

impl Drop for Track {
    fn drop(&mut self) {
        self.release_source();
        let _ = self.gain.disconnect();
    }
}

fn millis(seconds: f64) -> u64 {
    (seconds * 1000.) as u64
}

fn audio_error(error: JsValue) -> String {
    format!("Web Audio gagal: {error:?}")
}
