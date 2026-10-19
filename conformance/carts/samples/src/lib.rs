//! Conformance: sample channels and sample RAM (SPEC.md §14.6, §14.7).
//!
//! Scenes of 30 frames: 0 one-shots at several rates and volumes, on both
//! sides and at 50%; 1 a loop with a moving loop point, a pitch slide and the
//! stop bit; 2 streaming: a ring buffer that wraps past the end of sample RAM,
//! refilled behind `PCMx_POS` every frame; 3 edge cases: length 0, a loop
//! point past the end, rate 0, start and stop together, volume above 64.
//! Each frame draws `SOUNDCNT_X` and both positions, so they are in the hash.

#[path = "../../../common.rs"]
mod common;

use common::*;
use wasm_ad::*;

wasm_ad::cart! { title: "Conformance: samples", author: "raskell.io" }

/// Ring buffer for scene 2: 4096 samples from offset 0x6800, wrapping at 0x7000.
const RING: u16 = 0x6800;
const RING_LEN: u16 = 4096;

struct State {
    last_pos: u32,
    played: u32,
    written: u32,
}
static STATE: Global<State> = Global::new(State { last_pos: 0, played: 0, written: 0 });

fn sample_at(i: usize) -> &'static mut i8 {
    &mut sound::sample_ram()[i % addr::SAMPLE_RAM_SIZE]
}

/// The streamed signal: a sine whose pitch rises with each sample.
fn stream_value(n: u32) -> i8 {
    ((fixed::sin(((n * (4 + n / 512)) >> 4) as i32) * 120) >> 14) as i8
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    indicator_setup();
    reg::BGCNT[3].write(BgCnt::new().char_block(3).screen_block(31).0);
    reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG3);
    let ram = sound::sample_ram();
    // 0x0000: a saw, 2000 samples. 0x1000: a sine, 1000. 0x2000: noise, 3000.
    for i in 0..2000 {
        ram[i] = ((i * 7) % 256) as u8 as i8;
    }
    for i in 0..1000 {
        ram[0x1000 + i] = ((fixed::sin(i as i32 * 3) * 127) >> 14) as i8;
    }
    let mut x = 0xACE1u32;
    for i in 0..3000 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        ram[0x2000 + i] = x as i8;
    }
    sound::enable();
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let (s, t) = scene();
    indicator(1, 1, reg::SOUNDCNT_X.read() as u32, 8);
    indicator(2, 1, pcm::position(0) as u32, 16);
    indicator(3, 1, pcm::position(1) as u32, 16);
    match s {
        0 => {
            let rates = [8000, 22050, 48000, 65535, 11025];
            if t % 6 == 0 {
                let k = (t / 6) as usize;
                pcm::play(0, 0, 2000, rates[k], 16 * k as u16 + 10);
                pcm::play(1, 0x1000, 1000, rates[4 - k], 64);
            }
            // B on the left only at 50%, A on both sides at 100%.
            reg::SOUNDCNT_H.write(0x0002 | 0x0004 | 0x0300 | 0x2000);
        }
        1 => {
            reg::SOUNDCNT_H.write(0x330E);
            if t == 0 {
                pcm::play_looped(0, 0x1000, 1000, 250, 8000, 64);
                pcm::play_looped(1, 0x2000, 3000, 2999, 30000, 40);
            }
            reg::PCMA_RATE.write(8000 + t as u16 * 900);
            reg::PCMA_LOOP.write(250 + t as u16 * 20);
            reg::PCMA_CNT.write((reg::PCMA_CNT.read() & !0x7F) | (64 - t as u16 * 2));
            if t == 25 {
                pcm::stop(1);
            }
        }
        2 => {
            let st = STATE.get();
            if t == 0 {
                pcm::stop(1);
                for n in 0..RING_LEN as u32 {
                    *sample_at(RING as usize + n as usize) = stream_value(n);
                }
                *st = State { last_pos: 0, played: 0, written: RING_LEN as u32 };
                pcm::play_looped(0, RING, RING_LEN, 0, 24000, 64);
            } else {
                // Count what has played since last frame, then refill the ring
                // behind the playback position.
                let ring = RING_LEN as u32;
                let pos = pcm::position(0) as u32;
                st.played += (pos + ring - st.last_pos) % ring;
                st.last_pos = pos;
                while st.written < st.played + ring {
                    *sample_at(RING as usize + (st.written % RING_LEN as u32) as usize) = stream_value(st.written);
                    st.written += 1;
                }
            }
        }
        _ => match t {
            0 => {
                pcm::stop(0);
                pcm::play(1, 0, 0, 8000, 64); // length 0: stops at once
            }
            5 => {
                pcm::play_looped(0, 0x1000, 200, 300, 48000, 64); // loop past the end: stops
            }
            10 => {
                pcm::play(1, 0x1000, 500, 0, 64); // rate 0: holds its first sample
            }
            15 => {
                // Start and stop in one frame: the stop goes first, so it plays.
                pcm::play(0, 0, 2000, 16000, 64);
                reg::PCMA_CNT.write(reg::PCMA_CNT.read() | 0x4000);
            }
            20 => {
                pcm::play(1, 0x2000, 3000, 20000, 0);
                reg::PCMB_CNT.write(0x8000 | 0x7F); // volume 127 counts as 64
            }
            _ => {}
        },
    }
}
