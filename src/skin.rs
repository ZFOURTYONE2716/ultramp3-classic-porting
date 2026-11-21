use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, Vec2};
use std::collections::BTreeMap;
const SPRITES: &str = include_str!("../assets/spr.txt");
type SkinData = (
    &'static str,
    &'static [u8],
    &'static [u8],
    &'static str,
    bool,
);
const SKIN_DATA: [SkinData; 6] = [
    (
        "Default HD",
        include_bytes!("../assets/default-hd/skin.png"),
        include_bytes!("../assets/default-hd/sprites.png"),
        include_str!("../assets/default/skin.ini"),
        true,
    ),
    (
        "<DEFAULT>",
        include_bytes!("../assets/default/skin.png"),
        include_bytes!("../assets/default/sprites.png"),
        include_str!("../assets/default/skin.ini"),
        false,
    ),
    (
        "Tiger",
        include_bytes!("../assets/tiger/skin.png"),
        include_bytes!("../assets/tiger/sprites.png"),
        include_str!("../assets/tiger/skin.ini"),
        false,
    ),
    (
        "Technic",
        include_bytes!("../assets/technic/skin.png"),
        include_bytes!("../assets/technic/sprites.png"),
        include_str!("../assets/technic/skin.ini"),
        false,
    ),
    (
        "Ferrari",
        include_bytes!("../assets/ferrari/skin.png"),
        include_bytes!("../assets/ferrari/sprites.png"),
        include_str!("../assets/ferrari/skin.ini"),
        false,
    ),
    (
        "Orbit",
        include_bytes!("../assets/orbit/background.png"),
        include_bytes!("../assets/orbit/sprites.png"),
        include_str!("../assets/orbit/skin.ini"),
        true,
    ),
];
pub const SIZE: Vec2 = Vec2::new(176., 208.);
pub const SLOTS: usize = 6;
#[derive(Clone, Copy)]

struct Sprite {
    source: Rect,
    origin: Vec2,
}

impl Sprite {
    fn rect_at(self, at: Vec2) -> Rect {
        Rect::from_min_size((at - self.origin).to_pos2(), self.source.size())
    }
}

pub struct Skin {
    pub name: &'static str,
    background: TextureHandle,
    font: TextureHandle,
    sprites: TextureHandle,
    layout: BTreeMap<u8, (Vec2, [Color32; 3])>,
    definitions: BTreeMap<u8, Sprite>,
}

fn lines(input: &str) -> impl Iterator<Item = (u8, &str)> {
    input.lines().filter_map(|line| {
        let (id, fields) = line.split_once(':')?;
        Some((id.trim().parse().ok()?, fields))
    })
}

fn texture(ctx: &egui::Context, name: &str, bytes: &[u8], hd: bool) -> TextureHandle {
    let image = image::load_from_memory(bytes).expect("embedded skin asset");
    let limit = ctx.input(|input| input.max_texture_side) as u32;
    let rgba = if image.width().max(image.height()) > limit {
        image
            .resize(limit, limit, image::imageops::FilterType::Triangle)
            .to_rgba8()
    } else {
        image.to_rgba8()
    };
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(
            [rgba.width() as usize, rgba.height() as usize],
            rgba.as_raw(),
        ),
        if hd {
            egui::TextureOptions::LINEAR
        } else {
            egui::TextureOptions::NEAREST
        },
    )
}

