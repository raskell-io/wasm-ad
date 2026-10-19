//! The starter cart, the same in every SDK: a rotating checkerboard (an
//! affine background) blended over a backdrop gradient drawn per scanline,
//! a spinning star you move with the D-pad, and a little tune on all four
//! sound channels. B goes faster and plays a sample; A toggles mosaic with a
//! rising blip. A second controller brings in a blue star for player 2. It
//! uses integer math only, so every language's build renders bit-identical
//! frames and sound.
//!
//!   cargo build --release --target wasm32-unknown-unknown -p wasm-ad --example starter

use std::cell::UnsafeCell;
use wasm_ad::*;

wasm_ad::cart! {
    title: "Starter",
    author: "raskell.io",
    description: "The WASM-AD starter cart, in Rust"
}

const MAP_BLOCK: u16 = 8;
/// The zap sample's length in sample RAM.
const ZAP_LEN: u16 = 2048;
/// An arpeggio for the square channel and a bass line for the wave channel, in Hz.
const NOTES: [u32; 8] = [262, 330, 392, 523, 392, 330, 262, 196];
const BASS: [u32; 4] = [65, 55, 44, 49];

struct State {
    buttons: Buttons,
    x: i32,
    y: i32,
    mosaic: bool,
    p2: Buttons,
    x2: i32,
    y2: i32,
}

// Cart state lives in linear memory, never in WebAssembly globals (SPEC.md §12).
struct Global(UnsafeCell<State>);
// SAFETY: wasm32-unknown-unknown is single-threaded.
unsafe impl Sync for Global {}
static STATE: Global = Global(UnsafeCell::new(State {
    buttons: Buttons::new(),
    x: 112,
    y: 72,
    mosaic: false,
    p2: Buttons::new(),
    x2: 40,
    y2: 72,
}));

