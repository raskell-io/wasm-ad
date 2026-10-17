//! A tour of WASM-AD in one screen: a sky gradient drawn by changing the
//! backdrop color every scanline, a checkerboard blended over it, and a
//! spinning affine sprite you move with the D-pad. A toggles mosaic.

use std::cell::UnsafeCell;
use wasm_ad::*;

wasm_ad::cart! {
    title: "Hello WASM-AD",
    author: "raskell.io",
    version: "0.1.0",
    description: "Raster gradient, alpha blending and an affine sprite"
}

const MAP_BLOCK: usize = 31;

struct State {
    buttons: Buttons,
    x: i32,
    y: i32,
    mosaic: bool,
}

struct Global(UnsafeCell<State>);
// SAFETY: wasm32-unknown-unknown is single-threaded.
unsafe impl Sync for Global {}
static STATE: Global = Global(UnsafeCell::new(State { buttons: Buttons::new(), x: 112, y: 72, mosaic: false }));

fn state() -> &'static mut State {
    unsafe { &mut *STATE.0.get() }
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    trace("hello from WASM-AD");

    // Checkerboard: two solid tiles in alternating squares.
    let bg = bg_palette();
    bg[1] = rgb(255, 255, 255);
    bg[2] = rgb(40, 40, 72);
    let tiles = bg_tiles(0);
    tiles[1] = [0x11; 32];
    tiles[2] = [0x22; 32];
    let map = screen_block(MAP_BLOCK);
    for (i, e) in map.iter_mut().enumerate() {
        let (col, row) = (i % 32, i / 32);
        *e = ScreenEntry::new(if (col / 2 + row / 2) % 2 == 0 { 1 } else { 2 }, 0);
    }
    reg::BGCNT[0].write(BgCnt::new().screen_block(MAP_BLOCK as u16).priority(1).0);

    // A 16×16 star, drawn into four 1D-mapped sprite tiles.
    let obj = obj_palette();
    obj[1] = rgb(255, 216, 64);
    obj[2] = rgb(232, 96, 32);
    let star = obj_tiles();
    for y in 0..16i32 {
        for x in 0..16i32 {
            let (dx, dy) = ((x * 2 - 15).abs(), (y * 2 - 15).abs());
            let c = if dx + dy < 14 || dx.min(dy) < 3 && dx.max(dy) < 15 {
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

    let t = frame();
    reg::BGHOFS[0].write((t / 2) as u16);
    reg::BGVOFS[0].write((t / 3) as u16);

    let cnt = BgCnt::new().screen_block(MAP_BLOCK as u16).priority(1);
    reg::BGCNT[0].write(if s.mosaic { cnt.mosaic().0 } else { cnt.0 });
    reg::MOSAIC.write(0x3333);

    let pulse = 1.0 + 0.25 * (t as f32 * 0.07).sin();
    oam::set_affine(0, AffineMatrix::rotate_scale(t as f32 * 0.04, pulse, pulse));
    let star = Sprite::new(s.x, s.y, SpriteSize::S16x16, 0).affine(0, true);
    oam::set(0, if s.mosaic { star.mosaic() } else { star });
}

/// Raster effect: a new backdrop color on every line.
#[unsafe(no_mangle)]
pub extern "C" fn scanline(y: i32) {
    let f = y as f32 / 159.0;
    let lerp = |a: f32, b: f32| (a + (b - a) * f) as u8;
    bg_palette()[0] = rgb(lerp(40.0, 255.0), lerp(60.0, 140.0), lerp(160.0, 96.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boots_and_draws_the_star() {
        host::boot(1);
        start();
        for f in 0..30 {
            host::frame(f, key::RIGHT, || update());
        }
        let fb = host::render_with_scanline(|y| scanline(y));
        let star_color = wasm_ad_core_rgba(rgb(232, 96, 32));
        let (cx, cy) = (state().x as usize + 16, state().y as usize + 16);
        assert_eq!(fb[cy * 240 + cx], star_color, "star centre at ({cx}, {cy})");
        if let Some(dir) = std::env::var_os("WASM_AD_DUMP") {
            host::write_ppm(std::path::Path::new(&dir).join("hello.ppm"), &fb).unwrap();
        }
    }

    fn wasm_ad_core_rgba(c: u16) -> u32 {
        let e = |v: u16| ((v << 3) | (v >> 2)) as u32;
        0xFF00_0000 | (e((c >> 10) & 31) << 16) | (e((c >> 5) & 31) << 8) | e(c & 31)
    }
}