impl Skin {
    pub fn load(ctx: &egui::Context) -> Vec<Self> {
        let definitions: BTreeMap<_, _> = lines(SPRITES)
            .map(|(id, fields)| {
                let values: Vec<f32> = fields
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|part| !part.is_empty())
                    .map(|part| part.parse().expect("sprite coordinate"))
                    .collect();
                (
                    id,
                    Sprite {
                        source: Rect::from_min_size(
                            Pos2::new(values[0], values[1]),
                            Vec2::new(values[2], values[3]),
                        ),
                        origin: Vec2::new(values[4], values[5]),
                    },
                )
            })
            .collect();
        let font = texture(
            ctx,
            "Original bitmap font",
            include_bytes!("../assets/font.png"),
            false,
        );
        let hd_font = texture(
            ctx,
            "HD bitmap font",
            include_bytes!("../assets/default-hd/font.png"),
            true,
        );
        SKIN_DATA
            .iter()
            .map(|&(name, bg, sprites, config, hd)| {
                let layout = lines(config)
                    .map(|(id, fields)| {
                        let mut values = fields.split_whitespace();
                        let x = values.next().unwrap().parse().expect("skin x");
                        let y = values.next().unwrap().parse().expect("skin y");
                        let colors = std::array::from_fn(|_| {
                            let rgb = values
                                .next()
                                .and_then(|s| u32::from_str_radix(s, 16).ok())
                                .unwrap_or(0xffffff);
                            Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
                        });
                        (id, (Vec2::new(x, y), colors))
                    })
                    .collect();
                Self {
                    name,
                    font: if hd { hd_font.clone() } else { font.clone() },
                    background: texture(ctx, name, bg, hd),
                    sprites: texture(ctx, &format!("{name} sprites"), sprites, hd),
                    layout,
                    definitions: definitions.clone(),
                }
            })
            .collect()
    }

    pub fn is_orbit(&self) -> bool {
        self.name == "Orbit"
    }

    pub fn component(&self, id: u8) -> (Vec2, Color32) {
        let (position, colors) = self.layout[&id];
        (position, colors[0])
    }
    /// 16 glyphs per row, 5×6 pixels. See 0x10006048–0x1000607c.

    pub fn component_color(&self, id: u8, color: usize) -> Color32 {
        self.layout[&id].1[color]
    }

    pub fn text(
        &self,
        painter: &egui::Painter,
        screen: Rect,
        at: Vec2,
        text: &str,
        color: Color32,
        max: usize,
    ) {
        let scale = screen.width() / SIZE.x;
        for (column, c) in text.chars().take(max).enumerate() {
            if c == ' ' {
                continue;
            }
            let glyph = if ('!'..='~').contains(&c) {
                c as u32 - 32
            } else {
                '?' as u32 - 32
            };
            let source = Vec2::new((glyph % 16 * 5) as f32, (glyph / 16 * 6) as f32);
            let descend = if "gjpqy".contains(c) { 1. } else { 0. };
            let dest = Rect::from_min_size(
                screen.min + (at + Vec2::new(column as f32 * 5., descend)) * scale,
                Vec2::new(5., 6.) * scale,
            );
            let uv = Rect::from_min_max(
                Pos2::new(source.x / 80., source.y / 60.),
                Pos2::new((source.x + 5.) / 80., (source.y + 6.) / 60.),
            );
            painter.image(self.font.id(), dest, uv, color);
        }
    }
    /// Sample-to-Y projection from the oscilloscope branch at 0x100039fc–0x10003b58.

    pub fn oscilloscope(
        &self,
        painter: &egui::Painter,
        screen: Rect,
        frame: &crate::scope::Frame,
        stereo: bool,
    ) {
        let (at, colors) = self.layout[&8];
        let (size, _) = self.layout[&9];
        let scale = screen.width() / SIZE.x;
        let width = (size.x as usize).min(frame.len());
        let height = size.y as i32;
        let order: &[usize] = if stereo { &[1, 0] } else { &[0] };
        for &channel in order {
            let color = colors[channel * 2];
            for (x, samples) in frame[frame.len() - width..].iter().enumerate() {
                let y = (((samples[channel] as i32) * (height - 1)) >> 16) + height / 2;
                let pixel = Rect::from_min_size(
                    screen.min + (at + Vec2::new(x as f32, y as f32)) * scale,
                    Vec2::splat(scale),
                );
                painter.rect_filled(pixel, 0., color);
            }
        }
    }

    pub fn spectrum_bars(
        &self,
        painter: &egui::Painter,
        screen: Rect,
        spectrum: &crate::spectrum::Spectrum,
    ) {
        let (at, colors) = self.layout[&8];
        let (size, _) = self.layout[&9];
        let scale = screen.width() / SIZE.x;
        let height = size.y as i32;
        let orbit = self.is_orbit();
        for x in 0..(size.x as usize).min(SIZE.x as usize) {
            let (bar, peak) = spectrum.heights(x, height);
            let floor = screen.min + (at + Vec2::new(x as f32, height as f32 - 1.)) * scale;
            if bar > 0 {
                let rect = Rect::from_min_size(
                    floor - Vec2::new(0., bar as f32 * scale),
                    Vec2::new(scale, bar as f32 * scale),
                );
                let color = if orbit {
                    colors[0].gamma_multiply(0.5 + 0.5 * bar as f32 / height as f32)
                } else {
                    spectrum.color(x)
                };
                painter.rect_filled(rect, 0., color);
            }
            if peak > 0 {
                let rect = Rect::from_min_size(
                    floor - Vec2::new(0., peak as f32 * scale),
                    Vec2::splat(scale),
                );
                painter.rect_filled(rect, 0., colors[1]);
            }
        }
    }

    pub fn background(&self, painter: &egui::Painter, screen: Rect) {
        painter.image(
            self.background.id(),
            screen,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
        if self.is_orbit() {
            self.orbit_chrome(painter, screen);
        }
    }

    fn orbit_chrome(&self, painter: &egui::Painter, screen: Rect) {
        use egui::{Align2, FontId, Stroke, StrokeKind};
        let scale = screen.width() / SIZE.x;
        let point = |x, y| screen.min + Vec2::new(x, y) * scale;
        let area = |x, y, w, h| Rect::from_min_size(point(x, y), Vec2::new(w, h) * scale);
        let cyan = Color32::from_rgb(142, 231, 250);
        let muted = Color32::from_rgb(142, 180, 205);
        let edge = Color32::from_rgba_unmultiplied(103, 197, 230, 132);
        let fill = |alpha| Color32::from_rgba_unmultiplied(4, 12, 27, alpha);
        let stroke = Stroke::new(0.55 * scale, edge);

        for (x, y, w, h, alpha) in [
            (5., 20., 166., 13., 160),
            (5., 35., 166., 18., 132),
            (5., 62., 140., 69., 80),
            (5., 141., 166., 45., 142),
            (5., 188., 166., 18., 200),
        ] {
            let panel = area(x, y, w, h);
            painter.rect_filled(panel, 1. * scale, fill(alpha));
            painter.rect_stroke(panel, 1. * scale, stroke, StrokeKind::Inside);
        }
        for x in [30., 42., 101.] {
            let (top, bottom) = if x == 30. { (20., 33.) } else { (35., 53.) };
            painter.line_segment([point(x, top), point(x, bottom)], stroke);
        }
        for y in [152., 163., 174.] {
            painter.line_segment(
                [point(6., y), point(170., y)],
                Stroke::new(
                    0.35 * scale,
                    Color32::from_rgba_unmultiplied(103, 197, 230, 65),
                ),
            );
        }

        painter.text(
            point(7., 5.),
            Align2::LEFT_TOP,
            "UltraMP3",
            FontId::monospace(12. * scale),
            Color32::from_rgb(224, 247, 255),
        );
        self.text(painter, screen, Vec2::new(140., 8.), "ORBIT", cyan, 5);
        self.text(painter, screen, Vec2::new(8., 55.), "TRACK INFO", muted, 10);
        self.text(painter, screen, Vec2::new(149., 54.), "VOL", muted, 3);
        self.text(painter, screen, Vec2::new(7., 133.), "SIGNAL", muted, 6);
        self.text(painter, screen, Vec2::new(10., 195.), "MENU", cyan, 4);
        self.text(painter, screen, Vec2::new(144., 195.), "EXIT", cyan, 4);
    }

    pub fn sprite(&self, painter: &egui::Painter, screen: Rect, id: u8, at: Vec2) {
        let sprite = self.definitions[&id];
        let scale = screen.width() / SIZE.x;
        let logical = sprite.rect_at(at);
        let dest = Rect::from_min_size(
            screen.min + logical.min.to_vec2() * scale,
            logical.size() * scale,
        );
        let tex = Vec2::new(256., 128.);
        let uv = Rect::from_min_max(
            Pos2::new(sprite.source.min.x / tex.x, sprite.source.min.y / tex.y),
            Pos2::new(sprite.source.max.x / tex.x, sprite.source.max.y / tex.y),
        );
        painter.image(self.sprites.id(), dest, uv, Color32::WHITE);
    }

    pub fn volume_range(&self) -> Rect {
        let at = self.component(6).0;
        let loudest = self.definitions[&0].rect_at(at);
        let quietest = self.definitions[&9].rect_at(at);
        let top = loudest.center().y;
        // Leave one more bar's worth of travel below the last visible bar for mute.
        let bottom = quietest.center().y + (quietest.center().y - top) / 9.;
        Rect::from_min_max(
            Pos2::new(loudest.left(), top),
            Pos2::new(quietest.right(), bottom),
        )
    }

    pub fn button_at(&self, slot: usize) -> Vec2 {
        self.component(7).0 + Vec2::new(14. * slot as f32, 0.)
    }
    /// Translation of the sprite selection loop at 0x10003610–0x1000369c.

    pub fn buttons(
        &self,
        painter: &egui::Painter,
        screen: Rect,
        selected: usize,
        playing: bool,
        pressed: bool,
    ) {
        for slot in 0..SLOTS {
            let mut id = if slot == 2 && playing {
                16
            } else {
                10 + slot as u8
            };
            if pressed && slot == selected {
                id += 10;
            }
            self.sprite(painter, screen, id, self.button_at(slot));
        }
        self.sprite(painter, screen, 43, self.button_at(selected));
    }

    /// Digit positions and blinking colon from 0x1000291c–0x10002b8c.
    pub fn clock(
        &self,
        painter: &egui::Painter,
        screen: Rect,
        milliseconds: u64,
        remaining: bool,
        blink: bool,
    ) {
        let origin = self.component(3).0;
        let minutes = milliseconds / 60_000;
        let seconds = (milliseconds / 1000) % 60;
        let draw = |sprite, x| self.sprite(painter, screen, sprite, origin + Vec2::new(x, 0.));
        if remaining {
            draw(41, if minutes > 9 { 0. } else { 12. });
        }
        if minutes > 9 {
            draw(30 + (minutes / 10).min(9) as u8, 10.);
        }
        draw(30 + (minutes % 10) as u8, 22.);
        if blink != remaining {
            draw(40, 34.);
        }
        draw(30 + (seconds / 10) as u8, 39.);
        draw(30 + (seconds % 10) as u8, 51.);
    }
}
