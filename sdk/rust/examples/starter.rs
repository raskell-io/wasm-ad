//! The starter cart, the same in every SDK: a scrolling checkerboard blended
//! over a backdrop gradient drawn per scanline, and a spinning star you move
//! with the D-pad (B to go faster, A to toggle mosaic). It uses integer math
//! only, so every language's build renders bit-identical frames.
//!
//!   cargo build --release --target wasm32-unknown-unknown -p wasm-ad --example starter

use std::cell::UnsafeCell;
use wasm_ad::*;

wasm_ad::cart! {
    title: "Starter",
    author: "raskell.io",
    description: "The WASM-AD starter cart, in Rust"
}

const MAP_BLOCK: u16 = 31;

struct State {
    buttons: Buttons,
    x: i32,
    y: i32,
    mosaic: bool,
}

// Cart state lives in linear memory, never in WebAssembly globals (SPEC.md §12).
struct Global(UnsafeCell<State>);
// SAFETY: wasm32-unknown-unknown is single-threaded.
unsafe impl Sync for Global {}
static STATE: Global = Global(UnsafeCell::new(State { buttons: Buttons::new(), x: 112, y: 72, mosaic: false }));

fn state() -> &'static mut State {
    unsafe { &mut *STATE.0.get() }
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    trace("starter: hello from Rust");

    // A checkerboard of two solid tiles on background 0.
    let bg = bg_palette();
    bg[1] = rgb(255, 255, 255);
    bg[2] = rgb(40, 40, 72);
    let tiles = bg_tiles(0);
    tiles[1] = [0x11; 32];
    tiles[2] = [0x22; 32];
    let map = screen_block(MAP_BLOCK as usize);
    for (i, e) in map.iter_mut().enumerate() {
        let (col, row) = (i % 32, i / 32);
        *e = ScreenEntry::new(if (col / 2 + row / 2) % 2 == 0 { 1 } else { 2 }, 0);
    }

    // A 16×16 star in four 1D-mapped sprite tiles.
    let obj = obj_palette();
    obj[1] = rgb(255, 216, 64);
    obj[2] = rgb(232, 96, 32);
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

    reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG0 | dispcnt::OBJ | dispcnt::OBJ_1D);
    reg::BLDCNT.write(blend::cnt(blend::ALPHA, layer::BG0, layer::BACKDROP));
    reg::BLDALPHA.write(blend::alpha(6, 10));
    reg::MOSAIC.write(0x3333);
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
    }

    let t = frame() as i32;
    reg::BGHOFS[0].write((t / 2) as u16);
    reg::BGVOFS[0].write((t / 3) as u16);
    let cnt = BgCnt::new().screen_block(MAP_BLOCK).priority(1);
    reg::BGCNT[0].write(if s.mosaic { cnt.mosaic().0 } else { cnt.0 });

    // Spin a quarter turn every 32 frames while the size pulses ±25%.
    let scale = 256 + ((fixed::sin(t * 3) * 64) >> 14);
    oam::set_affine(0, AffineMatrix::rotate_scale_fixed(t * 2, scale, scale));
    let star = Sprite::new(s.x, s.y, SpriteSize::S16x16, 0).affine(0, true);
    oam::set(0, if s.mosaic { star.mosaic() } else { star });
}

/// Raster effect: a new backdrop color on every line.
#[unsafe(no_mangle)]
pub extern "C" fn scanline(y: i32) {
    let lerp = |a: i32, b: i32| (a + (b - a) * y / 159) as u8;
    bg_palette()[0] = rgb(lerp(40, 255), lerp(60, 140), lerp(160, 96));
}
