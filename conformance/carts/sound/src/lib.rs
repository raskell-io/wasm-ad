//! Conformance: the four PSG channels and mixing (SPEC.md §14.1–14.5, §14.7).
//!
//! Scenes of 30 frames: 0 square duties and a rising envelope with a live
//! rate change; 1 sweep up to overflow, sweep down, length; 2 retriggering
//! and envelope steps; 3 the wave channel, one bank and two, every volume,
//! the DAC switched off; 4 noise, 15- and 7-bit, divider 0 and a stopped
//! shift; 5 routing, master and PSG volumes, sound switched off and on.
//! Each frame draws `SOUNDCNT_X` bits 0–7 so status is in the frame hash.

#[path = "../../../common.rs"]
mod common;

use common::*;
use wasm_ad::*;

wasm_ad::cart! { title: "Conformance: sound", author: "raskell.io" }

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    indicator_setup();
    reg::BGCNT[3].write(BgCnt::new().char_block(3).screen_block(31).0);
    reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG3);
    // Bank 0: a ramp; bank 1: a square.
    let w = sound::wave_ram();
    for i in 0..16 {
        w[i] = ((i as u8) << 4) | (15 - i as u8);
        w[16 + i] = if i < 8 { 0xFF } else { 0x00 };
    }
    sound::enable();
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let (s, t) = scene();
    indicator(1, 1, reg::SOUNDCNT_X.read() as u32, 8);
    indicator(2, 1, frame(), 8);
    let start = sound::START;
    match s {
        0 => {
            if t % 8 == 0 {
                reg::SOUND1CNT_L.write(0);
                reg::SOUND1CNT_H.write(((t / 8) as u16 % 4) << 6 | sound::envelope(2, 1, true));
                reg::SOUND1CNT_X.write(start | sound::rate(330));
            }
            // Rate changes without a restart take effect at the next step.
            if t % 8 == 4 {
                reg::SOUND1CNT_X.write(sound::rate(440));
            }
            reg::SOUND2CNT_L.write(sound::DUTY_12 | sound::envelope(6, 0, false));
            if t == 0 {
                reg::SOUND2CNT_H.write(start | sound::rate(1000));
            }
        }
        1 => {
            reg::SOUND2CNT_H.write(0);
            match t {
                0 => {
                    reg::SOUND1CNT_L.write(sound::sweep(1, 2, false));
                    reg::SOUND1CNT_H.write(sound::DUTY_50 | sound::envelope(12, 0, false));
                    reg::SOUND1CNT_X.write(start | sound::rate(300));
                }
                12 => {
                    reg::SOUND1CNT_L.write(sound::sweep(2, 3, true));
                    reg::SOUND1CNT_X.write(start | sound::rate(2000));
                }
                20 => {
                    // Length 64 − 40 = 24 clocks: under 6 frames.
                    reg::SOUND1CNT_L.write(0);
                    reg::SOUND1CNT_H.write(sound::DUTY_25 | 40 | sound::envelope(10, 0, false));
                    reg::SOUND1CNT_X.write(start | sound::LENGTH | sound::rate(500));
                }
                _ => {}
            }
        }
        2 => {
            if t % 5 == 0 {
                reg::SOUND2CNT_L.write(sound::DUTY_75 | sound::envelope(15, (t / 5 % 8) as u16, t % 10 == 0));
                reg::SOUND2CNT_H.write(start | sound::rate(200 + t as u32 * 20));
            }
        }
        3 => {
            let sel = match t / 6 {
                0 => sound::WAVE_ON,
                1 => sound::WAVE_ON | sound::WAVE_BANK1,
                2 => sound::WAVE_ON | sound::WAVE_64,
                3 => sound::WAVE_ON | sound::WAVE_64 | sound::WAVE_BANK1,
                _ => 0, // DAC off: stops the channel
            };
            reg::SOUND3CNT_L.write(sel);
            let vol = [sound::WAVE_100, sound::WAVE_50, sound::WAVE_25, sound::WAVE_75, 0, sound::WAVE_100][(t % 6) as usize];
            reg::SOUND3CNT_H.write(vol | (256 - 100));
            if t % 6 == 0 {
                reg::SOUND3CNT_X.write(start | sound::LENGTH | sound::rate(150 + t as u32 * 10));
            }
        }
        4 => {
            reg::SOUND3CNT_L.write(0);
            if t % 6 == 0 {
                let n = t as u16 / 6;
                let shift = [0, 3, 6, 14, 2][n as usize];
                reg::SOUND4CNT_L.write(sound::envelope(12, 1, false));
                reg::SOUND4CNT_H.write(start | sound::noise(n % 3 * 3 % 8, shift, n % 2 == 1));
            }
        }
        _ => {
            if t == 0 {
                reg::SOUND1CNT_L.write(0);
                reg::SOUND1CNT_H.write(sound::DUTY_50 | sound::envelope(15, 0, false));
                reg::SOUND1CNT_X.write(start | sound::rate(262));
                reg::SOUND2CNT_L.write(sound::DUTY_25 | sound::envelope(15, 0, false));
                reg::SOUND2CNT_H.write(start | sound::rate(392));
                reg::SOUND4CNT_L.write(sound::envelope(8, 0, false));
                reg::SOUND4CNT_H.write(start | sound::noise(2, 4, false));
            }
            // Routing, side volumes and the PSG volume cycle; 3 counts as 100%.
            reg::SOUNDCNT_L.write(((t as u16 * 0x1357) & 0xFF00) | (t as u16 % 8) | ((7 - t as u16 % 8) << 4));
            reg::SOUNDCNT_H.write((reg::SOUNDCNT_H.read() & !3) | (t as u16 / 4 % 4));
            // Off for frames 20–23: channels stop, and starts are ignored.
            if t == 20 {
                sound::disable();
            }
            if t == 22 {
                reg::SOUND2CNT_H.write(start | sound::rate(392));
            }
            if t == 24 {
                sound::enable();
                reg::SOUND1CNT_X.write(start | sound::rate(330));
            }
        }
    }
}
