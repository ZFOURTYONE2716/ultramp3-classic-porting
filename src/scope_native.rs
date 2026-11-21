use rodio::{source::SeekError, Source};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::scope::{Frame, FFT_SIZE, SCOPE_RATE};
pub type LatestFrame = Arc<Mutex<Option<Frame>>>;

const PUBLISH_INTERVAL: usize = 64;

struct Window {
    samples: Frame,
    cursor: usize,
    len: usize,
    since_publish: usize,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            samples: [[0; 2]; FFT_SIZE],
            cursor: 0,
            len: 0,
            since_publish: 0,
        }
    }
}

impl Window {
    fn push(&mut self, sample: [i16; 2], latest: &LatestFrame) {
        self.samples[self.cursor] = sample;
        self.cursor = (self.cursor + 1) % FFT_SIZE;
        self.len = (self.len + 1).min(FFT_SIZE);
        self.since_publish += 1;
        if self.len < FFT_SIZE || self.since_publish < PUBLISH_INTERVAL {
            return;
        }
        // Never wait on the UI thread. The following sample can retry.
        if let Ok(mut destination) = latest.try_lock() {
            let mut frame = [[0; 2]; FFT_SIZE];
            let tail = FFT_SIZE - self.cursor;
            frame[..tail].copy_from_slice(&self.samples[self.cursor..]);
            frame[tail..].copy_from_slice(&self.samples[..self.cursor]);
            *destination = Some(frame);
            self.since_publish = 0;
        }
    }
}

pub struct Tap<S> {
    source: S,
    latest: LatestFrame,
    window: Window,
    channel: usize,
    pcm: [i16; 2],
    phase: u32,
}

impl<S> Tap<S> {
    pub fn new(source: S, latest: LatestFrame) -> Self {
        Self {
            source,
            latest,
            window: Window::default(),
            channel: 0,
            pcm: [0; 2],
            phase: 0,
        }
    }
}

impl<S: Source> Iterator for Tap<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let channels = self.source.channels() as usize;
        let rate = self.source.sample_rate();
        let sample = self.source.next()?;
        if self.channel < 2 {
            self.pcm[self.channel] = (sample * 32768.).clamp(-32768., 32767.) as i16;
        }
        self.channel += 1;
        if self.channel >= channels {
            self.channel = 0;
            if channels == 1 {
                self.pcm[1] = self.pcm[0];
            }
            self.phase += SCOPE_RATE.min(rate);
            if self.phase >= rate {
                self.phase -= rate;
                self.window.push(self.pcm, &self.latest);
            }
        }
        Some(sample)
    }
}

impl<S: Source> Source for Tap<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.source.current_span_len()
    }
    fn channels(&self) -> u16 {
        self.source.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.source.try_seek(position)?;
        self.window = Window::default();
        self.channel = 0;
        self.phase = 0;
        if let Ok(mut latest) = self.latest.try_lock() {
            *latest = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;

    #[test]
    fn slow_reader_gets_newest_decimated_stereo_window_in_order() {
        let latest = LatestFrame::default();
        let frames = (0..2000)
            .flat_map(|n| [n as f32 / 32768., -(n as f32) / 32768.])
            .collect::<Vec<_>>();
        let source = SamplesBuffer::new(2, 44_100, frames);
        let tap = Tap::new(source, latest.clone());
        assert_eq!(tap.count(), 4000);
        let frame = latest.lock().unwrap().unwrap();
        // Publish after frame 512, then every 64 scope-rate frames. The last
        // publication is at 960 samples, leaving fewer than 64 pending.
        assert_eq!(frame[0], [897, -897]);
        assert_eq!(frame[FFT_SIZE - 1], [1919, -1919]);
        assert!(frame.windows(2).all(|pair| pair[1][0] - pair[0][0] == 2));
    }
}
