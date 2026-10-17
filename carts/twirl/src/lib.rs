//! Twirl: steer a constantly spinning stick through a maze without touching
//! the walls. Inspired by the spinning-stick puzzlers of the handheld era.
//!
//! Hardware used: an affine sprite for the stick, BG0 for the maze (streamed
//! as the camera scrolls), BG2 parallax clouds alpha-blended over a backdrop
//! whose color changes every scanline, sprites for the HUD, save RAM for the
//! best time.

use std::cell::UnsafeCell;
use std::f32::consts::{PI, TAU};
use wasm_ad::*;

wasm_ad::cart! {
    title: "Twirl",
    author: "raskell.io",
    version: "0.1.0",
    description: "Steer a spinning stick through the maze without touching the walls"
}

/// `#` wall, `.` open, `o` spring (reverses the spin), `G` goal, `S` start.
const LEVEL: &str = "\
################################################
#........#######################################
#.................##...................#########
#.................##...................#########
#...S..................................o########
#.........................##...........#########
#.........................##...........#########
#........#########################.....#########
##################################.....#########
##################################.....#########
##################################.....#########
##################################.....#########
##########............##...............#########
##########............##...............#########
#########o.............................#########
##########..................##.........#########
##########..................##.........#########
##########.....#####################o###########
##########.....#########################.......#
##########.....#########################.......#
##########....................##...............#
##########....................##..........GGGG.#
##########................................GGGG.#
##########..........##....................GGGG.#
##########..........##....................GGGG.#
########################o###############..GGGG.#
########################################.......#
########################################.......#
################################################
################################################";

const CELL: f32 = 16.0;
/// Distance from the stick's centre to each tip, in pixels.
const HALF: f32 = 26.0;
const SPIN: f32 = TAU / 200.0;
const HEARTS: u8 = 3;

const MAZE_BLOCK: usize = 28;
const CLOUD_BLOCK: usize = 26;

// Background tiles (character block 0).
const T_BRICK: u16 = 1; // 4 tiles: a 16×16 brick block
const T_EDGE: u16 = 5; // 2 tiles: the top half of a brick block with a lit edge
const T_GOAL: u16 = 7; // 4 tiles
const T_SPRING: u16 = 11; // 4 tiles
const T_CLOUD: u16 = 16; // 8 tiles: a 32×16 cloud

// Sprite tiles (1D mapping).
const S_STICK: u16 = 0; // 64 tiles: 64×64
const S_FONT: u16 = 64;
const S_HEART: u16 = 120; // full, empty
const FONT_CHARS: &str = "0123456789:.!- ABCDEGIKLMNOPRSTUVWY";

const SAVE_MAGIC: &[u8; 4] = b"TWRL";

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Play,
    Clear(u32),
    Over(u32),
}

struct Game {
    buttons: Buttons,
    level: Vec<u8>,
    w: i32,
    h: i32,
    start: (f32, f32),
    x: f32,
    y: f32,
    angle: f32,
    spin: f32,
    vx: f32,
    vy: f32,
    hearts: u8,
    invulnerable: u32,
    shake: u32,
    spring_cooldown: u32,
    phase: Phase,
    started: bool,
    time: u32,
    best: Option<u32>,
    new_best: bool,
    frame: u32,
}

struct Global(UnsafeCell<Option<Game>>);
// SAFETY: wasm32-unknown-unknown is single-threaded.
unsafe impl Sync for Global {}
static GAME: Global = Global(UnsafeCell::new(None));

fn game() -> Option<&'static mut Game> {
    unsafe { (*GAME.0.get()).as_mut() }
}

impl Game {
    fn new() -> Self {
        let rows: Vec<&str> = LEVEL.lines().collect();
        let (w, h) = (rows[0].len() as i32, rows.len() as i32);
        let level: Vec<u8> = rows.concat().into_bytes();
        let s = level.iter().position(|&c| c == b'S').expect("level has a start") as i32;
        let start = (((s % w) as f32 + 0.5) * CELL, ((s / w) as f32 + 0.5) * CELL);

        let sram = save_ram();
        let best = (save_restored() && &sram[..4] == SAVE_MAGIC)
            .then(|| u32::from_le_bytes(sram[4..8].try_into().unwrap()))
            .filter(|&t| t > 0);

        let mut g = Self {
            buttons: Buttons::new(),
            level,
            w,
            h,
            start,
            x: 0.0,
            y: 0.0,
            angle: 0.0,
            spin: SPIN,
            vx: 0.0,
            vy: 0.0,
            hearts: HEARTS,
            invulnerable: 0,
            shake: 0,
            spring_cooldown: 0,
            phase: Phase::Play,
            started: false,
            time: 0,
            best,
            new_best: false,
            frame: 0,
        };
        g.reset();
        g
    }

