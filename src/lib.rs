use eframe::egui::{self, Color32, FontId, Rect, Vec2};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};
#[cfg(not(target_arch = "wasm32"))]
mod dialogs;
#[cfg(not(target_arch = "wasm32"))]
use dialogs::{choose_files, choose_folder, file_dialog, playlist_dialog};

mod audio;
#[cfg(not(target_arch = "wasm32"))]
mod audio_native;

mod skin;
mod spectrum;

mod scope;
#[cfg(not(target_arch = "wasm32"))]
mod scope_native;

#[cfg(not(target_arch = "wasm32"))]
mod state;
#[cfg(not(target_arch = "wasm32"))]
mod tracker;
#[cfg(target_arch = "wasm32")]
mod web;
use audio::{Mode, Track};
use skin::{Skin, SIZE, SLOTS};
#[cfg(target_arch = "wasm32")]
pub use web::WebPlayer;

#[cfg(not(target_arch = "wasm32"))]
fn title(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn time(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Visualization {
    #[default]
    Oscilloscope,
    Spectrum,
    None,
}

impl Visualization {
    fn label(self) -> &'static str {
        match self {
            Self::Oscilloscope => "OSCILLOSCOPE",
            Self::Spectrum => "SPECTRUM BARS",
            Self::None => "NONE",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Oscilloscope => Self::Spectrum,
            Self::Spectrum => Self::None,
            Self::None => Self::Oscilloscope,
        }
    }
}

#[derive(Default, PartialEq, Eq)]

enum View {
    #[default]
    Player,
    Menu,
    Playlist,
    About,
}
#[derive(Default)]

enum PlayMode {
    Single,
    #[default]
    Continuous,
    Random,
}

impl PlayMode {
    fn label(&self) -> &'static str {
        match self {
            Self::Single => "SINGLE",
            Self::Continuous => "CONTINUOUS",
            Self::Random => "RANDOM",
        }
    }

    fn next(&self) -> Self {
        match self {
            Self::Single => Self::Continuous,
            Self::Continuous => Self::Random,
            Self::Random => Self::Single,
        }
    }
}

struct Entry {
    #[cfg(not(target_arch = "wasm32"))]
    path: PathBuf,
    #[cfg(target_arch = "wasm32")]
    audio: web::DecodedAudio,
    name: String,
    bytes: Option<u64>,
    marked: bool,
}

impl Entry {
    #[cfg(not(target_arch = "wasm32"))]
    fn new(path: PathBuf) -> Self {
        let name = title(&path);
        let bytes = path.metadata().ok().map(|meta| meta.len());
        Self {
            path,
            name,
            bytes,
            marked: false,
        }
    }

