//! Deterministic, palette-based 256 × 231 software video.
//!
//! The original display's non-square pixels are corrected by the window layer.
//! Terrain, city stamps, ammunition spacing and the 8 × 8 letter shapes follow
//! the revision 1 drawing tables; see docs/evidence/ for provenance.

use crate::game::{FlierKind, Game, MissileKind, Phase, Vec2};
use crate::waves;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 231;
const WHITE: u32 = 0xffffff;
const CYAN: u32 = 0x00ffff;

pub struct Renderer {
    pub pixels: Vec<u32>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT],
        }
    }

    /// Expand the palette framebuffer into the tightly packed RGBA8 form Quasar presents.
    pub fn write_rgba(&self, output: &mut Vec<u8>) {
        output.clear();
        output.reserve(self.pixels.len() * 4);
        for pixel in &self.pixels {
            output.extend_from_slice(&[
                (pixel >> 16) as u8,
                (pixel >> 8) as u8,
                *pixel as u8,
                0xff,
            ]);
        }
    }

    pub fn render(&mut self, game: &Game, aim: Vec2, high_score: u64) -> &[u32] {
        let palette = waves::palette(game.wave);
        // SOFTOK increments the palette byte every vblank. Bit 0 is unused,
        // producing inverted RGB steps at half the frame rate.
        let flash = [
            0xffffff, 0xffff00, 0xff00ff, 0xff0000, 0x00ffff, 0x00ff00, 0x0000ff, 0x000000,
        ][((game.tick / 2) % 8) as usize];
        self.pixels.fill(palette.background);
        self.terrain(palette.terrain);

        for city in &game.cities {
            if city.alive {
                self.city(
                    city.position.x as i32,
                    city.position.y as i32,
                    palette.city,
                    palette.city_detail,
                );
            }
        }
        const AMMO_X: [i32; 10] = [0, -3, 3, -6, 0, 6, -3, 3, -9, 9];
        const AMMO_Y: [i32; 10] = [-2, 1, 1, 4, 4, 4, 7, 7, 7, 7];
        for base in &game.bases {
            if base.alive {
                for i in 0..base.ammo.min(10) as usize {
                    let x = base.position.x as i32 + AMMO_X[i];
                    let y = base.position.y as i32 + AMMO_Y[i];
                    self.line(x, y, x, y + 4, palette.interceptor);
                    self.pixel(x - 1, y + 4, palette.interceptor);
                    self.pixel(x + 1, y + 4, palette.interceptor);
                    self.pixel(x + 1, y + 3, palette.interceptor);
                }
            }
            if !matches!(game.phase, Phase::Attract) && base.ammo <= 3 {
                let label = if base.ammo == 0 { "EMPTY" } else { "LOW" };
                self.text(
                    base.position.x as i32 - text_width(label, 1) / 2,
                    223,
                    label,
                    palette.interceptor,
                    1,
                );
            }
        }

        for missile in &game.missiles {
            let friendly = matches!(missile.kind, MissileKind::Interceptor { .. });
            let color = if friendly {
                palette.interceptor
            } else {
                palette.incoming
            };
            if !matches!(missile.kind, MissileKind::SmartBomb) {
                for point in &missile.trail {
                    self.pixel(point.x as i32, point.y as i32, color);
                }
            }
            let x = missile.position.x as i32;
            let y = missile.position.y as i32;
            if matches!(missile.kind, MissileKind::SmartBomb) {
                self.line(x, y - 3, x + 2, y, flash);
                self.line(x + 2, y, x, y + 3, flash);
                self.line(x, y + 3, x - 2, y, flash);
                self.line(x - 2, y, x, y - 3, flash);
            } else {
                self.pixel(x, y, flash);
            }
            if friendly {
                let tx = missile.target.x as i32;
                let ty = missile.target.y as i32;
                self.line(tx - 1, ty - 1, tx + 1, ty + 1, flash);
                self.line(tx - 1, ty + 1, tx + 1, ty - 1, flash);
            }
        }
        for flier in &game.fliers {
            self.flier(
                flier.position,
                flier.kind,
                flier.direction,
                palette.incoming,
            );
        }
        for explosion in &game.explosions {
            self.blast(
                explosion.position.x as i32,
                explosion.position.y as i32,
                explosion.radius as i32,
                flash,
            );
        }

        self.text(40, 0, &format!("{:>4}", game.score), palette.incoming, 1);
        self.text(
            112,
            0,
            &format!("{:>4}", high_score.max(game.score)),
            palette.incoming,
            1,
        );

        match game.phase {
            Phase::Attract => {
                self.centered(42, "MISSILE", palette.incoming, 2);
                self.centered(62, "COMMAND", palette.incoming, 2);
                self.centered(96, "DEFEND YOUR CITIES", palette.interceptor, 1);
                if (game.tick / 35).is_multiple_of(2) {
                    self.centered(122, "PRESS ENTER TO START", WHITE, 1);
                }
                self.centered(151, "MOUSE TO AIM", palette.incoming, 1);
                self.centered(163, "A / S / D TO FIRE", palette.incoming, 1);
                self.centered(182, "F1 HELP", palette.interceptor, 1);
            }
            Phase::WaveBonus { .. } => {
                self.centered(65, "BONUS POINTS", palette.incoming, 1);
                let multiplier = waves::config(game.wave).multiplier;
                self.centered(
                    85,
                    &format!("{} X POINTS", multiplier),
                    palette.interceptor,
                    1,
                );
                let ammo: u32 = game.bases.iter().map(|base| base.ammo as u32).sum();
                let cities = game.cities.iter().filter(|city| city.alive).count();
                self.centered(
                    110,
                    &format!("{} MISSILES  {} CITIES", ammo, cities),
                    palette.incoming,
                    1,
                );
                self.centered(
                    134,
                    &format!("WAVE {} COMPLETE", game.wave),
                    palette.interceptor,
                    1,
                );
                if game.bonus_cities > 0 {
                    self.centered(
                        158,
                        &format!("BONUS CITIES {}", game.bonus_cities),
                        flash,
                        1,
                    );
                }
            }
            Phase::GameOver { ticks } => {
                self.blast(128, 109, (ticks / 3).min(71) as i32, flash);
                self.centered(102, "THE END", palette.background, 2);
                if ticks > 180 {
                    self.rect(22, 163, 212, 27, palette.background);
                    self.centered(173, "ENTER TO PLAY AGAIN", palette.incoming, 1);
                }
            }
            Phase::Playing => {
                self.crosshair(aim, palette.interceptor, flash);
            }
        }
        &self.pixels
    }

    /// A window-layer menu can reuse the same bitmap text and framebuffer.
    pub fn overlay(&mut self, title: &str, lines: &[&str]) {
        for pixel in &mut self.pixels {
            *pixel = (*pixel & 0xfefefe) >> 1;
        }
        let height = (lines.len() as i32 * 13 + 44).min(HEIGHT as i32 - 20);
        let top = (HEIGHT as i32 - height) / 2;
        self.rect(6, top, 244, height, 0x070e18);
        self.line(6, top, 249, top, CYAN);
        self.line(6, top + height - 1, 249, top + height - 1, 0x264258);
        self.centered(top + 10, title, CYAN, 1);
        for (i, line) in lines.iter().enumerate() {
            self.centered(top + 31 + i as i32 * 13, line, WHITE, 1);
        }
    }

    pub fn text(&mut self, x: i32, y: i32, text: &str, color: u32, scale: i32) {
        let scale = scale.max(1);
        for (column, letter) in text.chars().enumerate() {
            for (row, bits) in glyph(letter).iter().enumerate() {
                for bit in 0..8 {
                    if bits & (0x80 >> bit) != 0 {
                        self.rect(
                            x + (column as i32 * 8 + bit) * scale,
                            y + row as i32 * scale,
                            scale,
                            scale,
                            color,
                        );
                    }
                }
            }
        }
    }

    fn centered(&mut self, y: i32, text: &str, color: u32, scale: i32) {
        self.text(
            (WIDTH as i32 - text_width(text, scale)) / 2,
            y,
            text,
            color,
            scale,
        );
    }

    fn pixel(&mut self, x: i32, y: i32, color: u32) {
        if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
            self.pixels[y as usize * WIDTH + x as usize] = color;
        }
    }

    fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        for row in y.max(0)..(y + height).min(HEIGHT as i32) {
            for column in x.max(0)..(x + width).min(WIDTH as i32) {
                self.pixels[row as usize * WIDTH + column as usize] = color;
            }
        }
    }

    fn line(&mut self, mut x: i32, mut y: i32, x2: i32, y2: i32, color: u32) {
        let dx = (x2 - x).abs();
        let dy = -(y2 - y).abs();
        let sx = if x < x2 { 1 } else { -1 };
        let sy = if y < y2 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.pixel(x, y, color);
            if x == x2 && y == y2 {
                break;
            }
            let twice = 2 * error;
            if twice >= dy {
                error += dy;
                x += sx;
            }
            if twice <= dx {
                error += dx;
                y += sy;
            }
        }
    }

    fn blast(&mut self, x: i32, y: i32, radius: i32, color: u32) {
        // Hardware drew octagonal spans with a 3/8 slope, not smooth circles.
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let major = dx.abs().max(dy.abs());
                let minor = dx.abs().min(dy.abs());
                if major + ((minor + (minor >> 1)) >> 2) <= radius {
                    self.pixel(x + dx, y + dy, color);
                }
            }
        }
    }

    fn crosshair(&mut self, point: Vec2, color: u32, flash: u32) {
        let x = point.x as i32;
        let y = point.y as i32;
        self.line(x - 3, y, x + 3, y, color);
        self.line(x, y - 3, x, y + 3, color);
        self.pixel(x, y, flash);
        self.pixel(x - 1, y - 1, flash);
        self.pixel(x + 1, y + 1, flash);
    }

    fn terrain(&mut self, color: u32) {
        // MAPTAB's inclusive horizontal runs, beginning at physical row $e6.
        const COAST: &[&[(i32, i32)]] = &[
            &[
                (12, 13),
                (29, 30),
                (117, 118),
                (129, 130),
                (233, 234),
                (250, 251),
            ],
            &[
                (11, 14),
                (28, 31),
                (117, 119),
                (128, 131),
                (231, 234),
                (249, 252),
            ],
            &[(10, 32), (116, 132), (230, 253)],
            &[(9, 33), (115, 132), (230, 254)],
            &[(8, 34), (115, 133), (229, 254)],
            &[(6, 35), (114, 133), (228, 254)],
            &[(5, 35), (113, 134), (172, 187), (227, 254)],
            &[(4, 36), (56, 57), (112, 135), (171, 188), (226, 254)],
            &[(3, 37), (55, 58), (111, 135), (170, 189), (225, 255)],
            &[(1, 38), (50, 59), (107, 152), (168, 189), (223, 255)],
            &[(0, 60), (73, 88), (103, 192), (222, 254)],
            &[(0, 90), (101, 255)],
            &[(0, 92), (100, 255)],
            &[(0, 255)],
        ];
        for y in 205..HEIGHT as i32 {
            for &(left, right) in COAST[(y as usize - 205).min(COAST.len() - 1)] {
                self.line(left, y, right, y, color);
            }
        }
    }

    fn city(&mut self, x: i32, y: i32, bottom: u32, top: u32) {
        // Four original city stamps, vertically reversed into top-down space.
        const LEFT_BOTTOM: [u8; 8] = [0x7f, 0xff, 0x7f, 0x7e, 0x34, 0x30, 0x20, 0];
        const LEFT_TOP: [u8; 8] = [0x7c, 0x37, 0x25, 0, 0, 0, 0, 0];
        const RIGHT_BOTTOM: [u8; 8] = [0xfe, 0xff, 0xfe, 0xfe, 0xfa, 0x50, 0x10, 0x10];
        const RIGHT_TOP: [u8; 8] = [0x3e, 0x6c, 0xc8, 0xc0, 0x80, 0, 0, 0];
        for (offset, stamp, color) in [
            (-8, LEFT_BOTTOM, bottom),
            (-8, LEFT_TOP, top),
            (0, RIGHT_BOTTOM, bottom),
            (0, RIGHT_TOP, top),
        ] {
            for (row, bits) in stamp.iter().enumerate() {
                for bit in 0..8 {
                    if bits & (0x80 >> bit) != 0 {
                        self.pixel(x + offset + bit, y + 3 - row as i32, color);
                    }
                }
            }
        }
    }

    fn flier(&mut self, position: Vec2, kind: FlierKind, direction: f32, color: u32) {
        let x = position.x as i32;
        let y = position.y as i32;
        let sign = if direction < 0.0 { -1 } else { 1 };
        match kind {
            FlierKind::Bomber => {
                for (dy, left, right) in [
                    (-5, -3, -2),
                    (-4, -4, -1),
                    (-3, -3, 0),
                    (-2, -2, 3),
                    (-1, -4, 5),
                    (0, -8, 7),
                    (1, -6, 3),
                    (2, -4, 1),
                    (3, -4, -1),
                    (4, -6, -2),
                    (5, -7, -3),
                ] {
                    self.line(x + left * sign, y + dy, x + right * sign, y + dy, color);
                }
            }
            FlierKind::Satellite => {
                for dy in -3..=3 {
                    self.line(x - 4, y + dy, x + 3, y + dy, color);
                }
                for side in [-1, 1] {
                    self.line(x + side * 4, y - 4, x + side * 6, y - 6, color);
                    self.line(x + side * 4, y + 4, x + side * 6, y + 6, color);
                    self.line(x + side * 2, y - 1, x + side * 2, y + 1, 0x0000ff);
                }
            }
        }
    }
}

