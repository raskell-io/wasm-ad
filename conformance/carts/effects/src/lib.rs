//! Conformance: windows and color effects (SPEC.md §6.3, §6.5, §9.1, §10).
//!
//! Scenes of 30 frames: 0 alpha blending with coefficients past 16; 1
//! brighten and darken; 2 WIN0 and WIN1, including wrap-around bounds; 3 the
//! sprite window; 4 semi-transparent sprites; 5 the backdrop as a target and
//! forced blank.

#[path = "../../../common.rs"]
mod common;

use common::*;
use wasm_ad::*;

wasm_ad::cart! { title: "Conformance: effects", author: "raskell.io" }

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    ramp(bg_palette());
    ramp(obj_palette());
    bg_palette()[0] = rgb(200, 60, 90);
    let t4 = bg_tiles(0);
    for i in 1..=16 {
        t4[i] = pattern4(i as u32);
        t4[i + 16] = holey4(i as u32 + 3);
    }
    let m0 = screen_block(28);
    let m1 = screen_block(29);
    for i in 0..1024 {
        let (c, r) = (i % 32, i / 32);
        m0[i] = if (c + r) % 3 == 0 { ScreenEntry::EMPTY } else { ScreenEntry::new(17 + (c % 16) as u16, 1) };
        m1[i] = ScreenEntry::new(1 + (r % 16) as u16, 2);
    }
    let ot = obj_tiles();
    for i in 0..64 {
        ot[i] = pattern4(i as u32 + 40);
    }
    // A sprite-window shape: a ring, in tiles 64–79.
    for y in 0..32i32 {
        for x in 0..32i32 {
            let d = (x * 2 - 31).pow(2) + (y * 2 - 31).pow(2);
            if (300..900).contains(&d) {
                let tile = 64 + (y / 8 * 4 + x / 8) as usize;
                ot[tile][((y % 8) * 4 + (x % 8) / 2) as usize] |= 1 << (4 * (x % 2));
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let (s, t) = scene();
    if t == 0 {
        reset_video();
    }
    let t16 = t as u16;
    reg::BGCNT[0].write(BgCnt::new().screen_block(28).priority(1).0);
    reg::BGCNT[1].write(BgCnt::new().screen_block(29).priority(2).0);
    reg::BGHOFS[0].write(t16 * 2);
    let base = dispcnt::MODE0 | dispcnt::BG0 | dispcnt::BG1 | dispcnt::OBJ | dispcnt::OBJ_1D;
    match s {
        0 => {
            reg::BLDCNT.write(blend::cnt(blend::ALPHA, layer::BG0, layer::BG1 | layer::BACKDROP));
            // EVA and EVB run past 16 (taken as 16); bits 5–7 and 13–15 are ignored.
            reg::BLDALPHA.write((t16 % 21) | ((20 - t16 % 21) << 8) | 0xE0E0);
            reg::DISPCNT.write(base);
        }
        1 => {
            let effect = if t % 2 == 0 { blend::BRIGHTEN } else { blend::DARKEN };
            reg::BLDCNT.write(blend::cnt(effect, layer::BG0 | layer::BACKDROP, 0));
            reg::BLDY.write(t16);
            reg::DISPCNT.write(base);
        }
        2 => {
            // WIN0 moves right and wraps (left > right); WIN1 wraps vertically.
            let l = (t16 * 9) % 240;
            reg::WIN0H.write((l << 8) | ((l + 100) % 240));
            reg::WIN0V.write((20 << 8) | 120);
            reg::WIN1H.write((40 << 8) | 200);
            reg::WIN1V.write(((130 + t16) << 8) | (30 + t16));
            reg::WININ.write((layer::BG0 | layer::EFFECTS) | ((layer::BG1 | layer::OBJ) << 8));
            reg::WINOUT.write(layer::BG1 | layer::EFFECTS);
            reg::BLDCNT.write(blend::cnt(blend::DARKEN, layer::BG0 | layer::BG1, 0));
            reg::BLDY.write(10);
            for i in 0..4 {
                oam::set(i, Sprite::new(i as i32 * 60 + t, 70, SpriteSize::S32x32, (i * 16) as u16));
            }
            reg::DISPCNT.write(base | dispcnt::WIN0 | dispcnt::WIN1);
        }
        3 => {
            // Rings shape the sprite window; WIN0 takes precedence once enabled.
            oam::set(0, Sprite::new(60 + t * 3, 40, SpriteSize::S32x32, 64).window());
            oam::set(1, Sprite::new(100, 60 + t, SpriteSize::S32x32, 64).window());
            oam::set(2, Sprite::new(80, 50, SpriteSize::S32x32, 0));
            reg::WIN0H.write((150 << 8) | 190);
            reg::WIN0V.write(160);
            reg::WININ.write(layer::BG1);
            reg::WINOUT.write(layer::BG1 | ((layer::BG0 | layer::OBJ | layer::EFFECTS) << 8));
            reg::BLDCNT.write(blend::cnt(blend::BRIGHTEN, layer::BG0, 0));
            reg::BLDY.write(8);
            let win0 = if t >= 15 { dispcnt::WIN0 } else { 0 };
            reg::DISPCNT.write(base | dispcnt::OBJ_WIN | win0);
        }
        4 => {
            // Semi-transparent sprites blend with a second target beneath them
            // whatever the effect; over anything else they are drawn plain.
            for i in 0..6 {
                let sp = Sprite::new(i as i32 * 38, 30 + t + i as i32 * 10, SpriteSize::S32x32, (i * 16 % 64) as u16)
                    .priority(i as u16 % 3);
                oam::set(i, if i % 2 == 0 { sp.semi_transparent() } else { sp });
            }
            let effect = if t < 15 { blend::BRIGHTEN } else { blend::NONE };
            reg::BLDCNT.write(blend::cnt(effect, layer::BG0, layer::BG1));
            reg::BLDALPHA.write(blend::alpha(9, 7));
            reg::BLDY.write(6);
            reg::DISPCNT.write(base);
        }
        _ => {
            // The backdrop as first target (it tops where BG0 has holes),
            // brightening; forced blank shows white every few frames.
            reg::BLDCNT.write(blend::cnt(blend::BRIGHTEN, layer::BACKDROP, 0));
            reg::BLDY.write(t16 % 17);
            reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG0 | if t % 10 < 3 { dispcnt::FORCED_BLANK } else { 0 });
        }
    }
}