fn state() -> &'static mut State {
    unsafe { &mut *STATE.0.get() }
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    trace("starter: hello from Rust");

    // A checkerboard of two solid 8bpp tiles on affine background 2, 32×32 tiles.
    let bg = bg_palette();
    bg[1] = rgb(255, 255, 255);
    bg[2] = rgb(40, 40, 72);
    let tiles = bg_tiles8(0);
    tiles[1] = [1; 64];
    tiles[2] = [2; 64];
    let map = affine_map(MAP_BLOCK as usize);
    for (i, e) in map[..32 * 32].iter_mut().enumerate() {
        let (col, row) = (i % 32, i / 32);
        *e = if (col / 2 + row / 2) % 2 == 0 { 1 } else { 2 };
    }

    // A 16×16 star in four 1D-mapped sprite tiles.
    let obj = obj_palette();
    obj[1] = rgb(255, 216, 64);
    obj[2] = rgb(232, 96, 32);
    // Player 2's star: the same tiles in palette bank 1.
    obj[17] = rgb(96, 200, 255);
    obj[18] = rgb(40, 80, 220);
    let star = obj_tiles();
    for y in 0..16i32 {
        for x in 0..16i32 {
            let (dx, dy) = ((x * 2 - 15).abs(), (y * 2 - 15).abs());
            let c = if dx + dy < 14 || (dx.min(dy) < 3 && dx.max(dy) < 15) {
                if dx + dy < 7 { 2 } else { 1 }
            } else {
                0
            };
            let tile = (y / 8 * 2 + x / 8) as usize;
            let at = ((y % 8) * 4 + (x % 8) / 2) as usize;
            star[tile][at] |= c << (4 * (x % 2));
        }
    }

    reg::DISPCNT.write(dispcnt::MODE1 | dispcnt::BG2 | dispcnt::OBJ | dispcnt::OBJ_1D);
    reg::BLDCNT.write(blend::cnt(blend::ALPHA, layer::BG2, layer::BACKDROP));
    reg::BLDALPHA.write(blend::alpha(6, 10));
    reg::MOSAIC.write(0x3333);

    // A triangle wave for the bass, and a falling zap in sample RAM.
    let wave = sound::wave_ram();
    for (i, b) in wave[..16].iter_mut().enumerate() {
        let level = |j: usize| (if j < 16 { j } else { 31 - j }) as u8;
        *b = (level(i * 2) << 4) | level(i * 2 + 1);
    }
    let zap = sound::sample_ram();
    for i in 0..ZAP_LEN as i32 {
        let s = (fixed::sin((i * i) >> 9) * 127) >> 14;
        zap[i as usize] = ((s * (ZAP_LEN as i32 - i)) >> 11) as i8;
    }
    sound::enable();
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let s = state();
    s.buttons.update();
    let speed = if s.buttons.held(key::B) { 3 } else { 1 };
    if s.buttons.held(key::LEFT) { s.x -= speed }
    if s.buttons.held(key::RIGHT) { s.x += speed }
    if s.buttons.held(key::UP) { s.y -= speed }
    if s.buttons.held(key::DOWN) { s.y += speed }
    s.x = s.x.clamp(0, 240 - 32);
    s.y = s.y.clamp(0, 160 - 32);
    if s.buttons.pressed(key::A) {
        s.mosaic = !s.mosaic;
        // A rising blip: the sweep raises the pitch until it runs out.
        reg::SOUND1CNT_L.write(sound::sweep(2, 2, false));
        reg::SOUND1CNT_H.write(sound::DUTY_50 | sound::envelope(12, 2, false));
        reg::SOUND1CNT_X.write(sound::START | sound::rate(440));
    }
    if s.buttons.pressed(key::B) {
        pcm::play(0, 0, ZAP_LEN, 16000, 48);
    }

    let t = frame() as i32;
    // The tune: a note every 16 frames, a hi-hat between them, bass every 64.
    if t % 16 == 0 {
        reg::SOUND2CNT_L.write(sound::DUTY_25 | sound::envelope(9, 1, false));
        reg::SOUND2CNT_H.write(sound::START | sound::rate(NOTES[(t / 16 % 8) as usize]));
    }
    if t % 16 == 8 {
        reg::SOUND4CNT_L.write(sound::envelope(6, 1, false));
        reg::SOUND4CNT_H.write(sound::START | sound::noise(1, 1, false));
    }
    if t % 64 == 0 {
        reg::SOUND3CNT_L.write(sound::WAVE_ON);
        reg::SOUND3CNT_H.write(sound::WAVE_50 | (256 - 48));
        reg::SOUND3CNT_X.write(sound::START | sound::LENGTH | sound::rate(BASS[(t / 64 % 4) as usize] * 2));
    }

    // Turn once every 1024 frames around the screen centre, scrolling diagonally.
    let cnt = BgCnt::new().screen_block(MAP_BLOCK).affine_size(AffineSize::S32x32).wrap().priority(1);
    reg::BGCNT[2].write(if s.mosaic { cnt.mosaic().0 } else { cnt.0 });
    let m = AffineMatrix::rotate_scale_fixed(t / 4, 256, 256);
    set_bg_affine(2, m, (t / 2) % 256, (t / 3) % 256, 120, 80);

    // Spin a quarter turn every 32 frames while the size pulses ±25%.
    let scale = 256 + ((fixed::sin(t * 3) * 64) >> 14);
    oam::set_affine(0, AffineMatrix::rotate_scale_fixed(t * 2, scale, scale));
    let star = Sprite::new(s.x, s.y, SpriteSize::S16x16, 0).affine(0, true);
    oam::set(0, if s.mosaic { star.mosaic() } else { star });

    // Player 2 joins when a second controller connects (SPEC.md §5).
    if connected(2) {
        s.p2.update_for(2);
        if s.p2.held(key::LEFT) { s.x2 -= 1 }
        if s.p2.held(key::RIGHT) { s.x2 += 1 }
        if s.p2.held(key::UP) { s.y2 -= 1 }
        if s.p2.held(key::DOWN) { s.y2 += 1 }
        s.x2 = s.x2.clamp(0, 240 - 32);
        s.y2 = s.y2.clamp(0, 160 - 32);
        oam::set(1, Sprite::new(s.x2, s.y2, SpriteSize::S16x16, 0).bank(1).affine(0, true));
    } else {
        oam::hide(1);
    }
}

/// Raster effect: a new backdrop color on every line.
#[unsafe(no_mangle)]
pub extern "C" fn scanline(y: i32) {
    let lerp = |a: i32, b: i32| (a + (b - a) * y / 159) as u8;
    bg_palette()[0] = rgb(lerp(40, 255), lerp(60, 140), lerp(160, 96));
}
