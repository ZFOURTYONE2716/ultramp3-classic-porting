use crate::audio::{Mode, SongInfo};
use crate::scope::{Frame, LatestFrame, Tap};
use id3::TagLike;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::{fs::File, path::Path, time::Duration};
pub struct Track {
    frames: LatestFrame,
    pub wave: Option<Frame>,
    pub info: SongInfo,
    sink: Sink,
    _stream: OutputStream,
    pub duration: u64,
    pub position: u64,
    pub mode: Mode,
    pub _wav: Option<tempfile::TempPath>,
}

impl Track {
    pub fn open(path: &Path, wav: Option<tempfile::TempPath>, volume: u32) -> Result<Self, String> {
        let source_path = wav.as_ref().map_or(path, |file| file.as_ref());
        let file =
            File::open(source_path).map_err(|error| format!("Gagal membaca audio: {error}"))?;
        let source =
            Decoder::try_from(file).map_err(|error| format!("Gagal mendekode audio: {error}"))?;
        let duration = source.total_duration().map_or(0, millis);
        let mut stream = OutputStreamBuilder::open_default_stream()
            .map_err(|error| format!("Gagal membuka keluaran audio Windows: {error}"))?;
        stream.log_on_drop(false);
        let sink = Sink::connect_new(stream.mixer());
        sink.set_volume(volume as f32 / 100.);
        let mut info = SongInfo {
            sample_rate: source.sample_rate(),
            stereo: source.channels() > 1,
            ..Default::default()
        };
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
        {
            if duration > 0 {
                info.average_kbps = path
                    .metadata()
                    .ok()
                    .map(|metadata| metadata.len().saturating_mul(8) / duration);
            }
            if let Ok(tag) = id3::v1v2::read_from_path(path) {
                info.title = tag.title().map(str::to_owned);
                info.artist = tag.artist().map(str::to_owned);
                info.album = tag.album().map(str::to_owned);
                info.year = tag.year().map(|year| year.to_string());
                info.genre = tag.genre_parsed().map(|genre| genre.into_owned());
            }
        }
        let frames = LatestFrame::default();
        sink.append(Tap::new(source, frames.clone()));
        Ok(Self {
            sink,
            frames,
            wave: None,
            info,
            _stream: stream,
            duration,
            position: 0,
            mode: Mode::Playing,
            _wav: wav,
        })
    }

    pub fn play_from(&mut self, position: u64) -> Result<(), String> {
        if self.sink.empty() {
            return Err("Lagu sudah selesai.".into());
        }
        if position != self.position {
            self.seek(position)?;
        }
        self.sink.play();
        self.mode = Mode::Playing;
        Ok(())
    }

    pub fn pause(&mut self) {
        self.sink.pause();
        self.mode = Mode::Paused;
        self.read_position();
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.sink.pause();
        self.mode = Mode::Stopped;
        self.seek(0)
    }

    pub fn seek(&mut self, position: u64) -> Result<(), String> {
        let position = if self.duration > 0 {
            position.min(self.duration - 1)
        } else {
            position
        };
        self.sink
            .try_seek(Duration::from_millis(position))
            .map_err(|error| format!("Gagal mengubah posisi audio: {error}"))?;
        self.position = position;
        self.wave = None;
        Ok(())
    }

    pub fn volume(&self, volume: u32) {
        self.sink.set_volume(volume as f32 / 100.);
    }

    pub fn read_position(&mut self) {
        self.position = millis(self.sink.get_pos());
        if let Ok(frame) = self.frames.try_lock() {
            self.wave = *frame;
        }
    }

    pub fn ended(&mut self) -> bool {
        if self.mode != Mode::Playing {
            return false;
        }
        if !self.sink.empty() {
            self.read_position();
            return false;
        }
        self.mode = Mode::Stopped;
        self.position = self.duration.max(self.position);
        true
    }

    pub fn is_finished(&self) -> bool {
        self.sink.empty()
    }
}

fn millis(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}
