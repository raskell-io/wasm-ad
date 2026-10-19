//! Conformance: tiled backgrounds (SPEC.md §6.1, §6.2, §6.4, §8.1, §8.2, §10.2).
//!
//! Scenes of 30 frames: 0 four layers and their priority order; 1 flips and
//! palette banks; 2 8bpp tiles; 3 map sizes and scroll wrap; 4 mosaic; 5 tile
//! data past VRAM 0x10000 reads as transparent.

#[path = "../../../common.rs"]
mod common;

use common::*;
use wasm_ad::*;

wasm_ad::cart! { title: "Conformance: tiles", author: "raskell.io" }

/// 4bpp tiles 1–32 in char block 0, holey (see-through) ones 33–64.
/// 8bpp tiles 1–16 in char block 2.
#[unsafe(no_mangle)]
pub extern "C" fn start() {
    ramp(bg_palette());
    bg_palette()[0] = rgb(16, 24, 40);
    let t4 = bg_tiles(0);
    for i in 1..=32 {
        t4[i] = pattern4(i as u32);
        t4[i + 32] = holey4(i as u32);
    }
    let t8 = bg_tiles8(2);
    for i in 1..=16 {
        t8[i] = pattern8(i as u32);
    }
}

/// Fills screen block `sbb` with `f(col, row)`.
fn fill(sbb: usize, f: impl Fn(usize, usize) -> ScreenEntry) {
    let m = screen_block(sbb);
    for (i, e) in m.iter_mut().enumerate() {
        *e = f(i % 32, i / 32);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let (s, t) = scene();
    if t == 0 {
        reset_video();
    }
    let t16 = t as u16;
    match s {
        0 => {
            // Each layer covers a different sparse pattern; equal priorities
            // resolve to the lower-numbered background.
            fill(28, |c, r| if (c + r) % 2 == 0 { ScreenEntry::new(33 + (c % 8) as u16, 0) } else { ScreenEntry::EMPTY });
            fill(29, |c, r| if c % 3 == 0 { ScreenEntry::new(40, 1) } else if r % 4 == 0 { ScreenEntry::new(2, 2) } else { ScreenEntry::EMPTY });
            fill(30, |_, r| if r % 3 != 1 { ScreenEntry::new(45 + (r % 4) as u16, 3) } else { ScreenEntry::EMPTY });
            fill(27, |_, _| ScreenEntry::new(5, 4));
            let prio = [1, 1, 0, 3];
            for (bg, sbb) in [28u16, 29, 30, 27].into_iter().enumerate() {
                reg::BGCNT[bg].write(BgCnt::new().screen_block(sbb).priority(prio[bg]).0);
                reg::BGHOFS[bg].write(t16 * (bg as u16 + 1));
                reg::BGVOFS[bg].write(t16 * (3 - bg as u16));
            }
            reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG0 | dispcnt::BG1 | dispcnt::BG2 | dispcnt::BG3);
        }
        1 => {
            fill(28, |c, r| {
                let e = ScreenEntry::new(1 + ((c + r) % 32) as u16, ((c * 3 + r) % 16) as u16);
                match (c + 2 * r) % 4 {
                    1 => e.hflip(),
                    2 => e.vflip(),
                    3 => e.hflip().vflip(),
                    _ => e,
                }
            });
            reg::BGCNT[0].write(BgCnt::new().screen_block(28).0);
            reg::BGHOFS[0].write(t16 * 3);
            reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG0);
        }
        2 => {
            // 8bpp: an index counts 64 bytes and the bank bits are ignored.
            fill(28, |c, r| ScreenEntry::new(1 + ((c + r * 3) % 16) as u16, 15).hflip());
            reg::BGCNT[0].write(BgCnt::new().char_block(2).bpp8().screen_block(28).0);
            reg::BGVOFS[0].write(t16 * 5);
            reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG0);
        }
        3 => {
            // 64×32 in blocks 8–9, 32×64 in 12–13, 64×64 in 16–19; each block
            // gets its own tile so the order shows. Offsets use 9 bits.
            for b in 0..4usize {
                fill(8 + b, |c, r| ScreenEntry::new(if (c + r) % 2 == 0 { 1 + b as u16 } else { 0 }, b as u16));
                fill(12 + b, |c, _| ScreenEntry::new(if c % 2 == 0 { 9 + b as u16 } else { 0 }, 4 + b as u16));
                fill(16 + b, |_, r| ScreenEntry::new(if r % 2 == 0 { 17 + b as u16 } else { 0 }, 8 + b as u16));
            }
            reg::BGCNT[0].write(BgCnt::new().screen_block(8).size(MapSize::S64x32).priority(2).0);
            reg::BGCNT[1].write(BgCnt::new().screen_block(12).size(MapSize::S32x64).priority(1).0);
            reg::BGCNT[2].write(BgCnt::new().screen_block(16).size(MapSize::S64x64).0);
            reg::BGHOFS[0].write(t16 * 17 + 0xFE00); // high bits ignored
            reg::BGVOFS[1].write(t16 * 19 + 200);
            reg::BGHOFS[2].write(500 - t16 * 7);
            reg::BGVOFS[2].write(t16 * 23);
            reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG0 | dispcnt::BG1 | dispcnt::BG2);
        }
        4 => {
            fill(28, |c, r| ScreenEntry::new(1 + ((c * 5 + r) % 32) as u16, 0));
            fill(29, |c, r| if (c / 2 + r) % 3 == 0 { ScreenEntry::new(40, 0) } else { ScreenEntry::EMPTY });
            reg::BGCNT[0].write(BgCnt::new().screen_block(28).mosaic().priority(1).0);
            reg::BGCNT[1].write(BgCnt::new().screen_block(29).0);
            reg::MOSAIC.write((t16 % 16) | ((t16 / 2 % 16) << 4) | 0xFF00);
            reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG0 | dispcnt::BG1);
        }
        _ => {
            // Char block 3, 4bpp: index 511 is at 0xC000 + 511 × 32 ≥ 0x10000.
            // Char block 3, 8bpp: index 256 is at 0xC000 + 256 × 64 = 0x10000.
            bg_tiles(3)[200] = pattern4(9);
            bg_tiles8(3)[3] = pattern8(5);
            fill(28, |c, _| ScreenEntry::new(if c % 2 == 0 { 200 } else { 511 }, 1));
            fill(29, |c, r| ScreenEntry::new(if r % 2 == 0 { 3 } else { 256 + (c % 2) as u16 * 300 }, 0));
            reg::BGCNT[0].write(BgCnt::new().char_block(3).screen_block(28).0);
            reg::BGCNT[1].write(BgCnt::new().char_block(3).bpp8().screen_block(29).priority(1).0);
            reg::BGHOFS[1].write(t16);
            reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG0 | dispcnt::BG1);
        }
    }
}
