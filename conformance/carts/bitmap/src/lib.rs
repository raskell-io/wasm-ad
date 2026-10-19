//! Conformance: bitmap modes (SPEC.md §8.3).
//!
//! Scenes of 30 frames: 0 mode 3 with scroll and mosaic; 1 mode 4 with page
//! flips and transparent index 0; 2 mode 5 with page flips, scroll and the
//! transparent area outside 160×128; 3 sprites in a bitmap mode, where tiles
//! below 512 are not drawn, blended over BG2.

#[path = "../../../common.rs"]
mod common;

use common::*;
use wasm_ad::*;

wasm_ad::cart! { title: "Conformance: bitmap", author: "raskell.io" }

fn put16(offset: usize, v: u16) {
    let vr = vram();
    vr[offset..offset + 2].copy_from_slice(&v.to_le_bytes());
}

fn color(x: u32, y: u32, k: u32) -> u16 {
    rgb((x * 3 + k) as u8, (y * 5 + k * 2) as u8, ((x ^ y) * 4) as u8)
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    ramp(bg_palette());
    ramp(obj_palette());
    bg_palette()[0] = rgb(10, 80, 40);
    let ot = obj_tiles();
    for (i, t) in ot.iter_mut().enumerate() {
        *t = pattern4(i as u32);
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
            if t == 0 {
                for y in 0..160 {
                    for x in 0..240 {
                        put16((y * 240 + x) * 2, color(x as u32, y as u32, 0));
                    }
                }
            }
            // Draw a moving line each frame on top of the image.
            for x in 0..240 {
                put16(((t as usize * 5 % 160) * 240 + x) * 2, rgb(255, 255, 255));
            }
            reg::BGHOFS[2].write(t16 % 8);
            reg::BGVOFS[2].write(t16 % 5);
            reg::BGCNT[2].write(BgCnt::new().mosaic().0);
            reg::MOSAIC.write((t16 / 4 % 4) | ((t16 / 6 % 4) << 4));
            reg::DISPCNT.write(dispcnt::MODE3 | dispcnt::BG2);
        }
        1 => {
            if t == 0 {
                let vr = vram();
                for y in 0..160usize {
                    for x in 0..240usize {
                        vr[y * 240 + x] = if (x / 8 + y / 8) % 3 == 0 { 0 } else { ((x + y) % 255 + 1) as u8 };
                        vr[0xA000 + y * 240 + x] = if (x / 6) % 4 == 0 { 0 } else { ((x * 3 + y * 7) % 255 + 1) as u8 };
                    }
                }
            }
            let page = if (t / 4) % 2 == 1 { dispcnt::PAGE1 } else { 0 };
            reg::BGHOFS[2].write(t16);
            reg::DISPCNT.write(dispcnt::MODE4 | dispcnt::BG2 | page);
        }
        2 => {
            if t == 0 {
                for y in 0..128 {
                    for x in 0..160 {
                        put16((y * 160 + x) * 2, color(x as u32, y as u32, 50));
                        put16(0xA000 + (y * 160 + x) * 2, color(y as u32, x as u32, 120));
                    }
                }
            }
            let page = if (t / 3) % 2 == 1 { dispcnt::PAGE1 } else { 0 };
            reg::BGHOFS[2].write(t16 * 2);
            reg::BGVOFS[2].write(t16);
            reg::DISPCNT.write(dispcnt::MODE5 | dispcnt::BG2 | page);
        }
        _ => {
            // Mode 4 again; sprites from tiles 0–511 are not drawn.
            for i in 0..8 {
                let tile = if i % 2 == 0 { 100 + i * 8 } else { 512 + i * 16 };
                let sp = Sprite::new(i as i32 * 30, 40 + t + (i % 3) as i32 * 20, SpriteSize::S32x32, tile as u16);
                oam::set(i, if i % 4 == 1 { sp.semi_transparent() } else { sp });
            }
            reg::BLDCNT.write(blend::cnt(blend::NONE, 0, layer::BG2));
            reg::BLDALPHA.write(blend::alpha(8, 8));
            reg::DISPCNT.write(dispcnt::MODE4 | dispcnt::BG2 | dispcnt::OBJ | dispcnt::OBJ_1D);
        }
    }
}
