//! Arcade wave data, checked against the user's revision 1 program bytes.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaveConfig {
    pub icbms: u8,
    pub smart_bombs: u8,
    /// Frames to wait between movements, in unsigned 8.8 fixed point.
    pub missile_delay: u16,
    pub multiplier: u8,
    pub flier_cooldown: u16,
    /// Number of horizontal pixel movements required between flier salvos.
    pub flier_fire_distance: u16,
}

pub fn config(wave: u32) -> WaveConfig {
    let index = wave.saturating_sub(1).min(18) as usize;
    const ICBMS: [u8; 19] = [
        12, 15, 18, 12, 16, 14, 17, 10, 13, 16, 19, 12, 14, 16, 18, 14, 16, 18, 20,
    ];
    const BOMBS: [u8; 19] = [0, 0, 0, 0, 0, 1, 1, 2, 3, 4, 4, 5, 5, 6, 6, 7, 7, 7, 7];
    const DELAYS: [u16; 15] = [
        0x4d0, 0x2e0, 0x1c0, 0x108, 0xa0, 0x60, 0x40, 0x20, 0x10, 0x0a, 6, 4, 2, 1, 0,
    ];
    const FLIER_COOLDOWN: [u16; 7] = [240, 160, 128, 128, 96, 64, 32];
    const FIRE_DISTANCE: [u16; 7] = [128, 96, 64, 48, 32, 32, 16];
    let flier_index = wave.saturating_sub(2).min(6) as usize;
    WaveConfig {
        icbms: ICBMS[index],
        smart_bombs: BOMBS[index],
        missile_delay: DELAYS[index.min(14)],
        multiplier: wave.saturating_add(1).saturating_div(2).clamp(1, 6) as u8,
        flier_cooldown: FLIER_COOLDOWN[flier_index],
        flier_fire_distance: FIRE_DISTANCE[flier_index],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub background: u32,
    pub terrain: u32,
    pub incoming: u32,
    pub city: u32,
    pub interceptor: u32,
    pub city_detail: u32,
}

pub fn palette(wave: u32) -> Palette {
    const BLACK: u32 = 0x000000;
    const RED: u32 = 0xff0000;
    const GREEN: u32 = 0x00ff00;
    const BLUE: u32 = 0x0000ff;
    const YELLOW: u32 = 0xffff00;
    const CYAN: u32 = 0x00ffff;
    const MAGENTA: u32 = 0xff00ff;
    const WHITE: u32 = 0xffffff;
    const COLORS: [[u32; 6]; 10] = [
        [BLACK, YELLOW, RED, CYAN, BLUE, BLUE],
        [BLACK, YELLOW, GREEN, CYAN, BLUE, BLUE],
        [BLACK, BLUE, RED, YELLOW, GREEN, GREEN],
        [BLACK, RED, YELLOW, YELLOW, BLUE, BLUE],
        [BLUE, YELLOW, RED, MAGENTA, BLACK, BLACK],
        [CYAN, YELLOW, RED, BLACK, BLUE, BLUE],
        [MAGENTA, GREEN, BLACK, BLACK, YELLOW, YELLOW],
        [YELLOW, GREEN, BLACK, WHITE, RED, RED],
        [WHITE, RED, MAGENTA, YELLOW, GREEN, GREEN],
        [RED, YELLOW, BLACK, GREEN, BLUE, BLUE],
    ];
    let [
        background,
        terrain,
        incoming,
        city,
        interceptor,
        city_detail,
    ] = COLORS[((wave.saturating_sub(1) / 2) % 10) as usize];
    Palette {
        background,
        terrain,
        incoming,
        city,
        interceptor,
        city_detail,
    }
}
