//! Conformance: sprites (SPEC.md §9).
//!
//! Scenes of 30 frames: 0 all twelve sizes in 4bpp, 1D mapping, banks and
//! flips; 1 the same in 2D mapping and 8bpp; 2 overlap and priority against
//! backgrounds; 3 affine and double-size sprites; 4 coordinates that wrap; 5
//! mosaic, hidden encodings and tile indices that wrap at 1024.

#[path = "../../../common.rs"]
mod common;

use common::*;
use wasm_ad::*;

wasm_ad::cart! { title: "Conformance: sprites", author: "raskell.io" }

const SIZES: [SpriteSize; 12] = [
    SpriteSize::S8x8,
    SpriteSize::S16x16,
    SpriteSize::S32x32,
    SpriteSize::S64x64,
    SpriteSize::W16x8,
    SpriteSize::W32x8,
    SpriteSize::W32x16,
    SpriteSize::W64x32,
    SpriteSize::T8x16,
    SpriteSize::T8x32,
    SpriteSize::T16x32,
    SpriteSize::T32x64,
];

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    ramp(obj_palette());
    ramp(bg_palette());
    bg_palette()[0] = rgb(30, 30, 50);
    let ot = obj_tiles();
    for (i, t) in ot.iter_mut().enumerate() {
        *t = if i % 5 == 4 { holey4(i as u32) } else { pattern4(i as u32) };
    }
    // A background at priority 1 for sprites to go in front of or behind.
    bg_tiles(0)[1] = holey4(3);
    for (i, e) in screen_block(31).iter_mut().enumerate() {
        *e = if (i % 32 + i / 32) % 2 == 0 { ScreenEntry::new(1, 3) } else { ScreenEntry::EMPTY };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let (s, t) = scene();
    if t == 0 {
        reset_video();
    }
    let obj = dispcnt::MODE0 | dispcnt::OBJ;
    match s {
        0 | 1 => {
            for (i, &size) in SIZES.iter().enumerate() {
                let (x, y) = ((i % 4) as i32 * 60 + t, (i / 4) as i32 * 52);
                let mut sp = Sprite::new(x, y, size, (i * 37 % 900) as u16)
                    .bank(i as u16)
                    .hflip(i % 2 == 1)
                    .vflip(i % 3 == 1);
                if s == 1 {
                    sp = sp.bpp8();
                }
                oam::set(i, sp);
            }
            reg::DISPCNT.write(obj | if s == 0 { dispcnt::OBJ_1D } else { 0 });
        }
        2 => {
            // Overlapping sprites: lower priority number wins, then lower index;
            // BG0 at priority 1 hides priority-2 and -3 sprites.
            for i in 0..8 {
                let sp = Sprite::new(40 + i as i32 * 18 + t, 40 + (i % 3) as i32 * 10, SpriteSize::S32x32, (i * 16) as u16)
                    .priority([2, 0, 1, 1, 3, 0, 2, 1][i]);
                oam::set(i, sp);
            }
            reg::BGCNT[0].write(BgCnt::new().screen_block(31).priority(1).0);
            reg::DISPCNT.write(obj | dispcnt::OBJ_1D | dispcnt::BG0);
        }
        3 => {
            for slot in 0..4 {
                let scale = 128 + slot as i32 * 96;
                oam::set_affine(slot, AffineMatrix::rotate_scale_fixed(t * 4 + slot as i32 * 32, scale, 256 + t * 8));
            }
            for (i, &size) in SIZES.iter().enumerate() {
                let (x, y) = ((i % 4) as i32 * 60, (i / 4) as i32 * 52);
                oam::set(i, Sprite::new(x, y, size, (i * 64) as u16).affine((i % 4) as u16, i % 2 == 0));
            }
            reg::DISPCNT.write(obj | dispcnt::OBJ_1D);
        }
        4 => {
            // X ≥ 240 means X − 512, Y ≥ 160 means Y − 256.
            let xs = [500 + t, 230 + t, -40 + t * 3, 480];
            let ys = [150 + t, 250 - t, 10, 200 + t * 2];
            for i in 0..4 {
                oam::set(i, Sprite::new(xs[i], ys[i], SpriteSize::S64x64, (i * 64) as u16));
            }
            reg::DISPCNT.write(obj | dispcnt::OBJ_1D);
        }
        _ => {
            reg::MOSAIC.write(((t as u16 % 8) << 8) | ((t as u16 / 4 % 8) << 12));
            oam::set(0, Sprite::new(10, 10, SpriteSize::S64x64, 0).mosaic());
            oam::set_affine(0, AffineMatrix::rotate_scale_fixed(t * 3, 256, 256));
            oam::set(1, Sprite::new(80, 10, SpriteSize::S64x64, 64).affine(0, false).mosaic());
            // 64×64 from tile 1000 in 1D mapping wraps to tiles 0–39.
            oam::set(2, Sprite::new(150, 10, SpriteSize::S64x64, 1000));
            // Hidden: disable bit without affine, mode 3, shape 3.
            let hidden = Sprite::new(10, 90, SpriteSize::S32x32, 5);
            oam::set(3, Sprite { attr0: hidden.attr0 | 0x0200, ..hidden });
            oam::set(4, Sprite { attr0: hidden.attr0 | 0x0C00, attr1: hidden.attr1 + 40, ..hidden });
            oam::set(5, Sprite { attr0: hidden.attr0 | 0xC000, attr1: hidden.attr1 + 80, ..hidden });
            oam::set(6, Sprite::new(130, 100, SpriteSize::S32x32, 7));
            reg::DISPCNT.write(obj | dispcnt::OBJ_1D);
        }
    }
}
