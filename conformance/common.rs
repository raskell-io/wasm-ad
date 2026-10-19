//! Helpers shared by the conformance carts, included with `#[path]`.
//!
//! Every cart runs a fixed script of scenes keyed off `FRAME`, so a host's
//! output depends only on what the spec says it should.

#![allow(dead_code)]

use std::cell::UnsafeCell;
use wasm_ad::*;

/// Frames per scene.
pub const SCENE: u32 = 30;

/// The current scene and the frame within it.
pub fn scene() -> (u32, i32) {
    (frame() / SCENE, (frame() % SCENE) as i32)
}

/// Cart state kept in linear memory (SPEC.md §12).
pub struct Global<T>(pub UnsafeCell<T>);
// SAFETY: wasm32-unknown-unknown is single-threaded.
unsafe impl<T> Sync for Global<T> {}
impl<T> Global<T> {
    pub const fn new(v: T) -> Self {
        Self(UnsafeCell::new(v))
    }
    #[allow(clippy::mut_from_ref)]
    pub fn get(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}

/// 255 distinct colors for indices 1–255; index 0 stays as it is.
pub fn ramp(pal: &mut [u16]) {
    for i in 1..256u32 {
        pal[i as usize] = rgb((i * 37 % 256) as u8, (i * 91 % 256) as u8, (i * 53 % 256) as u8);
    }
}

/// A 4bpp tile whose pixels all differ in a way flips and offsets reveal.
pub fn pattern4(seed: u32) -> Tile4 {
    let mut t = [0u8; 32];
    for y in 0..8u32 {
        for x in 0..8u32 {
            let c = ((x * 3 + y * 5 + seed * 7) % 15 + 1) as u8;
            t[(y * 4 + x / 2) as usize] |= c << (4 * (x % 2));
        }
    }
    t
}

/// Like [`pattern4`], with a transparent diagonal so layers beneath show.
pub fn holey4(seed: u32) -> Tile4 {
    let mut t = pattern4(seed);
    for i in 0..8usize {
        let shift = 4 * (i % 2);
        t[i * 4 + i / 2] &= !(0xF << shift);
    }
    t
}

/// An 8bpp tile using colors 1–255.
pub fn pattern8(seed: u32) -> Tile8 {
    let mut t = [0u8; 64];
    for y in 0..8u32 {
        for x in 0..8u32 {
            t[(y * 8 + x) as usize] = ((x * 7 + y * 13 + seed * 31) % 255 + 1) as u8;
        }
    }
    t
}

/// Display registers to zero, sprites hidden, BG2 and BG3 matrices identity.
pub fn reset_video() {
    for a in (0x400..0x460).step_by(2) {
        if a != addr::VCOUNT {
            Reg16(a).write(0);
        }
    }
    for bg in [2, 3] {
        Reg16(addr::bgpa(bg)).write(256);
        Reg16(addr::bgpd(bg)).write(256);
    }
    oam::hide_all();
}

/// Draws `value`'s low `bits` bits as a row of 8×8 squares on BG3 (screen
/// block 31, 4bpp tiles 1 = off and 2 = on in block 3), for showing register
/// values in the frame hash.
pub fn indicator_setup() {
    let t = bg_tiles(3);
    t[1] = [0x33; 32];
    t[2] = [0x44; 32];
    let p = bg_palette();
    p[3] = rgb(48, 16, 16);
    p[4] = rgb(64, 240, 96);
}

pub fn indicator(row: usize, col: usize, value: u32, bits: u32) {
    let map = screen_block(31);
    for b in 0..bits as usize {
        let on = value >> b & 1 != 0;
        map[row * 32 + col + b] = ScreenEntry::new(if on { 2 } else { 1 }, 0);
    }
}
