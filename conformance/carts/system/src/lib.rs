//! Conformance: boot state, system registers, save RAM and the frame
//! sequence (SPEC.md §3, §5, §11).
//!
//! `start` checks what a host must leave in memory at boot. Every frame then
//! draws, as rows of squares on BG3: the boot checks, `FRAME`, `SEED`, all
//! four players' `KEYS` and `PADS`, `VCOUNT` during `update`, `SAVECNT` as the
//! host left it, save RAM, and whether every `scanline(y)` saw `VCOUNT` = y.
//! `scanline` also changes the backdrop per line.

#[path = "../../../common.rs"]
mod common;

use common::*;
use wasm_ad::*;

wasm_ad::cart! { title: "Conformance: system", author: "raskell.io" }

struct State {
    boot: u32,
    lines_ok: bool,
    lines_ok_last: bool,
}
static STATE: Global<State> = Global::new(State { boot: 0, lines_ok: true, lines_ok_last: false });

fn r16(a: usize) -> u16 {
    Reg16(a).read()
}

fn all_zero(from: usize, to: usize, skip: &[usize]) -> bool {
    (from..to).step_by(2).all(|a| skip.contains(&a) || r16(a) == 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    let oam_hidden = (0..128).all(|i| r16(addr::OAM + i * 8) == 0x0200 && r16(addr::OAM + i * 8 + 2) == 0 && r16(addr::OAM + i * 8 + 4) == 0)
        && (0..128).all(|i| r16(addr::OAM + i * 8 + 6) == 0);
    let matrices = [2, 3].iter().all(|&bg| {
        r16(addr::bgpa(bg)) == 0x100
            && r16(addr::bgpd(bg)) == 0x100
            && r16(addr::bgpb(bg)) == 0
            && r16(addr::bgpc(bg)) == 0
            && Reg32(addr::bgx(bg)).read() == 0
            && Reg32(addr::bgy(bg)).read() == 0
    });
    let affine = [addr::bgpa(2), addr::bgpd(2), addr::bgpa(3), addr::bgpd(3)];
    let flags = [
        oam_hidden,
        matrices,
        reg::SPEC.read() == 0x0003,
        all_zero(0x400, 0x460, &affine),
        all_zero(0x800, 0xC00, &[]) && all_zero(addr::VRAM, addr::VRAM + addr::VRAM_SIZE, &[]),
        all_zero(0x460, 0x700, &[]) && all_zero(addr::SOUND_STATE, 0x800, &[]),
        all_zero(addr::SAMPLE_RAM, addr::SAMPLE_RAM + addr::SAMPLE_RAM_SIZE, &[]),
        save_restored(),
        save_ram().iter().any(|&b| b != 0),
        all_zero(addr::KEYS, addr::KEYS + 2, &[]) && all_zero(addr::KEYS2, addr::PADS + 2, &[]),
        frame() == 0,
        reg::SAVECNT.read() == 0,
    ];
    STATE.get().boot = flags.iter().enumerate().map(|(i, &f)| (f as u32) << i).sum();
    let marks: String = flags.iter().map(|&f| if f { '1' } else { '0' }).collect();
    trace(&format!("boot checks {marks}"));

    indicator_setup();
    reg::BGCNT[3].write(BgCnt::new().char_block(3).screen_block(31).0);
    reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::BG3);
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let st = STATE.get();
    st.lines_ok_last = st.lines_ok;
    st.lines_ok = true;
    let f = frame();
    let saved = save_ram();
    indicator(1, 1, st.boot, 12);
    indicator(2, 1, f, 16);
    indicator(3, 1, seed() & 0xFFFF, 16);
    indicator(4, 1, seed() >> 16, 16);
    for p in 1..=4 {
        indicator(4 + p, 1, keys_of(p) as u32, 10);
    }
    indicator(9, 1, pads() as u32, 4);
    indicator(10, 1, reg::VCOUNT.read() as u32, 8);
    indicator(11, 1, reg::SAVECNT.read() as u32, 16);
    indicator(12, 1, u32::from(saved[0]) | u32::from(saved[1]) << 8, 16);
    let sum = saved.iter().fold(0u32, |a, &b| a.wrapping_mul(31).wrapping_add(b as u32));
    indicator(13, 1, sum & 0xFFFF, 16);
    indicator(14, 1, st.lines_ok_last as u32, 1);

    // Save RAM: write on some frames and ask the host to persist it; the
    // host clears SAVECNT bit 0 (upper bits are the cart's own).
    if f % 20 == 10 {
        saved[0] = saved[0].wrapping_add(1);
        saved[1] = (f & 0xFF) as u8;
        saved[100 + f as usize % 1000] ^= 0x5A;
        reg::SAVECNT.write(0x8000 | 1);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn scanline(y: i32) {
    let st = STATE.get();
    if reg::VCOUNT.read() as i32 != y {
        st.lines_ok = false;
    }
    let f = frame() as i32;
    bg_palette()[0] = rgb((y + f) as u8, (y * 2) as u8, (f * 3) as u8);
}