    fn reset(&mut self) {
        (self.x, self.y) = self.start;
        self.angle = 0.0;
        self.spin = SPIN;
        (self.vx, self.vy) = (0.0, 0.0);
        self.hearts = HEARTS;
        self.invulnerable = 0;
        self.shake = 0;
        self.phase = Phase::Play;
        self.started = false;
        self.time = 0;
        self.new_best = false;
    }

    fn cell(&self, cx: i32, cy: i32) -> u8 {
        if cx < 0 || cy < 0 || cx >= self.w || cy >= self.h {
            return b'#';
        }
        self.level[(cy * self.w + cx) as usize]
    }

    fn cell_at(&self, px: f32, py: f32) -> u8 {
        self.cell((px / CELL).floor() as i32, (py / CELL).floor() as i32)
    }

    /// The stick's on-screen direction for the current angle. The affine
    /// matrix maps screen to texture space, so a counter-clockwise visual
    /// rotation by `angle` points the texture's x axis along (cos, -sin).
    fn direction(&self) -> (f32, f32) {
        (self.angle.cos(), -self.angle.sin())
    }

    /// The first solid point under the stick, preferring springs. Samples
    /// cover the stick's length and thickness and the round cockpit.
    fn collision(&self) -> Option<(f32, f32, u8)> {
        let (dx, dy) = self.direction();
        let (nx, ny) = (-dy, dx);
        let stick = (0..=26).flat_map(|i| {
            let t = -HALF + i as f32 * 2.0;
            [-3.0, 0.0, 3.0].map(|o| (dx * t + nx * o, dy * t + ny * o))
        });
        let cockpit = (0..12).map(|i| {
            let a = i as f32 * TAU / 12.0;
            (a.cos() * 8.5, a.sin() * 8.5)
        });
        let mut wall = None;
        for (ox, oy) in stick.chain(cockpit) {
            let (px, py) = (self.x + ox, self.y + oy);
            match self.cell_at(px, py) {
                b'o' => return Some((px, py, b'o')),
                b'#' if wall.is_none() => wall = Some((px, py, b'#')),
                _ => {}
            }
        }
        wall
    }

    fn update(&mut self) {
        self.buttons.update();
        self.frame = self.frame.wrapping_add(1);
        self.shake = self.shake.saturating_sub(1);
        self.invulnerable = self.invulnerable.saturating_sub(1);
        self.spring_cooldown = self.spring_cooldown.saturating_sub(1);

        match self.phase {
            Phase::Play => self.play(),
            Phase::Clear(t) => {
                self.angle += self.spin * 2.0;
                self.phase = Phase::Clear(t + 1);
                if t > 240 || (t > 45 && self.buttons.pressed(key::A | key::START)) {
                    self.reset();
                }
            }
            Phase::Over(t) => {
                self.angle += self.spin * 0.5;
                self.phase = Phase::Over(t + 1);
                if t > 120 || (t > 30 && self.buttons.pressed(key::A | key::START)) {
                    self.reset();
                }
            }
        }
        self.draw();
    }