fn text_width(text: &str, scale: i32) -> i32 {
    text.chars().count() as i32 * 8 * scale
}

fn glyph(letter: char) -> [u8; 8] {
    match letter.to_ascii_uppercase() {
        '0' => [0, 0x38, 0x44, 0xc6, 0xc6, 0xc6, 0x44, 0x38],
        '1' => [0, 0x30, 0x70, 0x30, 0x30, 0x30, 0x30, 0xfc],
        '2' => [0, 0x7c, 0xc6, 0x0e, 0x3c, 0x78, 0xe0, 0xfe],
        '3' => [0, 0x7e, 0x0c, 0x18, 0x3c, 0x06, 0xc6, 0x7c],
        '4' => [0, 0x1c, 0x3c, 0x6c, 0xcc, 0xfe, 0x0c, 0x0c],
        '5' => [0, 0xfc, 0xc0, 0xfc, 0x06, 0x06, 0xc6, 0x7c],
        '6' => [0, 0x3c, 0x60, 0xc0, 0xfc, 0xc6, 0xc6, 0x7c],
        '7' => [0, 0xfe, 0xc6, 0x0c, 0x18, 0x30, 0x30, 0x30],
        '8' => [0, 0x78, 0xc4, 0xe4, 0x78, 0x9e, 0x86, 0x7c],
        '9' => [0, 0x7c, 0xc6, 0xc6, 0x7e, 0x06, 0x0c, 0x78],
        'A' => [0, 0x38, 0x6c, 0xc6, 0xc6, 0xfe, 0xc6, 0xc6],
        'B' => [0, 0xfc, 0xc6, 0xc6, 0xfc, 0xc6, 0xc6, 0xfc],
        'C' => [0, 0x3c, 0x66, 0xc0, 0xc0, 0xc0, 0x66, 0x3c],
        'D' => [0, 0xf8, 0xcc, 0xc6, 0xc6, 0xc6, 0xcc, 0xf8],
        'E' => [0, 0xfc, 0xc0, 0xc0, 0xf8, 0xc0, 0xc0, 0xfc],
        'F' => [0, 0xfe, 0xc0, 0xc0, 0xfc, 0xc0, 0xc0, 0xc0],
        'G' => [0, 0x3e, 0x60, 0xc0, 0xce, 0xc6, 0x66, 0x3e],
        'H' => [0, 0xc6, 0xc6, 0xc6, 0xfe, 0xc6, 0xc6, 0xc6],
        'I' => [0, 0xfc, 0x30, 0x30, 0x30, 0x30, 0x30, 0xfc],
        'J' => [0, 0x06, 0x06, 0x06, 0x06, 0x06, 0xc6, 0x7c],
        'K' => [0, 0xc6, 0xcc, 0xd8, 0xf0, 0xf8, 0xdc, 0xce],
        'L' => [0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0xfc],
        'M' => [0, 0xc6, 0xee, 0xfe, 0xfe, 0xd6, 0xc6, 0xc6],
        'N' => [0, 0xc6, 0xe6, 0xf6, 0xfe, 0xde, 0xce, 0xc6],
        'O' => [0, 0x7c, 0xc6, 0xc6, 0xc6, 0xc6, 0xc6, 0x7c],
        'P' => [0, 0xfc, 0xc6, 0xc6, 0xc6, 0xfc, 0xc0, 0xc0],
        'Q' => [0, 0x7c, 0xc6, 0xc6, 0xc6, 0xde, 0xcc, 0x7a],
        'R' => [0, 0xfc, 0xc6, 0xc6, 0xce, 0xf8, 0xdc, 0xce],
        'S' => [0, 0x7c, 0xc6, 0xc0, 0x7c, 0x06, 0xc6, 0x7c],
        'T' => [0, 0xfc, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30],
        'U' => [0, 0xc6, 0xc6, 0xc6, 0xc6, 0xc6, 0xc6, 0x7c],
        'V' => [0, 0xc6, 0xc6, 0xc6, 0xee, 0x6c, 0x38, 0x10],
        'W' => [0, 0xc6, 0xc6, 0xd6, 0xfe, 0xfe, 0xee, 0xc6],
        'X' => [0, 0xc6, 0xee, 0x7c, 0x38, 0x7c, 0xee, 0xc6],
        'Y' => [0, 0xcc, 0xcc, 0xcc, 0x78, 0x30, 0x30, 0x30],
        'Z' => [0, 0xfe, 0x0e, 0x1c, 0x38, 0x70, 0xe0, 0xfe],
        ':' => [0, 0, 0x30, 0x30, 0, 0x30, 0x30, 0],
        '/' => [0, 0x06, 0x0c, 0x18, 0x30, 0x60, 0xc0, 0],
        '-' => [0, 0, 0, 0x7c, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0, 0x30, 0x30],
        '+' => [0, 0, 0x18, 0x18, 0x7e, 0x18, 0x18, 0],
        '(' => [0, 0x0c, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0c],
        ')' => [0, 0x30, 0x18, 0x0c, 0x0c, 0x0c, 0x18, 0x30],
        _ => [0; 8],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_expansion_is_opaque_and_preserves_rgb() {
        let mut renderer = Renderer::new();
        renderer.pixels[0] = 0x123456;
        let mut rgba = Vec::new();
        renderer.write_rgba(&mut rgba);
        assert_eq!(rgba.len(), WIDTH * HEIGHT * 4);
        assert_eq!(&rgba[..4], &[0x12, 0x34, 0x56, 0xff]);
        assert!(rgba.iter().skip(3).step_by(4).all(|alpha| *alpha == 0xff));
    }
}
