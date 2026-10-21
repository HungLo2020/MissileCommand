//! Deterministic, platform-independent arcade simulation.
//!
//! Positions are exposed in display pixels (top-left origin). Missile integration
//! uses integer 8.8 arithmetic. Call `update` once per hardware video frame.

use crate::waves;

pub const WIDTH: u32 = 256;
pub const HEIGHT: u32 = 231;
pub const TICK_HZ: f64 = 5_000_000.0 / 320.0 / 256.0;
pub const MAX_INTERCEPTORS: usize = 8;
pub const MAX_ENEMIES: usize = 8;
pub const MAX_EXPLOSIONS: usize = 20;
const BONUS_INTERVAL: u64 = 10_000;
const BONUS_DURATION: u32 = 240;
const RADII: [u8; 28] = [
    0, 0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}
impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Attract,
    Playing,
    WaveBonus { ticks: u32 },
    GameOver { ticks: u32 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissileKind {
    Interceptor { base: usize },
    Icbm,
    SmartBomb,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplosionKind {
    Defensive,
    Hostile,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlierKind {
    Bomber,
    Satellite,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameEvent {
    Launch { base: usize },
    Empty { base: usize },
    Explosion,
    CityDestroyed { city: usize },
    BaseDestroyed { base: usize },
    BonusCity,
    WaveStarted { wave: u32 },
    WaveCompleted,
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct City {
    pub position: Vec2,
    pub alive: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Base {
    pub position: Vec2,
    pub ammo: u8,
    pub alive: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Missile {
    pub origin: Vec2,
    pub position: Vec2,
    pub target: Vec2,
    pub kind: MissileKind,
    pub trail: Vec<Vec2>,
    fixed: [i32; 2],
    velocity: [i32; 2],
    target_index: Option<usize>,
}
impl Missile {
    fn new(origin: Vec2, target: Vec2, kind: MissileKind, target_index: Option<usize>) -> Self {
        let mut result = Self {
            origin,
            position: origin,
            target,
            kind,
            trail: vec![origin],
            fixed: [(origin.x * 256.0) as i32, (origin.y * 256.0) as i32],
            velocity: [0, 0],
            target_index,
        };
        result.recalculate_velocity();
        result
    }
    fn recalculate_velocity(&mut self) {
        let dx = (self.target.x - self.position.x).trunc() as i32;
        let dy = (self.target.y - self.position.y).trunc() as i32;
        let major = dx.abs().max(dy.abs());
        let minor = dx.abs().min(dy.abs());
        let distance = (major + (minor + minor / 2) / 4).clamp(1, 255);
        self.velocity = [dx * 256 / distance, dy * 256 / distance];
    }
    fn step(&mut self, units: i32) -> bool {
        if self.velocity == [0, 0] {
            self.position = self.target;
            return true;
        }
        let old = self.fixed;
        self.fixed[0] += self.velocity[0] * units;
        self.fixed[1] += self.velocity[1] * units;
        self.position = Vec2::new(self.fixed[0] as f32 / 256.0, self.fixed[1] as f32 / 256.0);
        let goal = [
            (self.target.x * 256.0) as i32,
            (self.target.y * 256.0) as i32,
        ];
        let reached = (0..2).any(|i| {
            self.velocity[i] != 0
                && if self.velocity[i] > 0 {
                    old[i] <= goal[i] && self.fixed[i] >= goal[i]
                } else {
                    old[i] >= goal[i] && self.fixed[i] <= goal[i]
                }
        });
        if reached {
            self.position = self.target;
        }
        if self.kind != MissileKind::SmartBomb {
            let from = Vec2::new(old[0] as f32 / 256.0, old[1] as f32 / 256.0);
            let steps = (self.position.x - from.x)
                .abs()
                .max((self.position.y - from.y).abs())
                .ceil() as i32;
            for i in 1..=steps {
                let fraction = i as f32 / steps as f32;
                let point = Vec2::new(
                    (from.x + (self.position.x - from.x) * fraction).floor(),
                    (from.y + (self.position.y - from.y) * fraction).floor(),
                );
                if self.trail.last() != Some(&point) {
                    self.trail.push(point);
                }
            }
        }
        reached
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Explosion {
    pub position: Vec2,
    pub radius: f32,
    /// Logical animation age in video frames; advances by five on its slot turn.
    pub age: u32,
    pub kind: ExplosionKind,
    slot: u8,
}
impl Explosion {
    fn new(position: Vec2, kind: ExplosionKind) -> Self {
        Self {
            position,
            radius: 0.0,
            age: 0,
            kind,
            slot: 0,
        }
    }
    fn contains(&self, point: Vec2, padding: f32) -> bool {
        let dx = (self.position.x.floor() as i32 - point.x.floor() as i32).unsigned_abs();
        let dy = (self.position.y.floor() as i32 - point.y.floor() as i32).unsigned_abs();
        let minor = dx.min(dy);
        dx.max(dy) + ((minor + (minor >> 1)) >> 2) < (self.radius + padding) as u32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Flier {
    pub position: Vec2,
    pub kind: FlierKind,
    pub direction: f32,
    movement_timer: u32,
    fire_distance: u32,
}

/// Fire flags represent button press edges, not held buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub aim: Vec2,
    pub fire: [bool; 3],
    pub start: bool,
}
impl Default for Input {
    fn default() -> Self {
        Self {
            aim: Vec2::new(128.0, 115.0),
            fire: [false; 3],
            start: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    pub phase: Phase,
    pub wave: u32,
    pub score: u64,
    pub bonus_cities: u32,
    pub cities: [City; 6],
    pub bases: [Base; 3],
    pub missiles: Vec<Missile>,
    pub explosions: Vec<Explosion>,
    pub fliers: Vec<Flier>,
    pub tick: u64,
    pub aim: Vec2,
    pub events: Vec<GameEvent>,
    /// The tallies displayed during the inter-wave bonus screen.
    pub wave_bonus_ammo: u64,
    pub wave_bonus_cities: u64,
    rng: Rng,
    initial_seed: u64,
    remaining_icbms: u8,
    remaining_smart_bombs: u8,
    enemy_delay: u16,
    enemy_fraction: u16,
    flier_cooldown: u32,
    next_bonus: u64,
    cities_lost_this_wave: u8,
    wave_ticks: u32,
    next_explosion_slot: u8,
    explosion_phase: u8,
    cleanup: bool,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Self {
            phase: Phase::Attract,
            wave: 1,
            score: 0,
            bonus_cities: 0,
            cities: initial_cities(),
            bases: initial_bases(),
            missiles: Vec::new(),
            explosions: Vec::new(),
            fliers: Vec::new(),
            tick: 0,
            aim: Input::default().aim,
            events: Vec::new(),
            wave_bonus_ammo: 0,
            wave_bonus_cities: 0,
            rng: Rng::new(seed),
            initial_seed: seed,
            remaining_icbms: 0,
            remaining_smart_bombs: 0,
            enemy_delay: 0,
            enemy_fraction: 0,
            flier_cooldown: 0,
            next_bonus: BONUS_INTERVAL,
            cities_lost_this_wave: 0,
            wave_ticks: 0,
            next_explosion_slot: 0,
            explosion_phase: 0,
            cleanup: false,
        }
    }
    pub fn start(&mut self) {
        *self = Self::new(self.initial_seed);
        self.phase = Phase::Playing;
        self.prepare_wave();
    }
    pub fn alive_cities(&self) -> usize {
        self.cities.iter().filter(|city| city.alive).count()
    }
    pub fn multiplier(&self) -> u8 {
        waves::config(self.wave).multiplier
    }
    pub fn remaining_enemies(&self) -> u16 {
        u16::from(self.remaining_icbms) + u16::from(self.remaining_smart_bombs)
    }

    /// Advance exactly one video frame. No wall-clock time, I/O, or platform RNG.
    pub fn update(&mut self, input: Input) {
        self.events.clear();
        self.tick = self.tick.wrapping_add(1);
        if input.start && !matches!(self.phase, Phase::Playing | Phase::WaveBonus { .. }) {
            self.start();
            return;
        }
        self.aim = Vec2::new(
            finite_or(input.aim.x, 128.0).clamp(8.0, 247.0),
            finite_or(input.aim.y, 115.0).clamp(24.0, 185.0),
        );
        match self.phase {
            Phase::Attract => {}
            Phase::GameOver { ticks } => {
                self.phase = Phase::GameOver {
                    ticks: ticks.saturating_add(1),
                }
            }
            Phase::WaveBonus { ticks } => {
                if ticks + 1 >= BONUS_DURATION {
                    self.restore_cities();
                    if self.alive_cities() == 0 {
                        self.finish_game();
                    } else {
                        self.wave = self.wave.saturating_add(1);
                        self.prepare_wave();
                        self.phase = Phase::Playing;
                    }
                } else {
                    self.phase = Phase::WaveBonus { ticks: ticks + 1 };
                }
            }
            Phase::Playing => self.play_tick(input),
        }
    }
    fn prepare_wave(&mut self) {
        let cfg = waves::config(self.wave);
        self.remaining_icbms = cfg.icbms;
        self.remaining_smart_bombs = cfg.smart_bombs;
        self.enemy_delay = 0;
        self.enemy_fraction = 0;
        self.flier_cooldown = u32::from(cfg.flier_cooldown);
        self.cities_lost_this_wave = 0;
        self.wave_ticks = 0;
        self.next_explosion_slot = 0;
        self.explosion_phase = 0;
        self.cleanup = false;
        self.missiles.clear();
        self.explosions.clear();
        self.fliers.clear();
        self.bases = initial_bases();
        self.wave_bonus_ammo = 0;
        self.wave_bonus_cities = 0;
        self.events.push(GameEvent::WaveStarted { wave: self.wave });
    }
    fn play_tick(&mut self, input: Input) {
        self.wave_ticks = self.wave_ticks.saturating_add(1);
        // Cabinet software pauses for two seconds after rearming each wave.
        if self.wave_ticks <= 122 {
            return;
        }
        self.launch_offense();
        for base in 0..3 {
            if input.fire[base] {
                self.fire(base);
            }
        }
        self.move_fliers(self.cleanup);
        self.move_interceptors();
        if self.enemy_delay == 0 || self.cleanup {
            let sum = self.enemy_fraction
                + if self.cleanup {
                    0
                } else {
                    waves::config(self.wave).missile_delay
                };
            self.enemy_delay = sum >> 8;
            self.enemy_fraction = sum & 255;
            self.move_enemies();
        } else {
            self.enemy_delay -= 1;
        }
        self.update_explosions();
        self.cleanup |= self.bases.iter().all(|base| base.ammo == 0)
            && !self
                .missiles
                .iter()
                .any(|m| matches!(m.kind, MissileKind::Interceptor { .. }))
            && self.explosions.is_empty();
        if self.alive_cities() == 0
            || (self.cities_lost_this_wave >= 3 && self.bases.iter().all(|base| base.ammo == 0))
        {
            self.remaining_icbms = 0;
            self.remaining_smart_bombs = 0;
        }
        if self.remaining_enemies() == 0
            && self.missiles.is_empty()
            && self.explosions.is_empty()
            && self.fliers.is_empty()
        {
            self.complete_wave();
        }
    }
    fn fire(&mut self, base: usize) {
        if self.remaining_enemies() == 0
            && self.fliers.is_empty()
            && !self
                .missiles
                .iter()
                .any(|missile| !matches!(missile.kind, MissileKind::Interceptor { .. }))
        {
            return;
        }
        let defenders = self
            .missiles
            .iter()
            .filter(|m| matches!(m.kind, MissileKind::Interceptor { .. }))
            .count();
        let defensive_explosions = self
            .explosions
            .iter()
            .filter(|e| e.kind == ExplosionKind::Defensive)
            .count();
        if !self.bases[base].alive
            || self.bases[base].ammo == 0
            || defenders >= MAX_INTERCEPTORS
            || defenders + defensive_explosions >= 16
            || self.missiles.len() + self.explosions.len() >= MAX_EXPLOSIONS
        {
            self.events.push(GameEvent::Empty { base });
            return;
        }
        self.bases[base].ammo -= 1;
        let origin = Vec2::new(
            self.bases[base].position.x,
            self.bases[base].position.y - 2.0,
        );
        self.missiles.push(Missile::new(
            origin,
            self.aim,
            MissileKind::Interceptor { base },
            None,
        ));
        self.events.push(GameEvent::Launch { base });
    }
    fn move_interceptors(&mut self) {
        let mut detonations = Vec::new();
        self.missiles.retain_mut(|m| {
            if let MissileKind::Interceptor { base } = m.kind {
                if m.step(if base == 1 { 7 } else { 3 }) {
                    detonations.push(m.target);
                    return false;
                }
            }
            true
        });
        for position in detonations {
            self.explode(position, ExplosionKind::Defensive);
        }
    }
    fn move_enemies(&mut self) {
        let mut impacts = Vec::new();
        for missile in &mut self.missiles {
            if matches!(missile.kind, MissileKind::Interceptor { .. }) {
                continue;
            }
            let reached = if missile.kind == MissileKind::SmartBomb
                && evade(missile, &self.explosions, self.wave)
            {
                false
            } else {
                missile.step(1)
            };
            if reached {
                impacts.push((missile.target_index, missile.position));
            }
        }
        self.missiles.retain(|m| {
            matches!(m.kind, MissileKind::Interceptor { .. })
                || !impacts
                    .iter()
                    .any(|(i, p)| *i == m.target_index && *p == m.position)
        });
        for (target, position) in impacts {
            if let Some(target) = target {
                if target < 6 {
                    if self.cities[target].alive {
                        self.cities[target].alive = false;
                        self.cities_lost_this_wave += 1;
                        self.events.push(GameEvent::CityDestroyed { city: target });
                    }
                } else {
                    let base = target - 6;
                    if self.bases[base].alive {
                        self.bases[base].alive = false;
                        self.bases[base].ammo = 0;
                        self.events.push(GameEvent::BaseDestroyed { base });
                    }
                }
            }
            self.explode(position, ExplosionKind::Hostile);
        }
    }
    fn move_fliers(&mut self, fast: bool) {
        self.flier_cooldown = self.flier_cooldown.saturating_add(1);
        for flier in &mut self.fliers {
            if flier.movement_timer == 0 || fast {
                flier.position.x += flier.direction;
                flier.fire_distance += 1;
                flier.movement_timer = match flier.kind {
                    FlierKind::Bomber => 2,
                    FlierKind::Satellite => 1,
                };
            } else {
                flier.movement_timer -= 1;
            }
        }
        let before = self.fliers.len();
        self.fliers
            .retain(|flier| (0.0..=255.0).contains(&flier.position.x));
        if before != self.fliers.len() {
            self.flier_cooldown = 0;
        }
    }
    fn update_explosions(&mut self) {
        // PREXPL visits four of the twenty hardware slots per video frame.
        // Newly chained explosions may occupy a later slot in this same batch.
        self.explosion_phase = if self.explosion_phase == 0 {
            4
        } else {
            self.explosion_phase - 1
        };
        let first = self.explosion_phase * 4;
        for slot in (first..first + 4).rev() {
            let Some(index) = self
                .explosions
                .iter()
                .position(|explosion| explosion.slot == slot)
            else {
                continue;
            };
            self.explosions[index].age += 5;
            if self.explosions[index].age >= 135 {
                self.explosions.remove(index);
                continue;
            }
            self.explosions[index].radius =
                RADII[(self.explosions[index].age / 5 + 1).min(27) as usize] as f32;
            let explosion = self.explosions[index].clone();
            self.damage_from_explosion(&explosion);
        }
    }
    fn damage_from_explosion(&mut self, explosion: &Explosion) {
        for missile in &mut self.missiles {
            missile
                .trail
                .retain(|&point| !(explosion.radius > 0.0 && explosion.contains(point, 0.0)));
        }
        // The original damages only above native altitude 33. Ground bursts
        // never clear cities themselves or reward an incoming ground impact.
        let mut kills = Vec::new();
        self.missiles.retain(|missile| {
            if matches!(missile.kind, MissileKind::Interceptor { .. }) {
                return true;
            }
            let padding = if missile.kind == MissileKind::SmartBomb {
                3.0
            } else {
                1.0
            };
            let hit =
                explosion.position.y <= 197.0 && explosion.contains(missile.position, padding);
            if hit {
                kills.push((missile.position, missile.kind));
            }
            !hit
        });
        let mut flier_kills = Vec::new();
        self.fliers.retain(|flier| {
            let hit = explosion.position.y <= 197.0 && explosion.contains(flier.position, 6.0);
            if hit {
                flier_kills.push(flier.position);
            }
            !hit
        });
        for (position, kind) in kills {
            if kind == MissileKind::SmartBomb || position.y <= 197.0 {
                self.add_score(if kind == MissileKind::SmartBomb {
                    125
                } else {
                    25
                });
            }
            self.explode(position, ExplosionKind::Hostile);
        }
        for position in flier_kills {
            self.add_score(100);
            self.explode(position, ExplosionKind::Hostile);
            self.flier_cooldown = 0;
        }
    }
    fn launch_offense(&mut self) {
        if self.remaining_enemies() == 0 {
            return;
        }
        let launch_altitude = (202_i64 - 2 * i64::from(self.wave)).max(180) as f32;
        if self.missiles.iter().any(|m| {
            !matches!(m.kind, MissileKind::Interceptor { .. })
                && m.position.y < 230.0 - launch_altitude
        }) {
            return;
        }
        let enemy_count = self
            .missiles
            .iter()
            .filter(|m| !matches!(m.kind, MissileKind::Interceptor { .. }))
            .count();
        let smart_count = self
            .missiles
            .iter()
            .filter(|m| m.kind == MissileKind::SmartBomb)
            .count();
        let hostile_explosions = self
            .explosions
            .iter()
            .filter(|e| e.kind == ExplosionKind::Hostile)
            .count();
        let mut capacity =
            MAX_ENEMIES
                .saturating_sub(enemy_count + hostile_explosions + self.fliers.len())
                .min(MAX_EXPLOSIONS.saturating_sub(
                    self.missiles.len() + self.explosions.len() + self.fliers.len(),
                ));
        if capacity == 0 {
            return;
        }
        let cfg = waves::config(self.wave);
        if self.wave >= 2
            && self.remaining_icbms > 0
            && self.fliers.is_empty()
            && self.flier_cooldown >= u32::from(cfg.flier_cooldown)
        {
            let direction = if self.rng.range(2) == 0 { 1.0 } else { -1.0 };
            let offset = if self.wave < 4 {
                48
            } else if self.wave < 6 {
                32
            } else {
                0
            };
            let altitude = 100 + offset + self.rng.range(32);
            let kind = if self.rng.range(2) == 0 {
                FlierKind::Satellite
            } else {
                FlierKind::Bomber
            };
            self.fliers.push(Flier {
                position: Vec2::new(
                    if direction > 0.0 { 0.0 } else { 255.0 },
                    230.0 - altitude as f32,
                ),
                kind,
                direction,
                movement_timer: 0,
                fire_distance: 0,
            });
            capacity -= 1;
            if capacity == 0 {
                return;
            }
        }
        if self.remaining_smart_bombs > 0
            && smart_count < 3
            && enemy_count < 5
            && (self.remaining_icbms == 0 || self.rng.range(4) == 0)
        {
            let origin = Vec2::new(self.rng.range(256) as f32, 8.0);
            self.spawn_enemy(origin, MissileKind::SmartBomb);
            self.remaining_smart_bombs -= 1;
            return;
        }
        if self.remaining_icbms == 0 {
            return;
        }
        let count = capacity
            .min(4)
            .min(usize::from(self.remaining_icbms))
            .min(MAX_ENEMIES.saturating_sub(enemy_count + smart_count + self.fliers.len()));
        if count == 0 {
            return;
        }
        let flier_origin = self.fliers.iter().position(|f| {
            f.fire_distance >= u32::from(cfg.flier_fire_distance)
                && (48.0..208.0).contains(&f.position.x)
        });
        let split_index = if self.explosions.len() < 12 {
            self.missiles
                .iter()
                .position(|m| m.kind == MissileKind::Icbm && (70.0..=102.0).contains(&m.position.y))
        } else {
            None
        };
        let (origin, count) = if let Some(index) = flier_origin {
            let origin = self.fliers[index].position;
            self.fliers[index].fire_distance = 0;
            (Some(origin), count.min(3))
        } else if let Some(index) = split_index {
            (Some(self.missiles[index].position), count.min(3))
        } else {
            (None, count)
        };
        for _ in 0..count {
            let origin = origin.unwrap_or_else(|| Vec2::new(self.rng.range(256) as f32, 8.0));
            self.spawn_enemy(origin, MissileKind::Icbm);
            self.remaining_icbms -= 1;
        }
    }
    fn spawn_enemy(&mut self, origin: Vec2, kind: MissileKind) {
        let target_index = self.choose_target();
        let target = if target_index < 6 {
            self.cities[target_index].position
        } else {
            self.bases[target_index - 6].position
        };
        self.missiles
            .push(Missile::new(origin, target, kind, Some(target_index)));
    }
    fn choose_target(&mut self) -> usize {
        let targeted = |index: usize| self.missiles.iter().any(|m| m.target_index == Some(index));
        let cities_targeted = self
            .cities
            .iter()
            .enumerate()
            .filter(|(i, c)| c.alive && targeted(*i))
            .count();
        let mut candidates = Vec::new();
        if cities_targeted + usize::from(self.cities_lost_this_wave) < 3 {
            for (i, city) in self.cities.iter().enumerate() {
                if city.alive && !targeted(i) {
                    candidates.push(i);
                }
            }
        }
        if candidates.is_empty() {
            for (i, base) in self.bases.iter().enumerate() {
                if base.alive && !targeted(i + 6) {
                    candidates.push(i + 6);
                }
            }
        }
        if candidates.is_empty() {
            for i in 0..9 {
                if i >= 6 || !self.cities[i].alive || targeted(i) {
                    candidates.push(i);
                }
            }
        }
        candidates[self.rng.range(candidates.len() as u32) as usize]
    }
    fn explode(&mut self, position: Vec2, kind: ExplosionKind) {
        if position.y < 21.0 {
            return;
        }
        if self.explosions.len() >= MAX_EXPLOSIONS {
            return;
        }
        let mut explosion = Explosion::new(position, kind);
        explosion.slot = self.next_explosion_slot;
        self.next_explosion_slot = if self.next_explosion_slot == 0 {
            19
        } else {
            self.next_explosion_slot - 1
        };
        self.explosions
            .retain(|existing| existing.slot != explosion.slot);
        self.explosions.push(explosion);
        self.events.push(GameEvent::Explosion);
    }
    fn add_score(&mut self, base_points: u64) {
        self.award_points(base_points * u64::from(self.multiplier()));
    }
    fn award_points(&mut self, points: u64) {
        self.score = self.score.saturating_add(points);
        while self.score >= self.next_bonus && self.next_bonus < u64::MAX {
            self.bonus_cities = self.bonus_cities.saturating_add(1);
            self.next_bonus = self.next_bonus.saturating_add(BONUS_INTERVAL);
            self.events.push(GameEvent::BonusCity);
        }
    }
    fn complete_wave(&mut self) {
        self.wave_bonus_ammo = self
            .bases
            .iter()
            .map(|b| u64::from(b.ammo) * 5)
            .sum::<u64>()
            * u64::from(self.multiplier());
        self.wave_bonus_cities = self.alive_cities() as u64 * 100 * u64::from(self.multiplier());
        self.award_points(self.wave_bonus_ammo + self.wave_bonus_cities);
        self.events.push(GameEvent::WaveCompleted);
        if self.alive_cities() == 0 && self.bonus_cities == 0 {
            self.finish_game();
        } else {
            self.phase = Phase::WaveBonus { ticks: 0 };
        }
    }
    fn restore_cities(&mut self) {
        let mut empty: Vec<_> = self
            .cities
            .iter()
            .enumerate()
            .filter_map(|(index, city)| (!city.alive).then_some(index))
            .collect();
        while self.bonus_cities > 0 && !empty.is_empty() {
            let index = self.rng.range(empty.len() as u32) as usize;
            self.cities[empty.swap_remove(index)].alive = true;
            self.bonus_cities -= 1;
        }
    }
    fn finish_game(&mut self) {
        self.phase = Phase::GameOver { ticks: 0 };
        self.events.push(GameEvent::GameOver);
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}
fn initial_cities() -> [City; 6] {
    // Keep source target order; the three cities on each side are interleaved.
    [
        (95.0, 214.0),
        (180.0, 209.0),
        (148.0, 212.0),
        (44.0, 212.0),
        (71.0, 213.0),
        (208.0, 213.0),
    ]
    .map(|(x, y)| City {
        position: Vec2::new(x, y),
        alive: true,
    })
}
fn initial_bases() -> [Base; 3] {
    [20.0, 123.0, 240.0].map(|x| Base {
        position: Vec2::new(x, 208.0),
        ammo: 10,
        alive: true,
    })
}

#[derive(Clone, Debug, PartialEq)]
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x4d495353494c45 } else { seed })
    }
    fn range(&mut self, limit: u32) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((u128::from(self.0) * u128::from(limit)) >> 64) as u32
    }
}

/// The original eight pixel probes, 16 opening masks and 8.8 movement table.
/// Explosions are queried geometrically instead of reading framebuffer colors.
fn evade(missile: &mut Missile, explosions: &[Explosion], wave: u32) -> bool {
    if !(8.0..247.0).contains(&missile.position.x) || !(24.0..185.0).contains(&missile.position.y) {
        return false;
    }
    const PROBES: [(i32, i32); 8] = [
        (0, -8),
        (-6, -6),
        (-8, 0),
        (-6, 6),
        (0, 8),
        (6, 6),
        (8, 0),
        (6, -6),
    ];
    let mut danger = 0u8;
    for (i, (x, y)) in PROBES.iter().enumerate() {
        let point = Vec2::new(
            missile.position.x + *x as f32,
            missile.position.y + *y as f32,
        );
        if explosions
            .iter()
            .any(|e| e.radius > 0.0 && e.contains(point, 0.0))
        {
            danger |= 1 << i;
        }
    }
    if danger == 0 || danger == 255 {
        return false;
    }
    missile.recalculate_velocity();
    if wave < 9 {
        danger |= 1;
    }
    let dx = missile.target.x - missile.position.x;
    let dy = missile.target.y - missile.position.y;
    let slope = if dx == 0.0 {
        f32::INFINITY
    } else {
        dy.abs() / dx.abs()
    };
    let thresholds = [
        0.0,
        50.0 / 256.0,
        171.0 / 256.0,
        383.0 / 256.0,
        1286.0 / 256.0,
    ];
    let mut adjustment = thresholds
        .iter()
        .rposition(|&value| slope >= value)
        .unwrap_or(0);
    // Convert top-down signs to the source's bottom-up quadrants.
    if (dx < 0.0) == (dy < 0.0) {
        adjustment = 4 - adjustment;
    }
    let quadrant = if dx >= 0.0 {
        if dy <= 0.0 { 12 } else { 8 }
    } else if dy <= 0.0 {
        0
    } else {
        4
    };
    let desired = (quadrant + adjustment) % 16;
    const MASKS: [u8; 16] = [
        0x83, 0x87, 7, 15, 14, 30, 28, 60, 56, 120, 112, 240, 224, 225, 193, 195,
    ];
    let mut direction = None;
    for delta in 0..=8 {
        for i in [(desired + 16 - delta) % 16, (desired + delta) % 16] {
            if danger & MASKS[i] == 0 {
                direction = Some(i);
                break;
            }
        }
        if direction.is_some() {
            break;
        }
    }
    if direction.is_none() {
        let desired = desired / 2;
        for delta in 0..=4 {
            for i in [(desired + 8 - delta) % 8, (desired + delta) % 8] {
                if danger & (1 << i) == 0 {
                    direction = Some(i * 2);
                    break;
                }
            }
            if direction.is_some() {
                break;
            }
        }
    }
    let Some(direction) = direction else {
        return false;
    };
    const INCREMENTS: [(i32, i32); 16] = [
        (0, -256),
        (-98, -236),
        (-180, -180),
        (-236, -98),
        (-256, 0),
        (-236, 98),
        (-180, 180),
        (-98, 236),
        (0, 256),
        (98, 236),
        (180, 180),
        (236, 98),
        (256, 0),
        (236, -98),
        (180, -180),
        (98, -236),
    ];
    let (x, y) = INCREMENTS[direction];
    missile.fixed[0] += x;
    missile.fixed[1] += y;
    missile.position = Vec2::new(
        missile.fixed[0] as f32 / 256.0,
        missile.fixed[1] as f32 / 256.0,
    );
    missile.recalculate_velocity();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Game {
        let mut game = Game::new(1980);
        game.start();
        game.wave_ticks = 122;
        game.remaining_icbms = 1;
        game.remaining_smart_bombs = 0;
        game
    }

    #[test]
    fn arcade_start_has_six_cities_and_ten_missiles_at_each_base() {
        let mut game = Game::new(1);
        assert_eq!(game.phase, Phase::Attract);
        game.start();
        assert_eq!(game.alive_cities(), 6);
        assert_eq!(game.bases.map(|b| b.ammo), [10, 10, 10]);
        assert_eq!(game.remaining_enemies(), 12);
    }

    #[test]
    fn central_battery_travels_seven_units_side_batteries_three() {
        let mut game = ready();
        game.aim = Vec2::new(20.0, 100.0);
        game.fire(0);
        game.aim = Vec2::new(123.0, 100.0);
        game.fire(1);
        game.move_interceptors();
        assert_eq!(game.missiles[0].position, Vec2::new(20.0, 203.0));
        assert_eq!(game.missiles[1].position, Vec2::new(123.0, 199.0));
        assert_eq!(game.bases[0].ammo, 9);
        assert_eq!(game.bases[1].ammo, 9);
    }

    #[test]
    fn firing_capacity_preserves_ammo_when_all_eight_slots_are_busy() {
        let mut game = ready();
        for _ in 0..8 {
            game.fire(0);
        }
        game.fire(1);
        assert_eq!(game.missiles.len(), 8);
        assert_eq!(game.bases[1].ammo, 10);
        assert!(matches!(
            game.events.last(),
            Some(GameEvent::Empty { base: 1 })
        ));
    }

    #[test]
    fn firing_at_launcher_position_does_not_leave_stuck_interceptor() {
        let mut game = ready();
        game.aim = Vec2::new(123.0, 206.0);
        game.fire(1);
        game.move_interceptors();
        assert!(game.missiles.is_empty());
        assert_eq!(game.explosions.len(), 1);
    }

    #[test]
    fn missile_kill_starts_chain_reaction_and_awards_scaled_score() {
        let mut game = ready();
        game.wave = 5;
        game.explosions.push(Explosion {
            position: Vec2::new(100.0, 100.0),
            radius: 10.0,
            age: 49,
            kind: ExplosionKind::Defensive,
            slot: 19,
        });
        game.spawn_enemy(Vec2::new(105.0, 100.0), MissileKind::Icbm);
        game.update_explosions();
        assert!(game.missiles.is_empty());
        assert_eq!(game.score, 75);
        assert!(
            game.explosions
                .iter()
                .any(|e| e.kind == ExplosionKind::Hostile)
        );
        game.spawn_enemy(Vec2::new(105.0, 100.0), MissileKind::SmartBomb);
        for _ in 0..5 {
            game.update_explosions();
        }
        assert_eq!(game.score, 450);
    }

    #[test]
    fn low_ground_bursts_cannot_earn_points_or_chain_kills() {
        let mut game = ready();
        game.explosions.push(Explosion {
            position: Vec2::new(100.0, 208.0),
            radius: 13.0,
            age: 64,
            kind: ExplosionKind::Hostile,
            slot: 19,
        });
        game.spawn_enemy(Vec2::new(100.0, 200.0), MissileKind::Icbm);
        game.update_explosions();
        assert_eq!(game.score, 0);
        assert_eq!(game.missiles.len(), 1);
    }

    #[test]
    fn blast_lifetime_has_original_thirteen_pixel_peak_and_shrink() {
        let mut game = ready();
        game.explode(Vec2::new(80.0, 80.0), ExplosionKind::Defensive);
        for _ in 0..65 {
            game.update_explosions();
        }
        assert_eq!(game.explosions[0].radius, 13.0);
        for _ in 0..60 {
            game.update_explosions();
        }
        assert_eq!(game.explosions[0].radius, 1.0);
        for _ in 0..10 {
            game.update_explosions();
        }
        assert!(game.explosions.is_empty());
    }

    #[test]
    fn target_allocator_limits_simultaneous_live_city_targets_to_three() {
        let mut game = ready();
        for _ in 0..8 {
            game.spawn_enemy(Vec2::new(100.0, 8.0), MissileKind::Icbm);
        }
        let city_targets = game
            .missiles
            .iter()
            .filter(|m| m.target_index.unwrap() < 6)
            .count();
        assert_eq!(city_targets, 3);
    }

    #[test]
    fn banked_cities_restore_only_between_waves_and_survive_full_city_board() {
        let mut game = ready();
        game.award_points(20_050);
        assert_eq!(game.bonus_cities, 2);
        game.restore_cities();
        assert_eq!(game.bonus_cities, 2);
        game.cities[2].alive = false;
        game.cities[4].alive = false;
        assert_eq!(game.alive_cities(), 4);
        game.restore_cities();
        assert_eq!(game.alive_cities(), 6);
        assert_eq!(game.bonus_cities, 0);
        game.award_points(10_000);
        assert_eq!(game.bonus_cities, 1);
    }

    #[test]
    fn wave_bonus_rewards_unused_missiles_and_living_cities_then_rearms() {
        let mut game = ready();
        game.wave = 3;
        game.bases[0].ammo = 2;
        game.bases[2].alive = false;
        game.bases[2].ammo = 0;
        game.cities[1].alive = false;
        game.complete_wave();
        assert_eq!(game.wave_bonus_ammo, 120);
        assert_eq!(game.wave_bonus_cities, 1000);
        assert_eq!(game.score, 1120);
        for _ in 0..BONUS_DURATION {
            game.update(Input::default());
        }
        assert_eq!(game.wave, 4);
        assert_eq!(game.phase, Phase::Playing);
        assert_eq!(game.bases.map(|b| b.ammo), [10, 10, 10]);
        assert_eq!(game.alive_cities(), 5);
    }

    #[test]
    fn banked_city_can_continue_game_after_last_city_is_lost() {
        let mut game = ready();
        for city in &mut game.cities {
            city.alive = false;
        }
        game.bonus_cities = 1;
        game.complete_wave();
        assert!(matches!(game.phase, Phase::WaveBonus { .. }));
        for _ in 0..BONUS_DURATION {
            game.update(Input::default());
        }
        assert_eq!(game.alive_cities(), 1);
        assert_eq!(game.phase, Phase::Playing);
    }

    #[test]
    fn smart_bomb_probes_avoid_explosion_instead_of_descending_into_it() {
        let mut missile = Missile::new(
            Vec2::new(100.0, 100.0),
            Vec2::new(100.0, 212.0),
            MissileKind::SmartBomb,
            Some(0),
        );
        let explosion = Explosion {
            position: Vec2::new(100.0, 115.0),
            radius: 13.0,
            age: 65,
            kind: ExplosionKind::Defensive,
            slot: 0,
        };
        assert!(evade(&mut missile, &[explosion], 6));
        assert!(missile.position.x != 100.0 || missile.position.y <= 100.0);
    }

    #[test]
    fn bomber_and_satellite_use_three_and_two_frame_movement_periods() {
        let mut game = ready();
        for kind in [FlierKind::Bomber, FlierKind::Satellite] {
            game.fliers.push(Flier {
                position: Vec2::new(0.0, 80.0),
                kind,
                direction: 1.0,
                movement_timer: 0,
                fire_distance: 0,
            });
        }
        for _ in 0..6 {
            game.move_fliers(false);
        }
        assert_eq!(game.fliers[0].position.x, 2.0);
        assert_eq!(game.fliers[1].position.x, 3.0);
    }

    #[test]
    fn explosion_damage_waits_for_its_four_slot_batch() {
        let mut game = ready();
        game.explode(Vec2::new(100.0, 100.0), ExplosionKind::Defensive);
        game.spawn_enemy(Vec2::new(100.0, 100.0), MissileKind::Icbm);
        for _ in 0..4 {
            game.update_explosions();
            assert_eq!(game.missiles.len(), 1);
            assert_eq!(game.explosions[0].radius, 0.0);
        }
        game.update_explosions();
        assert!(game.missiles.is_empty());
        assert_eq!(game.explosions[0].radius, 2.0);
        assert_eq!(game.score, 25);
    }

    #[test]
    fn no_ammunition_cleanup_remains_fast_after_a_ground_impact() {
        let mut game = ready();
        for base in &mut game.bases {
            base.ammo = 0;
        }
        game.spawn_enemy(Vec2::new(100.0, 100.0), MissileKind::Icbm);
        game.play_tick(Input::default());
        assert!(game.cleanup);
        game.explode(Vec2::new(20.0, 208.0), ExplosionKind::Hostile);
        let before = game.missiles[0].position;
        game.play_tick(Input::default());
        let middle = game.missiles[0].position;
        game.play_tick(Input::default());
        assert!(game.cleanup);
        assert_ne!(before, middle);
        assert_ne!(middle, game.missiles[0].position);
    }

    #[test]
    fn incoming_split_heads_consume_the_wave_budget() {
        let mut game = ready();
        game.remaining_icbms = 3;
        game.spawn_enemy(Vec2::new(100.0, 80.0), MissileKind::Icbm);
        game.launch_offense();
        assert_eq!(game.remaining_icbms, 0);
        assert_eq!(game.missiles.len(), 4);
        assert!(
            game.missiles
                .iter()
                .all(|missile| missile.origin == Vec2::new(100.0, 80.0))
        );
    }

    #[test]
    fn no_extra_interceptor_is_spent_after_the_last_hostile_is_gone() {
        let mut game = ready();
        game.remaining_icbms = 0;
        game.explode(Vec2::new(100.0, 100.0), ExplosionKind::Hostile);
        game.fire(1);
        assert_eq!(game.bases[1].ammo, 10);
        assert!(game.missiles.is_empty());
    }

    #[test]
    fn first_wave_fractional_delay_moves_sixteen_units_in_ninety_three_frames() {
        let mut game = ready();
        game.remaining_icbms = 0;
        game.missiles.push(Missile::new(
            Vec2::new(20.0, 100.0),
            Vec2::new(220.0, 100.0),
            MissileKind::Icbm,
            None,
        ));
        for _ in 0..93 {
            game.play_tick(Input::default());
        }
        assert_eq!(game.missiles[0].position, Vec2::new(36.0, 100.0));
    }

    #[test]
    fn ninth_wave_enables_upward_smart_bomb_escape() {
        let bomb = Missile::new(
            Vec2::new(100.0, 100.0),
            Vec2::new(100.0, 212.0),
            MissileKind::SmartBomb,
            Some(0),
        );
        let explosion = Explosion {
            position: Vec2::new(100.0, 115.0),
            radius: 13.0,
            age: 65,
            kind: ExplosionKind::Defensive,
            slot: 0,
        };
        let mut early = bomb.clone();
        let mut late = bomb;
        assert!(evade(&mut early, std::slice::from_ref(&explosion), 6));
        assert!(evade(&mut late, &[explosion], 9));
        assert!(early.position.y >= 100.0);
        assert!(late.position.y < 100.0);
    }

    #[test]
    fn every_wave_configuration_finishes_with_aircraft_and_smart_bombs_exercised() {
        let mut observed_smart = false;
        let mut observed_flier = false;
        for wave in 1..=40 {
            let mut game = Game::new(1980 + u64::from(wave));
            game.start();
            game.wave = wave;
            game.prepare_wave();
            for _ in 0..20_000 {
                // Preserve targets/ammo to exercise the entire offense budget;
                // the separate unattended test covers real loss and game over.
                for city in &mut game.cities {
                    city.alive = true;
                }
                game.bases = initial_bases();
                game.cities_lost_this_wave = 0;
                game.update(Input::default());
                observed_smart |= game
                    .missiles
                    .iter()
                    .any(|missile| missile.kind == MissileKind::SmartBomb);
                observed_flier |= !game.fliers.is_empty();
                if matches!(game.phase, Phase::WaveBonus { .. }) {
                    break;
                }
            }
            assert!(
                matches!(game.phase, Phase::WaveBonus { .. }),
                "wave {wave} did not finish"
            );
            assert_eq!(game.remaining_enemies(), 0);
        }
        assert!(observed_smart && observed_flier);
    }
}