    fn play(&mut self) {
        self.angle = (self.angle + self.spin).rem_euclid(TAU);
        let b = &self.buttons;
        let mut ix = 0.0;
        let mut iy = 0.0;
        if b.held(key::LEFT) { ix -= 1.0 }
        if b.held(key::RIGHT) { ix += 1.0 }
        if b.held(key::UP) { iy -= 1.0 }
        if b.held(key::DOWN) { iy += 1.0 }
        if ix != 0.0 && iy != 0.0 {
            ix *= std::f32::consts::FRAC_1_SQRT_2;
            iy *= std::f32::consts::FRAC_1_SQRT_2;
        }
        let speed = if b.held(key::A) || b.held(key::B) { 2.0 } else { 1.0 };
        if ix != 0.0 || iy != 0.0 {
            self.started = true;
        }
        if self.started {
            self.time += 1;
        }

        let (px, py) = (self.x, self.y);
        self.x += ix * speed + self.vx;
        self.y += iy * speed + self.vy;
        self.vx *= 0.86;
        self.vy *= 0.86;

        if let Some((hx, hy, kind)) = self.collision() {
            // Bounce away from the contact point.
            let (mut ax, mut ay) = (px - hx, py - hy);
            let len = (ax * ax + ay * ay).sqrt();
            if len < 0.5 {
                (ax, ay) = (-ix, -iy);
            } else {
                (ax, ay) = (ax / len, ay / len);
            }
            (self.x, self.y) = (px, py);
            match kind {
                b'o' if self.spring_cooldown == 0 => {
                    self.spin = -self.spin;
                    self.spring_cooldown = 30;
                    (self.vx, self.vy) = (ax * 3.0, ay * 3.0);
                }
                b'#' if self.invulnerable == 0 => {
                    self.hearts -= 1;
                    self.invulnerable = 90;
                    self.shake = 14;
                    (self.vx, self.vy) = (ax * 3.5, ay * 3.5);
                    if self.hearts == 0 {
                        self.phase = Phase::Over(0);
                    }
                }
                _ => (self.vx, self.vy) = (ax * 1.5, ay * 1.5),
            }
            // The stick keeps spinning; step it back so it cannot rotate into the wall.
            self.angle -= self.spin;
        }

        if self.cell_at(self.x, self.y) == b'G' && self.phase == Phase::Play {
            self.new_best = self.best.is_none_or(|b| self.time < b);
            if self.new_best {
                self.best = Some(self.time);
                let sram = save_ram();
                sram[..4].copy_from_slice(SAVE_MAGIC);
                sram[4..8].copy_from_slice(&self.time.to_le_bytes());
                request_save();
            }
            self.phase = Phase::Clear(0);
        }
    }

    fn camera(&self) -> (i32, i32) {
        let lw = self.w * CELL as i32;
        let lh = self.h * CELL as i32;
        let mut cx = (self.x as i32 - 120).clamp(0, lw - 240);
        let mut cy = (self.y as i32 - 80).clamp(0, lh - 160);
        if self.shake > 0 {
            cx += (self.frame as i32 * 7 % 5) - 2;
            cy += (self.frame as i32 * 3 % 5) - 2;
        }
        (cx.max(0), cy.max(0))
    }

    fn draw(&self) {
        let (cx, cy) = self.camera();
        reg::BGHOFS[0].write(cx as u16);
        reg::BGVOFS[0].write(cy as u16);
        reg::BGHOFS[2].write((cx / 3) as u16);
        reg::BGVOFS[2].write((cy / 4) as u16);

        // Stream the visible part of the maze into the wrapping 32×32 map.
        let map = screen_block(MAZE_BLOCK);
        for ty in cy / 8..=(cy + 159) / 8 {
            for tx in cx / 8..=(cx + 239) / 8 {
                map[(ty as usize % 32) * 32 + tx as usize % 32] = self.tile(tx, ty);
            }
        }

        oam::hide_all();
        let mut slot = 0;
        let blink = self.invulnerable > 0 && self.frame / 3 % 2 == 0;
        if !blink {
            oam::set_affine(0, AffineMatrix::rotate_scale(self.angle, 1.0, 1.0));
            let (sx, sy) = (self.x as i32 - cx - 32, self.y as i32 - cy - 32);
            oam::set(slot, Sprite::new(sx, sy, SpriteSize::S64x64, S_STICK).affine(0, false).priority(1));
        }
        slot += 1;

        for i in 0..HEARTS {
            let full = i < self.hearts;
            let s = Sprite::new(8 + i as i32 * 11, 8, SpriteSize::S8x8, S_HEART + !full as u16).bank(2);
            oam::set(slot, s);
            slot += 1;
        }
        let clock = format_time(self.time);
        slot = text(slot, 232 - clock.len() as i32 * 7, 8, &clock);

        match self.phase {
            Phase::Play if !self.started && self.frame / 30 % 2 == 0 => {
                text(slot, centered("D-PAD MOVE  A BOOST"), 140, "D-PAD MOVE  A BOOST");
            }
            Phase::Clear(_) => {
                slot = text(slot, centered("GOAL!"), 62, "GOAL!");
                let line = format!("TIME {}", format_time(self.time));
                slot = text(slot, centered(&line), 78, &line);
                let best = if self.new_best { "NEW BEST!".to_string() } else { format!("BEST {}", format_time(self.best.unwrap_or(0))) };
                text(slot, centered(&best), 94, &best);
            }
            Phase::Over(_) => {
                text(slot, centered("TRY AGAIN"), 74, "TRY AGAIN");
            }
            _ => {}
        }
    }