    fn size_label(&self) -> String {
        let Some(bytes) = self.bytes else {
            return String::new();
        };
        let kb = bytes / 1024;
        if kb < 1024 {
            format!("{kb}Kb")
        } else {
            format!("{}.{}Mb", kb / 1024, (kb % 1024) * 10 / 1024)
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
type DecodeResult = Result<tempfile::TempPath, String>;
#[derive(Default)]

struct App {
    skins: Vec<Skin>,
    skin: usize,
    queue: Vec<Entry>,
    current: Option<usize>,
    track: Option<Track>,
    #[cfg(not(target_arch = "wasm32"))]
    decode: Option<mpsc::Receiver<DecodeResult>>,
    status: String,
    volume: u32,
    repeat: bool,
    selected: usize,
    play_mode: PlayMode,
    played: Vec<usize>,
    view: View,
    menu_row: usize,
    playlist_row: usize,
    playlist_top: usize,
    playlist_menu: bool,
    visualization: Visualization,
    spectrum: spectrum::Spectrum,
    remaining: bool,
    play_minimized: bool,
    idle_timeout: usize,
    #[cfg(not(target_arch = "wasm32"))]
    last_input: Option<std::time::Instant>,
    #[cfg(not(target_arch = "wasm32"))]
    paused_by_window: bool,
    #[cfg(not(target_arch = "wasm32"))]
    state_dir: Option<PathBuf>,
    dirty: bool,
    context_row: usize,
    file_context: bool,
    reorder: bool,
    #[cfg(not(target_arch = "wasm32"))]
    scan: Option<mpsc::Receiver<std::io::Result<Vec<PathBuf>>>>,
}

impl App {
    #[cfg(not(target_arch = "wasm32"))]
    fn restore(&mut self) {
        let Some(dir) = self.state_dir.clone() else {
            return;
        };
        if let Ok(contents) = std::fs::read_to_string(dir.join("windows.ini")) {
            self.apply_settings(&contents);
        }
        match state::read_playlist(&dir.join("playlist.ini")) {
            Ok(playlist) => self.set_playlist(playlist),
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                self.status = error.to_string()
            }
            _ => {}
        }
    }

    fn apply_settings(&mut self, contents: &str) {
        for (key, value) in contents.lines().filter_map(|line| line.split_once('=')) {
            match (key, value) {
                ("skin", value) => self.skin = value.parse().unwrap_or(0),
                ("volume", value) => self.volume = value.parse::<u32>().unwrap_or(80).min(100),
                ("repeat", value) => self.repeat = value == "1",
                ("remaining", value) => self.remaining = value == "1",
                ("play_minimized", value) => self.play_minimized = value == "1",
                ("idle_timeout", value) => {
                    self.idle_timeout = value.parse::<usize>().unwrap_or(0).min(4)
                }
                ("mode", "single") => self.play_mode = PlayMode::Single,
                ("mode", "random") => self.play_mode = PlayMode::Random,
                ("visual", "spectrum") => self.visualization = Visualization::Spectrum,
                ("visual", "none") => self.visualization = Visualization::None,
                _ => {}
            }
        }
    }

    fn settings(&self) -> String {
        let mode = match self.play_mode {
            PlayMode::Single => "single",
            PlayMode::Continuous => "continuous",
            PlayMode::Random => "random",
        };
        let visual = match self.visualization {
            Visualization::Oscilloscope => "oscilloscope",
            Visualization::Spectrum => "spectrum",
            Visualization::None => "none",
        };
        format!(
            "skin={}\nvolume={}\nrepeat={}\nremaining={}\nmode={mode}\nvisual={visual}\nplay_minimized={}\nidle_timeout={}\n",
            self.skin, self.volume, self.repeat as u8, self.remaining as u8,
            self.play_minimized as u8,
            self.idle_timeout
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_state(&mut self) {
        let Some(dir) = &self.state_dir else { return };
        let settings = self.settings();
        if let Err(error) = self
            .export_playlist(&dir.join("playlist.ini"))
            .and_then(|()| state::atomic_write(&dir.join("windows.ini"), settings.as_bytes()))
        {
            self.status = format!("Gagal menyimpan pengaturan: {error}");
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn export_playlist(&self, path: &Path) -> std::io::Result<()> {
        state::write_playlist(
            path,
            self.current,
            self.queue
                .iter()
                .map(|entry| (entry.path.as_path(), entry.marked)),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn set_playlist(&mut self, playlist: state::Playlist) {
        self.track = None;
        self.decode = None;
        self.played.clear();
        self.spectrum = spectrum::Spectrum::default();
        self.current = playlist
            .current
            .or_else(|| (!playlist.entries.is_empty()).then_some(0));
        self.queue = playlist
            .entries
            .into_iter()
            .map(|(path, marked)| {
                let mut entry = Entry::new(path);
                entry.marked = marked;
                entry
            })
            .collect();
        self.playlist_row = self.current.unwrap_or(0);
        self.playlist_top = self.playlist_row.saturating_sub(17);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn scan_folder(&mut self) {
        if self.scan.is_some() {
            return;
        }
        let Some(folder) = choose_folder() else {
            return;
        };
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(state::scan_audio(&folder));
        });
        self.scan = Some(receive);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn update_scan(&mut self) {
        let Some(scan) = &self.scan else { return };
        let result = match scan.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(std::io::Error::other("Pemindaian berhenti."))
            }
        };
        self.scan = None;
        match result {
            Ok(mut files) => {
                let existing: std::collections::HashSet<_> =
                    self.queue.iter().map(|entry| entry.path.clone()).collect();
                files.retain(|path| !existing.contains(path));
                let count = files.len();
                self.append(files);
                self.status = format!("{count} file ditambahkan.");
            }
            Err(error) => self.status = format!("Gagal memindai folder: {error}"),
        }
    }

    fn toggle_mark(&mut self) {
        if let Some(entry) = self.queue.get_mut(self.playlist_row) {
            entry.marked = !entry.marked;
            self.dirty = true;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn file_action(&mut self, action: u8) {
        let Some(entry) = self.queue.get(self.playlist_row) else {
            return;
        };
        let source = entry.path.clone();
        let destination = if action == 0 {
            file_dialog("Semua file\0*.*\0", false, Some(&source))
                .into_iter()
                .next()
        } else {
            choose_folder().and_then(|folder| source.file_name().map(|name| folder.join(name)))
        };
        let Some(destination) = destination else {
            return;
        };
        if source == destination {
            return;
        }
        if action != 2 && self.current == Some(self.playlist_row) && self.decode.is_some() {
            self.status = "Tunggu dekoder tracker selesai.".into();
            return;
        }
        let result = if action == 2 {
            state::copy_audio(&source, &destination)
        } else {
            state::move_audio(&source, &destination)
        };
        if let Err(error) = result {
            self.status = format!("Operasi file gagal: {error}");
            return;
        }
        if action == 2 {
            self.append(vec![destination]);
        } else {
            for entry in self.queue.iter_mut().filter(|entry| entry.path == source) {
                let marked = entry.marked;
                *entry = Entry::new(destination.clone());
                entry.marked = marked;
            }
        }
        self.dirty = true;
        self.status = match action {
            0 => "Nama file diperbarui.",
            1 => "File dipindahkan.",
            _ => "File disalin.",
        }
        .into();
    }

    fn remove_from_playlist(&mut self) {
        let selected = self.playlist_row;
        if selected >= self.queue.len() {
            return;
        }
        self.queue.remove(selected);
        self.played.clear();
        self.current = match self.current {
            Some(index) if index == selected => {
                self.track = None;
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.decode = None;
                }
                self.spectrum = spectrum::Spectrum::default();
                (!self.queue.is_empty()).then_some(index.min(self.queue.len().saturating_sub(1)))
            }
            Some(index) if index > selected => Some(index - 1),
            index => index,
        };
        self.playlist_row = selected.min(self.queue.len().saturating_sub(1));
        self.dirty = true;
    }

    fn load(&mut self, index: usize) {
        self.dirty = true;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.paused_by_window = false;
            self.decode = None;
        }
        self.track = None;
        self.spectrum = spectrum::Spectrum::default();
        self.current = Some(index);
        self.status.clear();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.queue[index].path.clone();
            if tracker::is_tracker(&path) {
                let (send, receive) = mpsc::channel();
                std::thread::spawn(move || {
                    let _ = send.send(tracker::decode(&path));
                });
                self.decode = Some(receive);
            } else {
                self.open(None);
            }
        }
        #[cfg(target_arch = "wasm32")]
        match Track::open(&self.queue[index].audio, self.volume) {
            Ok(track) => self.track = Some(track),
            Err(error) => self.status = error,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open(&mut self, wav: Option<tempfile::TempPath>) {
        let Some(index) = self.current else { return };
        match Track::open(&self.queue[index].path, wav, self.volume) {
            Ok(track) => self.track = Some(track),
            Err(error) => self.status = error,
        }
    }

    fn update_audio(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(receiver) = &self.decode {
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => Err("Dekoder tracker berhenti.".into()),
            };
            self.decode = None;
            match result {
                Ok(wav) => self.open(Some(wav)),
                Err(error) => self.status = error,
            }
        }
        if self.track.as_mut().is_some_and(Track::ended) {
            match self.play_mode {
                PlayMode::Continuous => self.step(1, self.repeat),
                PlayMode::Single if self.repeat => {
                    if let Some(index) = self.current {
                        self.load(index);
                    }
                }
                PlayMode::Random => self.next_random(),
                PlayMode::Single => {}
            }
        }
    }

    fn neighbor(&self, delta: isize, wrap: bool) -> Option<usize> {
        let index = self.current?;
        let next = index as isize + delta;
        let len = self.queue.len() as isize;
        if wrap && len > 0 {
            Some(next.rem_euclid(len) as usize)
        } else if (0..len).contains(&next) {
            Some(next as usize)
        } else {
            None
        }
    }

    fn step(&mut self, delta: isize, wrap: bool) {
        if let Some(index) = self.neighbor(delta, wrap) {
            self.load(index);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn append(&mut self, files: Vec<PathBuf>) {
        if files.is_empty() {
            return;
        }
        self.dirty = true;
        self.queue.extend(files.into_iter().map(|path| {
            let path = std::path::absolute(&path).unwrap_or(path);
            Entry::new(path)
        }));
        if self.current.is_none() {
            self.load(0);
        }
    }

    fn loading(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        return self.decode.is_some();
        #[cfg(target_arch = "wasm32")]
        false
    }

    fn pick_audio(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.append(choose_files());
        #[cfg(target_arch = "wasm32")]
        {
            self.status = "Pilih Buka audio di atas pemutar.".into();
        }
    }

    fn next_random(&mut self) {
        if let Some(index) = self.current {
            self.played.push(index);
        }
        let mut remaining: Vec<_> = (0..self.queue.len())
            .filter(|i| !self.played.contains(i))
            .collect();
        if remaining.is_empty() && self.repeat {
            self.played.clear();
            remaining.extend(
                (0..self.queue.len()).filter(|i| self.queue.len() == 1 || Some(*i) != self.current),
            );
        }
        if !remaining.is_empty() {
            let index = getrandom::u64().map_or(0, |n| (n % remaining.len() as u64) as usize);
            self.load(remaining[index]);
        }
    }
    /// Dispatch order follows the six-entry jump table at 0x10004310.

    fn activate(&mut self) {
        match self.selected {
            0 => self.step(-1, false),
            5 => self.step(1, false),
            2 => {
                if self.track.as_ref().is_none_or(Track::is_finished) {
                    if let Some(index) = self.current {
                        if !self.loading() {
                            self.load(index);
                        }
                    } else {
                        self.pick_audio();
                    }
                } else if let Some(track) = &mut self.track {
                    if track.mode == Mode::Playing {
                        track.pause();
                    } else {
                        self.status = track.play_from(track.position).err().unwrap_or_default();
                    }
                }
            }
            slot => {
                if let Some(track) = &mut self.track {
                    let result = match slot {
                        1 => track.seek(track.position.saturating_sub(10_000)),
                        3 => track.stop(),
                        4 if track.duration == 0 || track.position + 10_000 < track.duration => {
                            track.seek(track.position + 10_000)
                        }
                        _ => return,
                    };
                    self.status = result.err().unwrap_or_default();
                }
            }
        }
    }

    fn set_volume(&mut self, volume: u32) {
        if self.volume == volume {
            return;
        }
        self.volume = volume;
        self.dirty = true;
        if let Some(track) = &self.track {
            track.volume(volume);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn update_window_playback(&mut self, ctx: &egui::Context) {
        let now = std::time::Instant::now();
        let (minimized, activity) = ctx.input(|input| {
            (
                input.viewport().minimized.unwrap_or(false),
                !input.events.is_empty(),
            )
        });
        if activity || minimized || self.last_input.is_none() {
            self.last_input = Some(now);
        }
        let timeout = [0, 15, 30, 60, 120][self.idle_timeout];
        let idle = !minimized
            && timeout > 0
            && self.view == View::Player
            && self
                .last_input
                .is_some_and(|last| now.duration_since(last).as_secs() >= timeout);
        if idle {
            self.last_input = Some(now);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        if let Some(track) = &mut self.track {
            if (minimized || idle) && !self.play_minimized && track.mode == Mode::Playing {
                track.pause();
                self.paused_by_window = true;
            } else if !minimized && !idle && std::mem::take(&mut self.paused_by_window) {
                if let Err(error) = track.play_from(track.position) {
                    self.status = error;
                }
            }
        }
    }

    fn player(&mut self, ui: &mut egui::Ui) {
        use egui::{Key, Modifiers, Sense};
        let scale = (ui.available_width() / SIZE.x).min(ui.available_height() / SIZE.y);
        let screen = Rect::from_center_size(ui.max_rect().center(), SIZE * scale);
        let to_screen = |offset| screen.min + offset * scale;
        let area =
            |x, y, w, h| Rect::from_min_size(to_screen(Vec2::new(x, y)), Vec2::new(w, h) * scale);
        let painter = ui.painter().with_clip_rect(screen);
        let interactive = self.view == View::Player;
        if interactive && scale > 0. {
            let range = self.skins[self.skin].volume_range();
            let meter = Rect::from_min_max(
                to_screen(range.min.to_vec2()),
                to_screen(range.max.to_vec2()),
            );
            let hit = ui.interact(
                meter.expand(3. * scale).intersect(screen),
                ui.id().with("volume"),
                Sense::click_and_drag(),
            );
            if let Some(pointer) = hit.interact_pointer_pos() {
                let level = ((meter.bottom() - pointer.y) / meter.height()).clamp(0., 1.);
                self.set_volume((level * 10.).round() as u32 * 10);
            }
            hit.on_hover_and_drag_cursor(egui::CursorIcon::ResizeVertical)
                .on_hover_text(format!("Volume {}%", self.volume));
        }
        let skin = &self.skins[self.skin];
        skin.background(&painter, screen);
        let mut pressed = false;
        let mut activate = false;
        for (slot, label) in [
            "Previous",
            "Rewind",
            "Play / Pause",
            "Stop",
            "Forward",
            "Next",
        ]
        .iter()
        .enumerate()
        {
            let pos = skin.button_at(slot);
            let hit = ui
                .interact(
                    area(pos.x - 1., pos.y - 1., 14., 15.),
                    ui.id().with(slot),
                    Sense::click(),
                )
                .on_hover_text(*label);
            if interactive && (hit.is_pointer_button_down_on() || hit.clicked()) {
                self.selected = slot;
                pressed = hit.is_pointer_button_down_on();
                activate |= hit.clicked();
            }
        }
        let menu = ui.interact(
            area(0., 186., 45., 22.),
            ui.id().with("menu"),
            Sense::click(),
        );
        let close = ui.interact(
            area(134., 186., 42., 22.),
            ui.id().with("exit"),
            Sense::click(),
        );
        let (left, right, up, down, enter, enter_held, open, show_menu) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::Enter),
                i.key_down(Key::Enter),
                i.modifiers.matches_logically(Modifiers::CTRL) && i.key_pressed(Key::O),
                i.key_pressed(Key::F10),
            )
        });
        if interactive {
            if left {
                self.selected = (self.selected + SLOTS - 1) % SLOTS;
            }
            if right {
                self.selected = (self.selected + 1) % SLOTS;
            }
        }
        // The original overlay paths call 0x10004264 with transport drawing disabled.
        if interactive {
            skin.buttons(
                &painter,
                screen,
                self.selected,
                self.track.as_ref().is_some_and(|t| t.mode == Mode::Playing),
                pressed || enter_held,
            );
        }
        let volume_at = skin.component(6).0;
        // 0x10003518 draws sprites 9, 8, ... as the volume rises.
        for id in (10 - (self.volume / 10) as u8)..10 {
            skin.sprite(&painter, screen, id, volume_at);
        }
        let text = |component, value: &str| {
            let (at, color) = skin.component(component);
            skin.text(
                &painter,
                screen,
                at,
                value,
                color,
                ((SIZE.x - at.x) / 5.) as usize,
            );
        };
        if let Some(index) = self.current {
            let name = self
                .track
                .as_ref()
                .and_then(|track| track.info.title.as_deref())
                .unwrap_or(&self.queue[index].name);
            // 0x1000263c limits the song field to 20 glyphs.
            let (at, color) = skin.component(0);
            skin.text(&painter, screen, at, name, color, 20);
            text(1, &format!("{}/{}", index + 1, self.queue.len()));
        }
        if let Some(track) = &self.track {
            text(2, &time(track.duration));
            let position = if self.remaining {
                track.duration.saturating_sub(track.position)
            } else {
                track.position
            };
            let blink = track.mode != Mode::Playing || position % 1000 >= 500;
            skin.clock(&painter, screen, position, self.remaining, blink);
            let (at, color) = skin.component(5);
            if let Some(kbps) = track.info.average_kbps {
                skin.text(&painter, screen, at, &format!("{kbps} kbit"), color, 12);
            }
            let khz = track.info.sample_rate as f32 / 1000.;
            skin.text(
                &painter,
                screen,
                at + Vec2::new(0., 8.),
                &format!("{khz:.1} kHz"),
                color,
                12,
            );
            let (at, label_color) = skin.component(4);
            let value_color = skin.component_color(4, 1);
            // 0x10002d20–0x10003074 uses fixed 8 px lines and a 10 px indent.
            for (row, (label, value)) in [
                ("Title:", &track.info.title),
                ("Author:", &track.info.artist),
                ("Album:", &track.info.album),
            ]
            .into_iter()
            .enumerate()
            {
                let line = at + Vec2::new(0., row as f32 * 16.);
                skin.text(&painter, screen, line, label, label_color, 27);
                if let Some(value) = value {
                    skin.text(
                        &painter,
                        screen,
                        line + Vec2::new(10., 8.),
                        value,
                        value_color,
                        27,
                    );
                }
            }
            let mut y = at.y + 48.;
            for (label, value) in [("Year:", &track.info.year), ("Genre:", &track.info.genre)] {
                if let Some(value) = value {
                    skin.text(&painter, screen, Vec2::new(at.x, y), label, label_color, 6);
                    skin.text(
                        &painter,
                        screen,
                        Vec2::new(at.x + 30., y),
                        value,
                        value_color,
                        20,
                    );
                    y += 8.;
                }
            }
            if track.mode == Mode::Playing {
                if let Some(wave) = &track.wave {
                    match self.visualization {
                        Visualization::Oscilloscope => {
                            skin.oscilloscope(&painter, screen, wave, track.info.stereo)
                        }
                        Visualization::Spectrum => {
                            self.spectrum.update(wave, skin.component(9).0.x as usize);
                            skin.spectrum_bars(&painter, screen, &self.spectrum);
                        }
                        Visualization::None => {}
                    }
                }
            }
        }
        if !self.status.is_empty() {
            let error = ui.fonts(|fonts| {
                fonts.layout(
                    self.status.clone(),
                    FontId::proportional(8. * scale),
                    Color32::WHITE,
                    164. * scale,
                )
            });
            let pos = to_screen(Vec2::new(6., 64.));
            painter.rect_filled(
                Rect::from_min_size(pos, error.size()),
                0.,
                Color32::from_black_alpha(220),
            );
            painter.galley(pos, error, Color32::WHITE);
        } else if self.loading() {
            text(4, "Loading...");
        }
        if open {
            self.pick_audio();
        }
        if interactive {
            if close.clicked() {
                #[cfg(not(target_arch = "wasm32"))]
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                #[cfg(target_arch = "wasm32")]
                if let Some(track) = &mut self.track {
                    self.status = track.stop().err().unwrap_or_default();
                }
            }
            if menu.clicked() || show_menu {
                self.view = View::Menu;
            } else if ui.input(|i| i.key_pressed(Key::Tab)) {
                self.open_playlist();
            } else {
                if up {
                    self.set_volume((self.volume + 10).min(100));
                }
                if down {
                    self.set_volume(self.volume.saturating_sub(10));
                }
                if activate || enter {
                    self.activate();
                }
            }
        }
    }

    fn overlay(&mut self, ui: &mut egui::Ui) {
        use egui::{Key, Sense, Stroke, StrokeKind};
        let scale = (ui.available_width() / SIZE.x).min(ui.available_height() / SIZE.y);
        let screen = Rect::from_center_size(ui.max_rect().center(), SIZE * scale);
        let area = |x, y, w, h| {
            Rect::from_min_size(
                screen.min + Vec2::new(x, y) * scale,
                Vec2::new(w, h) * scale,
            )
        };
        let painter = ui.painter().with_clip_rect(screen);
        let skin = &self.skins[self.skin];
        let (y, height, fill, border) = match self.view {
            // E32 0x10006d4c: x=(176-164)/2, y=(208-168)/2+8.
            View::Menu => (
                28.,
                168.,
                Color32::from_rgba_unmultiplied(0, 0, 64, 192),
                Color32::from_rgb(128, 128, 192),
            ),
            // E32 0x100072bc: centered, with a muted second palette under FILE.
            View::Playlist if self.playlist_menu => (
                20.,
                168.,
                Color32::from_rgba_unmultiplied(32, 0, 0, 192),
                Color32::from_rgb(96, 64, 64),
            ),
            View::Playlist => (
                20.,
                168.,
                Color32::from_rgba_unmultiplied(64, 0, 0, 192),
                Color32::from_rgb(192, 128, 128),
            ),
            // E32 0x10008344: about uses the taller rectangle.
            View::About => (
                13.,
                182.,
                Color32::from_rgba_unmultiplied(64, 64, 64, 192),
                Color32::from_gray(192),
            ),
            View::Player => return,
        };
        let orbit = skin.is_orbit();
        let orbit_fill = Color32::from_rgba_unmultiplied(6, 19, 38, 242);
        let orbit_edge = Color32::from_rgb(90, 158, 188);
        let orbit_accent = Color32::from_rgb(134, 234, 255);
        let (fill, border) = if orbit {
            (orbit_fill, orbit_edge)
        } else {
            (fill, border)
        };
        let popup_fill = if orbit {
            orbit_fill
        } else {
            Color32::from_rgba_unmultiplied(64, 0, 0, 192)
        };
        let popup_border = if orbit {
            orbit_edge
        } else {
            Color32::from_rgb(192, 128, 128)
        };
        let focus = if orbit { orbit_accent } else { Color32::RED };
        let panel = area(6., y, 164., height);
        painter.rect_filled(panel, 0., fill);
        painter.rect_stroke(
            area(5., y - 1., 166., height + 2.),
            0.,
            Stroke::new(scale, border),
            StrokeKind::Inside,
        );
        let label = |x, y, value: &str, color, max| {
            skin.text(&painter, screen, Vec2::new(x, y), value, color, max)
        };
        let hit = |name: &str, bounds| ui.interact(bounds, ui.id().with(name), Sense::click());
        let (escape, up, down, enter, tab, scroll) = ui.input(|i| {
            (
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Tab),
                i.smooth_scroll_delta.y,
            )
        });
        let back = hit("overlay back", area(127., 190., 49., 18.)).clicked();
        if escape && self.view == View::Playlist && self.reorder {
            self.reorder = false;
            return;
        }
        if escape && self.view == View::Playlist && self.playlist_menu {
            self.playlist_menu = false;
            return;
        }
        if escape || back || tab {
            self.view = View::Player;
            return;
        }
        let centered = |y, value: &str, color| {
            label(
                ((176 - value.chars().count().min(35) * 5) / 2) as f32,
                y,
                value,
                color,
                35,
            );
        };
        if self.view == View::About {
            centered(17., "ABOUT UltraMP3", Color32::WHITE);
            centered(37., "Copyright", Color32::WHITE);
            centered(45., "(c) Lonely Cat Games", Color32::WHITE);
            centered(61., "www.lonelycatgames.com", Color32::WHITE);
            centered(81., "UltraMP3 1.52", Color32::from_gray(192));
            return;
        }
        if self.view == View::Menu {
            centered(32., "MAIN MENU", Color32::WHITE);
            let skin_name = if skin.name == "<DEFAULT>" {
                "<DEFAULT>".into()
            } else {
                skin.name.to_uppercase()
            };
            let rows = [
                (
                    0,
                    "EDIT PLAYLIST",
                    String::new(),
                    [
                        "Playlist editor",
                        "allows you to browse/arrange",
                        "files on your file system",
                    ],
                ),
                (
                    1,
                    "PLAY MODE:",
                    self.play_mode.label().into(),
                    ["Configures the playback mode", "", ""],
                ),
                (
                    2,
                    "REPEAT:",
                    if self.repeat { "YES" } else { "NO" }.into(),
                    [
                        "Check to repeat song or",
                        "playlist, uncheck to",
                        "play one time only",
                    ],
                ),
                (
                    3,
                    "VISUALIZATION:",
                    match self.visualization {
                        Visualization::Oscilloscope => "OSCILOSCOPE",
                        other => other.label(),
                    }
                    .into(),
                    ["Selection of", "visualization type", ""],
                ),
                #[cfg(not(target_arch = "wasm32"))]
                (
                    4,
                    "IDLE TIMEOUT:",
                    ["NEVER", "15 Sec", "30 Sec", "1 Min", "2 Min"][self.idle_timeout].into(),
                    [
                        "Timeout after which player",
                        "switches itself to",
                        "minimized mode",
                    ],
                ),
                (
                    5,
                    "TIME DISPLAY:",
                    if self.remaining {
                        "REMAINING"
                    } else {
                        "NORMAL"
                    }
                    .into(),
                    ["Song time display -", "normal or countdown", ""],
                ),
                #[cfg(not(target_arch = "wasm32"))]
                (
                    6,
                    "PLAY MINIMIZED:",
                    if self.play_minimized { "YES" } else { "NO" }.into(),
                    [
                        "Keep playing sound also",
                        "when player is not active",
                        "application",
                    ],
                ),
                (
                    7,
                    "SKIN:",
                    skin_name,
                    ["Selection of current skin", "Press to change skin", ""],
                ),
                (
                    8,
                    "ABOUT",
                    String::new(),
                    ["Show information about", "developer company", ""],
                ),
            ];
            if up {
                self.menu_row = (self.menu_row + rows.len() - 1) % rows.len();
            }
            if down {
                self.menu_row = (self.menu_row + 1) % rows.len();
            }
            let mut action = enter.then_some(self.menu_row);
            for (row, (_, name, value, _)) in rows.iter().enumerate() {
                let y = 48. + row as f32 * 10.;
                let selected = self.menu_row == row;
                let color = if selected {
                    focus
                } else {
                    Color32::from_gray(192)
                };
                label(11., y, name, color, 31);
                label(
                    11. + (name.chars().count() + 1) as f32 * 5.,
                    y,
                    value,
                    color,
                    30 - name.chars().count(),
                );
                if hit(*name, area(7., y - 1., 162., 10.)).clicked() {
                    self.menu_row = row;
                    action = Some(row);
                }
            }
            for (i, description) in rows[self.menu_row].3.iter().enumerate() {
                centered(164. + i as f32 * 8., description, Color32::from_gray(128));
            }
            let action = action.map(|row| rows[row].0);
            if action.is_some_and(|id| id > 0 && id < 8) {
                self.dirty = true;
            }
            match action {
                Some(0) => self.open_playlist(),
                Some(1) => {
                    self.play_mode = self.play_mode.next();
                    self.played.clear();
                }
                Some(2) => self.repeat = !self.repeat,
                Some(3) => {
                    self.visualization = self.visualization.next();
                    self.spectrum = spectrum::Spectrum::default();
                }
                #[cfg(not(target_arch = "wasm32"))]
                Some(4) => {
                    self.idle_timeout = (self.idle_timeout + 1) % 5;
                    self.last_input = Some(std::time::Instant::now());
                }
                Some(5) => self.remaining = !self.remaining,
                #[cfg(not(target_arch = "wasm32"))]
                Some(6) => self.play_minimized = !self.play_minimized,
                Some(7) => {
                    self.skin = (self.skin + 1) % self.skins.len();
                    self.spectrum = spectrum::Spectrum::default();
                }
                Some(8) => self.view = View::About,
                _ => {}
            }
            return;
        }
        label(68., 22., "PLAYLIST", Color32::WHITE, 8);
        if !self.playlist_menu && ui.input(|i| i.key_pressed(Key::F2)) {
            if let Some(entry) = self.queue.get_mut(self.playlist_row) {
                entry.marked = !entry.marked;
                self.dirty = true;
            }
        }
        let before = self.playlist_row;
        if !self.playlist_menu && !self.queue.is_empty() {
            if up || scroll > 0. {
                self.playlist_row = (self.playlist_row + self.queue.len() - 1) % self.queue.len();
            }
            if down || scroll < 0. {
                self.playlist_row = (self.playlist_row + 1) % self.queue.len();
            }
        }
        if self.reorder && before != self.playlist_row {
            self.queue.swap(before, self.playlist_row);
            self.current = self.current.map(|index| {
                if index == before {
                    self.playlist_row
                } else if index == self.playlist_row {
                    before
                } else {
                    index
                }
            });
            self.played.clear();
            self.dirty = true;
        }
        let mut opened_by_enter = false;
        if !self.playlist_menu && self.reorder && enter {
            self.reorder = false;
        } else if !self.playlist_menu && enter && !self.queue.is_empty() {
            self.playlist_menu = true;
            self.file_context = true;
            self.context_row = 0;
            opened_by_enter = true;
        }
        const VISIBLE: usize = 18;
        let limit = self.queue.len().saturating_sub(VISIBLE);
        if self.playlist_row < self.playlist_top + 3 {
            self.playlist_top = self.playlist_row.saturating_sub(3);
        } else if self.playlist_row > self.playlist_top + 14 {
            self.playlist_top = (self.playlist_row - 14).min(limit);
        }
        let mut play = None;
        let mut context = hit("playlist menu", area(0., 190., 44., 18.)).clicked();
        let mut file_context = false;
        for (row, entry) in self
            .queue
            .iter()
            .enumerate()
            .skip(self.playlist_top)
            .take(VISIBLE)
        {
            let y = 30. + (row - self.playlist_top) as f32 * 8.;
            if row == self.playlist_row {
                let gray = if entry.marked { 96 } else { 48 };
                let tint = if self.reorder {
                    Color32::YELLOW
                } else if orbit {
                    Color32::from_rgb(19, 68, 98)
                } else {
                    Color32::from_gray(if self.playlist_menu { gray / 2 } else { gray })
                };
                painter.rect_filled(area(12., y - 1., 157., 8.), 0., tint);
            }
            let gray = if entry.marked { 255 } else { 128 };
            let row_color = if self.reorder && row == self.playlist_row {
                Color32::BLACK
            } else if orbit && entry.marked {
                orbit_accent
            } else {
                Color32::from_gray(if self.playlist_menu { gray / 2 } else { gray })
            };
            let number = format!("{:>2}.", row + 1);
            let x = 13. + number.chars().count() as f32 * 5.;
            label(13., y, &number, row_color, 5);
            label(
                x,
                y,
                &entry.name,
                row_color,
                ((142. - x) / 5.).max(0.) as usize,
            );
            let bytes = entry.size_label();
            label(
                164. - bytes.chars().count() as f32 * 5.,
                y,
                &bytes,
                row_color,
                5,
            );
            let response = hit(&format!("playlist {row}"), area(12., y - 1., 157., 8.));
            if !self.reorder
                && !self.playlist_menu
                && (response.clicked() || response.secondary_clicked())
            {
                self.playlist_row = row;
            }
            if !self.reorder && !self.playlist_menu && response.double_clicked() {
                play = Some(row);
            }
            if !self.reorder && !self.playlist_menu && response.secondary_clicked() {
                context = true;
                file_context = true;
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let scanning = self.scan.is_some();
        #[cfg(target_arch = "wasm32")]
        let scanning = false;
        if scanning || !self.status.is_empty() {
            let message = if scanning {
                "Scanning..."
            } else {
                &self.status
            };
            painter.rect_filled(area(7., 176., 162., 10.), 0., Color32::from_rgb(64, 16, 16));
            label(9., 178., message, Color32::WHITE, 32);
        }
        if context {
            self.playlist_menu = !self.playlist_menu;
            self.file_context = file_context;
            self.context_row = 0;
        }
        if self.playlist_menu && self.file_context {
            let marked = self
                .queue
                .get(self.playlist_row)
                .is_some_and(|entry| entry.marked);
            let rows = [
                "Play",
                if marked { "Unmark" } else { "Mark" },
                "Remove",
                "Move",
            ];
            let top =
                (20. + self.playlist_row.saturating_sub(self.playlist_top) as f32 * 8.).min(146.);
            painter.rect_filled(area(80., top, 90., 60.), 0., popup_fill);
            painter.rect_stroke(
                area(79., top - 1., 92., 62.),
                0.,
                Stroke::new(scale, popup_border),
                StrokeKind::Inside,
            );
            label(115., top + 4., "FILE", Color32::WHITE, 4);
            if up {
                self.context_row = (self.context_row + 3) % 4;
            }
            if down {
                self.context_row = (self.context_row + 1) % 4;
            }
            let mut action = (enter && !context && !opened_by_enter).then_some(self.context_row);
            for (row, item) in rows.iter().enumerate() {
                let y = top + 16. + row as f32 * 8.;
                let color = if row == self.context_row {
                    painter.rect_filled(area(81., y - 1., 87., 8.), 0., Color32::from_gray(128));
                    Color32::WHITE
                } else {
                    Color32::from_gray(128)
                };
                label(82., y, item, color, 17);
                if hit(*item, area(81., y - 1., 87., 8.)).clicked() {
                    action = Some(row);
                }
            }
            if let Some(action) = action {
                self.playlist_menu = false;
                match action {
                    0 => play = Some(self.playlist_row),
                    1 => self.toggle_mark(),
                    2 => self.remove_from_playlist(),
                    3 => self.reorder = true,
                    _ => {}
                }
            }
        } else if self.playlist_menu {
            let mark = if self
                .queue
                .get(self.playlist_row)
                .is_some_and(|entry| entry.marked)
            {
                "Unmark"
            } else {
                "Mark"
            };
            #[cfg(not(target_arch = "wasm32"))]
            let options = [
                "Tambah audio...",
                "Rescan file system",
                "Sort list",
                "Play",
                mark,
                "Remove from playlist",
                "Import playlist.ini",
                "Export playlist.ini",
                "Rename",
                "Move",
                "Copy",
            ];
            #[cfg(target_arch = "wasm32")]
            let options = [
                "Tambah audio...",
                "Sort list",
                "Play",
                mark,
                "Remove from playlist",
            ];
            if up {
                self.context_row = (self.context_row + options.len() - 1) % options.len();
            }
            if down {
                self.context_row = (self.context_row + 1) % options.len();
            }
            let popup = area(23., 65., 130., 14. + options.len() as f32 * 9.);
            painter.rect_filled(popup, 0., popup_fill);
            painter.rect_stroke(
                popup,
                0.,
                Stroke::new(scale, popup_border),
                StrokeKind::Inside,
            );
            centered(69., "FILE", Color32::WHITE);
            let mut action = (enter && !context).then_some(self.context_row);
            for (row, text) in options.iter().enumerate() {
                let y = 78. + row as f32 * 9.;
                let color = if row == self.context_row {
                    focus
                } else {
                    Color32::from_gray(192)
                };
                label(28., y, text, color, 24);
                if hit(text, area(25., y - 1., 126., 9.)).clicked() {
                    action = Some(row);
                }
            }
            if let Some(action) = action {
                self.playlist_menu = false;
                #[cfg(not(target_arch = "wasm32"))]
                match action {
                    0 => self.pick_audio(),
                    1 => self.scan_folder(),
                    2 => self.sort_playlist(),
                    3 if !self.queue.is_empty() => play = Some(self.playlist_row),
                    4 => self.toggle_mark(),
                    5 => self.remove_from_playlist(),
                    6 => {
                        if let Some(path) = playlist_dialog(false) {
                            match state::read_playlist(&path) {
                                Ok(playlist) => {
                                    self.set_playlist(playlist);
                                    self.dirty = true;
                                    self.status.clear();
                                }
                                Err(error) => {
                                    self.status = format!("Gagal membaca playlist: {error}")
                                }
                            }
                        }
                    }
                    7 => {
                        if let Some(path) = playlist_dialog(true) {
                            match self.export_playlist(&path) {
                                Ok(()) => self.status = "Playlist disimpan.".into(),
                                Err(error) => {
                                    self.status = format!("Gagal mengekspor playlist: {error}")
                                }
                            }
                        }
                    }
                    8 => self.file_action(0),
                    9 => self.file_action(1),
                    10 => self.file_action(2),
                    _ => {}
                }
                #[cfg(target_arch = "wasm32")]
                match action {
                    0 => self.pick_audio(),
                    1 => self.sort_playlist(),
                    2 if !self.queue.is_empty() => play = Some(self.playlist_row),
                    3 => self.toggle_mark(),
                    4 => self.remove_from_playlist(),
                    _ => {}
                }
            }
        }
        if let Some(index) = play {
            self.load(index);
            self.view = View::Player;
            self.playlist_menu = false;
        }
    }

    fn open_playlist(&mut self) {
        self.playlist_row = self.current.unwrap_or(0);
        self.playlist_top = self.playlist_row.saturating_sub(17);
        self.playlist_menu = self.queue.is_empty();
        self.file_context = false;
        self.reorder = false;
        self.view = View::Playlist;
    }

    fn sort_playlist(&mut self) {
        self.dirty = true;
        self.played.clear();
        let mut indexed: Vec<_> = std::mem::take(&mut self.queue)
            .into_iter()
            .enumerate()
            .collect();
        indexed.sort_by_cached_key(|(_, entry)| entry.name.to_lowercase());
        self.current = self
            .current
            .and_then(|old| indexed.iter().position(|(i, _)| *i == old));
        let selected = self.playlist_row;
        self.playlist_row = indexed
            .iter()
            .position(|(i, _)| *i == selected)
            .unwrap_or(0);
        self.queue = indexed.into_iter().map(|(_, entry)| entry).collect();
    }
}

impl eframe::App for App {
    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.skins.is_empty() {
            self.skins = Skin::load(ctx);
            if self.skin >= self.skins.len() {
                self.skin = 0;
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
            let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.update_scan();
        self.update_audio();
        #[cfg(not(target_arch = "wasm32"))]
        self.update_window_playback(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::BLACK))
            .show(ctx, |ui| {
                let overlay_open = self.view != View::Player;
                self.player(ui);
                if overlay_open {
                    self.overlay(ui);
                }
            });
        if std::mem::take(&mut self.dirty) {
            #[cfg(not(target_arch = "wasm32"))]
            self.save_state();
            #[cfg(target_arch = "wasm32")]
            web::save_settings(&self.settings());
        }
        let scope_active = self.view == View::Player
            && self.visualization != Visualization::None
            && self
                .track
                .as_ref()
                .is_some_and(|track| track.mode == Mode::Playing);
        let delay = if scope_active { 16 } else { 70 };
        ctx.request_repaint_after(std::time::Duration::from_millis(delay));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(SIZE * 3.)
            .with_min_inner_size(SIZE),
        ..Default::default()
    };
    eframe::run_native(
        "UltraMP3 Windows",
        options,
        Box::new(|_| {
            let mut app = App {
                selected: 2,
                skin: 1,
                volume: 80,
                play_minimized: true,
                state_dir: state::directory(),
                ..Default::default()
            };
            app.restore();
            let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
            if !args.is_empty() {
                let first_added = app.queue.len();
                let was_empty = app.current.is_none();
                app.append(args);
                if !was_empty && app.queue.len() > first_added {
                    app.load(first_added);
                }
            }
            Ok(Box::new(app))
        }),
    )
}
#[cfg(all(test, not(target_arch = "wasm32")))]

mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    #[ignore = "requires the bundled Windows decoder and a working audio device"]

    fn plays_and_advances_between_sis_demos() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("extracted");
        let mut app = App {
            volume: 70,
            ..Default::default()
        };
        app.append(vec![
            directory.join("05_0_UltraDemo.mp3"),
            directory.join("04_0_UltraDemo.s3m"),
        ]);
        assert!(app.track.is_some(), "{}", app.status);
        let track = app.track.as_mut().unwrap();
        assert!(track.duration > 1_000);
        std::thread::sleep(Duration::from_millis(200));
        track.read_position();
        assert!(track
            .wave
            .is_some_and(|frame| frame.iter().flatten().any(|&sample| sample != 0)));
        track.pause();
        track.seek(500).unwrap();
        track.play_from(track.position).unwrap();
        track.play_from(track.duration - 200).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.current != Some(1) || app.decode.is_some() {
            assert!(Instant::now() < deadline, "{}", app.status);
            app.update_audio();
            std::thread::sleep(Duration::from_millis(100));
        }
        let tracker = app
            .track
            .as_mut()
            .unwrap_or_else(|| panic!("{}", app.status));
        assert!(tracker.duration > 1_000);
        assert!(tracker._wav.is_some());
        tracker.read_position();
        assert!(tracker.position > 0);
        tracker.volume(40);
        tracker.stop().unwrap();
        assert_eq!(tracker.position, 0);
    }

    #[test]
    fn restores_selected_track_and_settings_without_starting_playback() {
        let root = tempfile::tempdir().unwrap();
        let mut app = App {
            state_dir: Some(root.path().into()),
            volume: 40,
            skin: 3,
            play_mode: PlayMode::Random,
            visualization: Visualization::Spectrum,
            repeat: true,
            remaining: true,
            play_minimized: false,
            idle_timeout: 2,
            ..Default::default()
        };
        app.set_playlist(state::Playlist {
            current: Some(1),
            entries: vec![
                (PathBuf::from(r"C:\Music\one.mp3"), true),
                (PathBuf::from(r"C:\Music\two.s3m"), false),
            ],
        });
        app.save_state();
        let mut restored = App {
            state_dir: app.state_dir.clone(),
            ..Default::default()
        };
        restored.restore();
        assert_eq!(restored.current, Some(1));
        assert!(restored.queue[0].marked);
        assert!(restored.track.is_none());
        assert_eq!(restored.skin, 3);
        assert_eq!(restored.volume, 40);
        assert!(restored.repeat && restored.remaining);
        assert!(matches!(restored.play_mode, PlayMode::Random));
        assert!(matches!(restored.visualization, Visualization::Spectrum));
        assert_eq!(restored.idle_timeout, 2);
    }
}
