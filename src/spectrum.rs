use crate::scope::{Frame, FFT_SIZE};
use eframe::egui::Color32;
use std::f32::consts::TAU;
use web_time::Instant;

const BINS: usize = FFT_SIZE / 2;
const MAX_COLUMNS: usize = 176;
const ONE: i32 = 1 << 16;

#[derive(Default, Clone, Copy)]
struct Column {
    level: i32,
    peak: i32,
    fall: i32,
    color_level: i32,
}

pub struct Spectrum {
    columns: [Column; MAX_COLUMNS],
    last: Option<Instant>,
}

impl Default for Spectrum {
    fn default() -> Self {
        Self {
            columns: [Column::default(); MAX_COLUMNS],
            last: None,
        }
    }
}

impl Spectrum {
    pub fn update(&mut self, frame: &Frame, columns: usize) {
        let bins = magnitudes(frame);
        let now = Instant::now();
        let elapsed = self.last.replace(now).map_or(16, |before| {
            now.duration_since(before).as_millis().min(100) as i32
        });
        let decay = elapsed * 32;

        // 0x10003d18–0x10003e0c: evenly spaced FFT bins, immediate rise,
        // linear bar decay, and a peak marker that falls with acceleration.
        let columns = columns.clamp(1, MAX_COLUMNS);
        let bin_step = (BINS << 16) / columns;
        for (x, column) in self.columns.iter_mut().take(columns).enumerate() {
            let bin = (x * bin_step) >> 16;
            let target = (bins[bin].min(32767.) * (0.2 * 256. / (FFT_SIZE as f32).sqrt())) as i32;
            column.color_level = target;
            column.level = column.level.max(target).min(ONE);
            if column.peak <= column.level {
                column.peak = column.level;
                column.fall = 0;
            } else {
                column.fall = (column.fall + elapsed).min(ONE);
                column.peak = (column.peak - column.fall).max(0);
            }
            column.level = (column.level - decay).max(0);
        }
    }

    pub fn heights(&self, x: usize, height: i32) -> (i32, i32) {
        let column = self.columns[x];
        (
            (column.level * height / ONE).min(height - 1),
            (column.peak * height / ONE).min(height - 1),
        )
    }

    pub fn color(&self, x: usize) -> Color32 {
        // 0x10003eb8–0x10003f70: blue, cyan, green, yellow, red.
        let index = (self.columns[x].color_level >> 8).clamp(0, 255);
        let ramp = |step: i32| (step * 256 / 51).min(255) as u8;
        match index {
            0..=50 => Color32::from_rgb(0, 0, ramp(index)),
            51..=101 => Color32::from_rgb(0, ramp(index - 51), 255),
            102..=152 => Color32::from_rgb(0, 255, 255 - ramp(index - 102)),
            153..=203 => Color32::from_rgb(ramp(index - 153), 255, 0),
            _ => Color32::from_rgb(255, 255 - ramp(index - 204), 0),
        }
    }
}

/// Radix-2 FFT of the 512 signed PCM frames used by 0x10009f38.
/// The original folds stereo to mono and scales each sample by 1/64.
fn magnitudes(frame: &Frame) -> [f32; BINS] {
    let mut real = [0.; FFT_SIZE];
    let mut imaginary = [0.; FFT_SIZE];
    let bits = FFT_SIZE.trailing_zeros();
    for (i, &[left, right]) in frame.iter().enumerate() {
        let reversed = i.reverse_bits() >> (usize::BITS - bits);
        real[reversed] = ((left as i32 + right as i32) / 2) as f32 / 64.;
    }
    for stage in 1..=bits {
        let span = 1 << stage;
        let half = span / 2;
        for k in 0..half {
            let (sin, cos) = (-TAU * k as f32 / span as f32).sin_cos();
            for a in (k..FFT_SIZE).step_by(span) {
                let b = a + half;
                let re = real[b] * cos - imaginary[b] * sin;
                let im = real[b] * sin + imaginary[b] * cos;
                real[b] = real[a] - re;
                imaginary[b] = imaginary[a] - im;
                real[a] += re;
                imaginary[a] += im;
            }
        }
    }
    std::array::from_fn(|bin| real[bin].hypot(imaginary[bin]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_tone_lands_in_its_fft_bin_and_antiphase_cancels() {
        let tone: Frame = std::array::from_fn(|i| {
            let sample = (4000. * (TAU * 17. * i as f32 / FFT_SIZE as f32).sin()) as i16;
            [sample, sample]
        });
        let bins = magnitudes(&tone);
        assert_eq!(
            bins.iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0,
            17
        );
        assert!((bins[17] - 16000.).abs() < 10.);
        assert!(magnitudes(&tone.map(|[l, r]| [l, -r]))
            .iter()
            .all(|&level| level == 0.));
    }
}