    /// The map entry for maze tile (tx, ty), from its 16×16 cell.
    fn tile(&self, tx: i32, ty: i32) -> ScreenEntry {
        let (cx, cy) = (tx / 2, ty / 2);
        let q = ((ty % 2) * 2 + tx % 2) as u16;
        match self.cell(cx, cy) {
            b'#' => {
                if ty % 2 == 0 && self.cell(cx, cy - 1) != b'#' {
                    ScreenEntry::new(T_EDGE + q, 0)
                } else {
                    ScreenEntry::new(T_BRICK + q, 0)
                }
            }
            b'G' => ScreenEntry::new(T_GOAL + q, 1),
            b'o' => ScreenEntry::new(T_SPRING + q, 2),
            _ => ScreenEntry::EMPTY,
        }
    }
}

fn format_time(frames: u32) -> String {
    let cs = frames * 100 / 60;
    format!("{}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
}

fn centered(s: &str) -> i32 {
    120 - (s.len() as i32 * 7) / 2
}

/// Draws `s` with font sprites starting at OAM `slot`; returns the next free slot.
fn text(mut slot: usize, x: i32, y: i32, s: &str) -> usize {
    for (i, c) in s.chars().enumerate() {
        if c == ' ' || slot >= 128 {
            continue;
        }
        if let Some(g) = FONT_CHARS.find(c) {
            oam::set(slot, Sprite::new(x + i as i32 * 7, y, SpriteSize::S8x8, S_FONT + g as u16).bank(1));
            slot += 1;
        }
    }
    slot
}

// ---- Art -------------------------------------------------------------------

/// Packs a pixel grid (color indices) into 4bpp tiles, row-major, as
/// consecutive 1D-mapped tiles.
fn pack(px: &[Vec<u8>]) -> Vec<Tile4> {
    let (h, w) = (px.len(), px[0].len());
    let mut out = Vec::new();
    for ty in (0..h).step_by(8) {
        for tx in (0..w).step_by(8) {
            let mut t = [0u8; 32];
            for y in 0..8 {
                for x in 0..8 {
                    t[y * 4 + x / 2] |= px[ty + y][tx + x] << (4 * (x % 2));
                }
            }
            out.push(t);
        }
    }
    out
}

fn grid(w: usize, h: usize, f: impl Fn(usize, usize) -> u8) -> Vec<Vec<u8>> {
    (0..h).map(|y| (0..w).map(|x| f(x, y)).collect()).collect()
}

fn brick(x: usize, y: usize) -> u8 {
    let row = y % 8;
    let offset = if y < 8 { 0 } else { 8 };
    if row == 0 || (x + offset) % 16 == 0 {
        1 // mortar
    } else if row == 1 {
        3 // lit top of each brick
    } else if row == 7 {
        4 // shadow
    } else {
        2
    }
}

fn edge(x: usize, y: usize) -> u8 {
    match y {
        0 | 1 => 5,
        2 => 6,
        3 => 1,
        _ => brick(x, y),
    }
}

fn goal(x: usize, y: usize) -> u8 {
    if (x / 4 + y / 4) % 2 == 0 { 1 } else { 2 }
}

fn spring(x: usize, y: usize) -> u8 {
    let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
    let r = (dx * dx + dy * dy).sqrt();
    match r {
        r if r > 7.6 => 4,
        r if r > 6.4 => 3,
        r if r > 4.6 => 1,
        r if r > 3.0 => 2,
        _ => 1,
    }
}

fn cloud(x: usize, y: usize) -> u8 {
    let blobs = [(8.0, 10.0, 6.0), (15.0, 7.0, 7.0), (23.0, 9.0, 6.5), (28.0, 12.0, 3.5)];
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let inside = |dy: f32| blobs.iter().any(|&(cx, cy, r)| (fx - cx).powi(2) + (fy + dy - cy).powi(2) < r * r) && fy + dy < 14.5;
    if !inside(0.0) {
        0
    } else if !inside(-2.0) {
        2 // underside
    } else {
        1
    }
}

fn stick(x: usize, y: usize) -> u8 {
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let (dx, dy) = (fx - 32.0, fy - 32.0);
    let r = (dx * dx + dy * dy).sqrt();
    // Cockpit with the pilot.
    if r <= 9.0 {
        if r > 7.6 {
            return 3;
        }
        if (fx - 34.5).abs() < 1.6 && (fy - 30.0).abs() < 1.6 {
            return 3; // eye
        }
        if fx > 37.0 && (fy - 33.0).abs() < 1.5 {
            return 6; // beak
        }
        return 1;
    }
    // The stick: 52 px long, 8 px thick, outlined, red tips.
    let (x0, x1, y0, y1) = (6.0, 58.0, 28.0, 36.0);
    if fx < x0 || fx > x1 || fy < y0 || fy > y1 {
        return 0;
    }
    if fx < x0 + 1.0 || fx > x1 - 1.0 || fy < y0 + 1.0 || fy > y1 - 1.0 {
        return 3;
    }
    let tip = fx < 15.0 || fx > 49.0;
    let lit = fy < y0 + 3.0;
    match (tip, lit) {
        (true, true) => 7,
        (true, false) => 4,
        (false, true) => 5,
        (false, false) => 2,
    }
}

/// 5×7 glyphs for the HUD, in `FONT_CHARS` order.
const GLYPHS: [&str; 35] = [
    ".###.|#..##|#.#.#|##..#|#...#|#...#|.###.", // 0
    "..#..|.##..|..#..|..#..|..#..|..#..|.###.",
    ".###.|#...#|....#|...#.|..#..|.#...|#####",
    "####.|....#|....#|.###.|....#|....#|####.",
    "...#.|..##.|.#.#.|#..#.|#####|...#.|...#.",
    "#####|#....|####.|....#|....#|#...#|.###.",
    ".###.|#....|#....|####.|#...#|#...#|.###.",
    "#####|....#|...#.|..#..|.#...|.#...|.#...",
    ".###.|#...#|#...#|.###.|#...#|#...#|.###.",
    ".###.|#...#|#...#|.####|....#|....#|.###.", // 9
    ".....|..#..|.....|.....|.....|..#..|.....", // :
    ".....|.....|.....|.....|.....|.....|..#..", // .
    "..#..|..#..|..#..|..#..|..#..|.....|..#..", // !
    ".....|.....|.....|.###.|.....|.....|.....", // -
    ".....|.....|.....|.....|.....|.....|.....", // space
    ".###.|#...#|#...#|#####|#...#|#...#|#...#", // A
    "####.|#...#|#...#|####.|#...#|#...#|####.", // B
    ".###.|#...#|#....|#....|#....|#...#|.###.", // C
    "####.|#...#|#...#|#...#|#...#|#...#|####.", // D
    "#####|#....|#....|####.|#....|#....|#####", // E
    ".###.|#...#|#....|#.###|#...#|#...#|.####", // G
    ".###.|..#..|..#..|..#..|..#..|..#..|.###.", // I
    "#...#|#..#.|#.#..|##...|#.#..|#..#.|#...#", // K
    "#....|#....|#....|#....|#....|#....|#####", // L
    "#...#|##.##|#.#.#|#.#.#|#...#|#...#|#...#", // M
    "#...#|#...#|##..#|#.#.#|#..##|#...#|#...#", // N
    ".###.|#...#|#...#|#...#|#...#|#...#|.###.", // O
    "####.|#...#|#...#|####.|#....|#....|#....", // P
    "####.|#...#|#...#|####.|#.#..|#..#.|#...#", // R
    ".###.|#...#|#....|.###.|....#|#...#|.###.", // S
    "#####|..#..|..#..|..#..|..#..|..#..|..#..", // T
    "#...#|#...#|#...#|#...#|#...#|#...#|.###.", // U
    "#...#|#...#|#...#|#...#|#...#|.#.#.|..#..", // V
    "#...#|#...#|#...#|#.#.#|#.#.#|#.#.#|.#.#.", // W
    "#...#|#...#|.#.#.|..#..|..#..|..#..|..#..", // Y
];

/// A glyph with ink in color 1 and a drop shadow in color 2.
fn glyph(rows: &str) -> Tile4 {
    let ink: Vec<Vec<bool>> = rows.split('|').map(|r| r.bytes().map(|c| c == b'#').collect()).collect();
    let on = |x: i32, y: i32| (0..5).contains(&x) && (0..7).contains(&y) && ink[y as usize][x as usize];
    pack(&grid(8, 8, |x, y| {
        let (gx, gy) = (x as i32 - 1, y as i32);
        if on(gx, gy) { 1 } else if on(gx - 1, gy - 1) { 2 } else { 0 }
    }))[0]
}

const HEART: [&str; 8] = [
    ".##.##..",
    "#33#11#.",
    "#3111111",
    "#1111111",
    ".#11111.",
    "..#111..",
    "...#1...",
    "........",
];

fn heart(empty: bool) -> Tile4 {
    pack(&grid(8, 8, |x, y| match HEART[y].as_bytes()[x] {
        b'#' => 2,
        b'1' | b'3' if empty => 4,
        b'1' => 1,
        b'3' => 3,
        _ => 0,
    }))[0]
}

fn load_graphics() {
    let bg = bg_palette();
    let banks: [&[u16]; 4] = [
        // 0 walls: mortar, base, light, shadow, edge, edge shade
        &[rgb(24, 38, 74), rgb(61, 123, 224), rgb(121, 180, 255), rgb(36, 80, 156), rgb(255, 224, 102), rgb(249, 160, 63)],
        // 1 goal
        &[rgb(255, 255, 255), rgb(34, 30, 52)],
        // 2 spring: red, white, dark, (outside the ring: transparent wall colour)
        &[rgb(255, 77, 109), rgb(255, 255, 255), rgb(59, 13, 28), rgb(36, 80, 156)],
        // 3 clouds
        &[rgb(255, 255, 255), rgb(214, 232, 255)],
    ];
    for (b, colors) in banks.iter().enumerate() {
        bg[b * 16 + 1..][..colors.len()].copy_from_slice(colors);
    }
    let obj = obj_palette();
    let banks: [&[u16]; 3] = [
        // 0 stick: white, yellow, outline, red, light yellow, orange, pink
        &[rgb(255, 255, 255), rgb(255, 210, 63), rgb(43, 29, 58), rgb(239, 71, 111), rgb(255, 243, 160), rgb(255, 159, 28), rgb(255, 143, 171)],
        // 1 font
        &[rgb(255, 255, 255), rgb(27, 21, 48)],
        // 2 hearts
        &[rgb(255, 77, 109), rgb(59, 13, 28), rgb(255, 179, 198), rgb(107, 111, 138)],
    ];
    for (b, colors) in banks.iter().enumerate() {
        obj[b * 16 + 1..][..colors.len()].copy_from_slice(colors);
    }

    let tiles = bg_tiles(0);
    let mut put = |at: u16, src: Vec<Tile4>| tiles[at as usize..][..src.len()].copy_from_slice(&src);
    put(T_BRICK, pack(&grid(16, 16, brick)));
    put(T_EDGE, pack(&grid(16, 8, edge)));
    put(T_GOAL, pack(&grid(16, 16, goal)));
    put(T_SPRING, pack(&grid(16, 16, spring)));
    put(T_CLOUD, pack(&grid(32, 16, cloud)));

    let objs = obj_tiles();
    objs[S_STICK as usize..][..64].copy_from_slice(&pack(&grid(64, 64, stick)));
    for (i, g) in GLYPHS.iter().enumerate() {
        objs[S_FONT as usize + i] = glyph(g);
    }
    objs[S_HEART as usize] = heart(false);
    objs[S_HEART as usize + 1] = heart(true);

    // Clouds scattered over the parallax layer.
    let clouds = screen_block(CLOUD_BLOCK);
    clouds.fill(ScreenEntry::EMPTY);
    for &(cx, cy) in &[(1, 2), (12, 5), (22, 1), (6, 12), (18, 15), (27, 10), (3, 22), (14, 25), (24, 20), (9, 29)] {
        for t in 0..8u16 {
            let (tx, ty) = (cx + (t % 4) as usize, cy + (t / 4) as usize);
            clouds[(ty % 32) * 32 + tx % 32] = ScreenEntry::new(T_CLOUD + t, 3);
        }
    }
}

/// Uploads graphics and configures the display.
fn setup() {
    load_graphics();
    reg::BGCNT[0].write(BgCnt::new().priority(1).screen_block(MAZE_BLOCK as u16).0);
    reg::BGCNT[2].write(BgCnt::new().priority(3).screen_block(CLOUD_BLOCK as u16).0);
    reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG0 | dispcnt::BG2 | dispcnt::OBJ | dispcnt::OBJ_1D);
    reg::BLDCNT.write(blend::cnt(blend::ALPHA, layer::BG2, layer::BACKDROP));
    reg::BLDALPHA.write(blend::alpha(10, 6));
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    setup();
    unsafe { *GAME.0.get() = Some(Game::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    if let Some(g) = game() {
        g.update();
    }
}

/// Sky gradient: a new backdrop color every scanline.
#[unsafe(no_mangle)]
pub extern "C" fn scanline(y: i32) {
    let f = y as f32 / 159.0;
    let wave = (f * PI).sin() * 0.15;
    let lerp = |a: f32, b: f32| (a + (b - a) * f + wave * 40.0).clamp(0.0, 255.0) as u8;
    bg_palette()[0] = rgb(lerp(110.0, 255.0), lerp(198.0, 214.0), lerp(255.0, 232.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each test thread gets its own hardware and its own game.
    fn boot() -> Game {
        host::boot(1);
        setup();
        Game::new()
    }

    fn run(g: &mut Game, frame: &mut u32, keys: u16, frames: u32) {
        for _ in 0..frames {
            host::frame(*frame, keys, || g.update());
            *frame += 1;
        }
    }

    #[test]
    fn idling_in_the_start_room_is_safe() {
        let g = &mut boot();
        let mut f = 0;
        run(g, &mut f, 0, 600);
        assert_eq!(g.hearts, HEARTS, "a full spin in the start room never touches a wall");
        assert!(!g.started);
        assert_eq!(g.time, 0);
    }

    #[test]
    fn walls_cost_hearts_and_bounce_back() {
        let g = &mut boot();
        let mut f = 0;
        run(g, &mut f, key::UP, 120);
        assert!(g.hearts < HEARTS);
        assert!(g.y > 16.0 + 4.0, "pushed back out of the wall");
    }

    #[test]
    fn running_out_of_hearts_restarts() {
        let g = &mut boot();
        let mut f = 0;
        run(g, &mut f, key::UP | key::A, 600);
        assert!(matches!(g.phase, Phase::Over(_)) || g.hearts == HEARTS);
    }

    #[test]
    fn reaching_the_goal_records_a_best_time() {
        let g = &mut boot();
        let mut f = 0;
        g.started = true;
        g.time = 1234;
        (g.x, g.y) = (43.5 * CELL, 23.0 * CELL);
        run(g, &mut f, 0, 1);
        assert!(matches!(g.phase, Phase::Clear(_)));
        assert_eq!(g.best, Some(1235));
        assert_eq!(&save_ram()[..4], SAVE_MAGIC);
    }

    #[test]
    fn level_rows_are_rectangular() {
        let w = LEVEL.lines().next().unwrap().len();
        assert!(LEVEL.lines().all(|l| l.len() == w));
    }

    #[test]
    fn renders_a_frame() {
        let g = &mut boot();
        let mut f = 0;
        run(g, &mut f, 0, 20);
        let fb = host::render_with_scanline(|y| scanline(y));
        if let Some(dir) = std::env::var_os("WASM_AD_DUMP") {
            host::write_ppm(std::path::Path::new(&dir).join("twirl.ppm"), &fb).unwrap();
        }
        assert!(fb.iter().any(|&p| p != fb[0]));
    }
}
