//! Conformance: affine backgrounds (SPEC.md §6.1, §6.2, §8.4).
//!
//! Scenes of 30 frames: 0 mode 1 with a clipped, rotating and scaling BG2
//! over tiled BG0; 1 every map size with wrap, shear and mosaic; 2 mode 2
//! with BG2 and BG3, negative reference points and the ignored bits 28–31;
//! 3 a "mode 7" floor: BG2X, BG2Y, PA and PC set per line from `scanline`;
//! 4 a 128×128 map at screen block 31, whose end lies past VRAM 0x10000.

#[path = "../../../common.rs"]
mod common;

use common::*;
use wasm_ad::*;

wasm_ad::cart! { title: "Conformance: affine", author: "raskell.io" }

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    ramp(bg_palette());
    bg_palette()[0] = rgb(20, 20, 60);
    // 8bpp tiles 1–63 in char block 0; tile 0 stays transparent.
    let t8 = bg_tiles8(0);
    for i in 1..64 {
        t8[i] = pattern8(i as u32);
    }
    // 4bpp tile for BG0, in char block 1.
    bg_tiles(1)[1] = holey4(2);
}

/// An N×N byte map at screen block `sbb`, a mix of tiles and holes.
fn map(sbb: usize, n: usize, seed: usize) {
    let m = affine_map(sbb);
    for i in 0..(n * n).min(m.len()) {
        let (c, r) = (i % n, i / n);
        m[i] = if (c * 3 + r * 5 + seed) % 7 == 0 { 0 } else { ((c + r * 2 + seed) % 63 + 1) as u8 };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let (s, t) = scene();
    if t == 0 {
        reset_video();
        match s {
            0 => {
                map(16, 16, 0);
                for e in screen_block(30).iter_mut() {
                    *e = ScreenEntry::new(1, 2);
                }
            }
            1 => {
                map(8, 16, 1);
                map(10, 32, 2);
                map(16, 64, 3);
            }
            2 => {
                map(16, 32, 4);
                map(20, 32, 5);
            }
            3 => map(16, 64, 6),
            _ => map(31, 128, 7),
        }
    }
    match s {
        0 => {
            reg::BGCNT[0].write(BgCnt::new().char_block(1).screen_block(30).priority(1).0);
            reg::BGHOFS[0].write(t as u16);
            reg::BGCNT[2].write(BgCnt::new().screen_block(16).affine_size(AffineSize::S16x16).0);
            let m = AffineMatrix::rotate_scale_fixed(t * 5, 200 + t * 6, 256);
            set_bg_affine(2, m, 64, 64, 120, 80);
            reg::DISPCNT.write(dispcnt::MODE1 | dispcnt::BG0 | dispcnt::BG2);
        }
        1 => {
            let (sbb, size) = [(8, AffineSize::S16x16), (10, AffineSize::S32x32), (16, AffineSize::S64x64)][(t / 10) as usize];
            let mut cnt = BgCnt::new().screen_block(sbb).affine_size(size).wrap();
            if t % 2 == 1 {
                cnt = cnt.mosaic();
                reg::MOSAIC.write(2 | (3 << 4));
            }
            reg::BGCNT[2].write(cnt.0);
            let shear = AffineMatrix { pa: 256, pb: (t * 9) as i16, pc: -(t * 5) as i16, pd: 300 };
            set_bg_affine(2, shear, t * 7, t * 3, 0, 0);
            reg::DISPCNT.write(dispcnt::MODE1 | dispcnt::BG2);
        }
        2 => {
            reg::BGCNT[2].write(BgCnt::new().screen_block(16).affine_size(AffineSize::S32x32).priority(1).0);
            reg::BGCNT[3].write(BgCnt::new().screen_block(20).affine_size(AffineSize::S32x32).wrap().0);
            set_bg_affine(2, AffineMatrix::rotate_scale_fixed(-t * 3, 256, 256), 0, 0, 30, 20);
            // BG3: identity, reference point −1000.5 px with junk in bits 28–31.
            reg::BG3PA.write(256);
            reg::BG3PB.write(0);
            reg::BG3PC.write(0);
            reg::BG3PD.write(256);
            reg::BG3X.write(((-1000 * 256 - 128 + t * 64) as u32 & 0x0FFF_FFFF) | 0xA000_0000);
            reg::BG3Y.write((t * 300) as u32 | 0x5000_0000);
            reg::DISPCNT.write(dispcnt::MODE2 | dispcnt::BG2 | dispcnt::BG3 | dispcnt::BG0);
        }
        3 => {
            reg::BGCNT[2].write(BgCnt::new().screen_block(16).affine_size(AffineSize::S64x64).wrap().0);
            reg::DISPCNT.write(dispcnt::MODE1 | dispcnt::BG2);
        }
        _ => {
            // Rows of the 128×128 map past VRAM 0x10000 read as transparent.
            reg::BGCNT[2].write(BgCnt::new().screen_block(31).affine_size(AffineSize::S128x128).0);
            set_bg_affine(2, AffineMatrix { pa: 512, pb: 0, pc: 0, pd: 2048 }, 0, t * 40, 0, 0);
            reg::DISPCNT.write(dispcnt::MODE1 | dispcnt::BG2);
        }
    }
}

/// Scene 3's floor: below the horizon each line gets its own scale and
/// reference point, so PB and PD stay 0 (SPEC.md §8.4).
#[unsafe(no_mangle)]
pub extern "C" fn scanline(y: i32) {
    let (s, t) = scene();
    if s != 3 {
        return;
    }
    let horizon = 40;
    if y < horizon {
        Reg16(addr::bgpa(2)).write(0);
        Reg16(addr::bgpc(2)).write(0);
        reg::BG2X.write(0);
        reg::BG2Y.write(0);
        return;
    }
    let dist = 256 * 64 / (y - horizon + 1); // 8.8 scale for this line
    let angle = t * 2;
    let (sin, cos) = (fixed::sin(angle), fixed::cos(angle));
    let pa = (cos * dist) >> 14;
    let pc = (sin * dist) >> 14;
    reg::BG2PA.write(pa as u16);
    reg::BG2PB.write(0);
    reg::BG2PC.write(pc as u16);
    reg::BG2PD.write(0);
    // Camera at (256, 256 + 4t) in the map, looking along the angle.
    let (cx, cy) = (256 * 256, (256 + t * 4) * 256);
    reg::BG2X.write((cx - pa * 120) as u32);
    reg::BG2Y.write((cy + dist * 64 - pc * 120) as u32);
}
