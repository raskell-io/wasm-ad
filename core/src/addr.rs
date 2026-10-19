//! The WASM-AD memory map and register addresses (SPEC.md §4–6).

pub const SCREEN_W: usize = 240;
pub const SCREEN_H: usize = 160;

pub const IO: usize = 0x00400;
pub const PAL_BG: usize = 0x00800;
pub const PAL_OBJ: usize = 0x00A00;
pub const OAM: usize = 0x00C00;
pub const VRAM: usize = 0x01000;
pub const VRAM_SIZE: usize = 0x18000;
/// Sprite tile 0.
pub const OBJ_VRAM: usize = VRAM + 0x10000;
pub const SAVE: usize = 0x19000;
pub const SAVE_SIZE: usize = 0x8000;
/// Sample RAM: signed 8-bit samples for the sample channels (SPEC.md §14.6).
pub const SAMPLE_RAM: usize = 0x21000;
pub const SAMPLE_RAM_SIZE: usize = 0x7000;
/// First address the cart may use for its own data, stack and heap.
pub const CART: usize = 0x28000;
/// OAM attribute 0 that hides a sprite; the host writes it to every entry at boot.
pub const OBJ_HIDDEN: u16 = 0x0200;
/// The renderer reads memory below this address.
pub const VIDEO_END: usize = SAVE;

// Display registers.
pub const DISPCNT: usize = 0x400;
pub const VCOUNT: usize = 0x406;
pub const fn bgcnt(bg: usize) -> usize {
    0x408 + bg * 2
}
pub const fn bghofs(bg: usize) -> usize {
    0x410 + bg * 4
}
pub const fn bgvofs(bg: usize) -> usize {
    0x412 + bg * 4
}
pub const WIN0H: usize = 0x440;
pub const WIN1H: usize = 0x442;
pub const WIN0V: usize = 0x444;
pub const WIN1V: usize = 0x446;
pub const WININ: usize = 0x448;
pub const WINOUT: usize = 0x44A;
pub const MOSAIC: usize = 0x44C;
pub const BLDCNT: usize = 0x450;
pub const BLDALPHA: usize = 0x452;
pub const BLDY: usize = 0x454;

// Affine background parameters (SPEC.md §6, §8.4), for BG2 and BG3.
pub const fn bgpa(bg: usize) -> usize {
    0x420 + (bg - 2) * 0x10
}
pub const fn bgpb(bg: usize) -> usize {
    bgpa(bg) + 2
}
pub const fn bgpc(bg: usize) -> usize {
    bgpa(bg) + 4
}
pub const fn bgpd(bg: usize) -> usize {
    bgpa(bg) + 6
}
pub const fn bgx(bg: usize) -> usize {
    bgpa(bg) + 8
}
pub const fn bgy(bg: usize) -> usize {
    bgpa(bg) + 12
}

// Sound registers (SPEC.md §14).
pub const SOUND1CNT_L: usize = 0x460;
pub const SOUND1CNT_H: usize = 0x462;
pub const SOUND1CNT_X: usize = 0x464;
pub const SOUND2CNT_L: usize = 0x468;
pub const SOUND2CNT_H: usize = 0x46C;
pub const SOUND3CNT_L: usize = 0x470;
pub const SOUND3CNT_H: usize = 0x472;
pub const SOUND3CNT_X: usize = 0x474;
pub const SOUND4CNT_L: usize = 0x478;
pub const SOUND4CNT_H: usize = 0x47C;
pub const SOUNDCNT_L: usize = 0x480;
pub const SOUNDCNT_H: usize = 0x482;
pub const SOUNDCNT_X: usize = 0x484;
/// Two banks of 32 4-bit samples, bank 1 at `WAVE_RAM + 16`.
pub const WAVE_RAM: usize = 0x490;
/// Sample channel A; channel B is at `PCM_B`. Register offsets below.
pub const PCM_A: usize = 0x4B0;
pub const PCM_B: usize = 0x4C0;
pub const PCM_SAD: usize = 0x0;
pub const PCM_LEN: usize = 0x2;
pub const PCM_LOOP: usize = 0x4;
pub const PCM_RATE: usize = 0x6;
pub const PCM_CNT: usize = 0x8;
pub const PCM_POS: usize = 0xA;
/// Host-owned sound state, so a snapshot of memory captures it; its layout
/// is fixed by SPEC.md §14.8.
pub const SOUND_STATE: usize = 0x780;
pub const SOUND_STATE_SIZE: usize = 0x80;
/// The sound registers, which the host reads and writes back each frame.
pub const SOUND_REGS: usize = 0x460;
pub const SOUND_REGS_END: usize = 0x4D0;

// System registers.
pub const KEYS: usize = 0x700;
pub const FRAME: usize = 0x704;
pub const SEED: usize = 0x708;
pub const SAVECNT: usize = 0x70C;
pub const SAVESTAT: usize = 0x70E;
pub const SPEC: usize = 0x710;
/// Buttons held by players 2–4 (`KEYS` is player 1).
pub const KEYS2: usize = 0x712;
pub const KEYS3: usize = 0x714;
pub const KEYS4: usize = 0x716;
/// Bits 0–3: players 1–4 have a controller.
pub const PADS: usize = 0x718;

/// Version this crate implements, as major × 256 + minor.
pub const SPEC_VERSION: u16 = 0x0003;

/// `KEYS` bits.
pub mod key {
    pub const A: u16 = 1 << 0;
    pub const B: u16 = 1 << 1;
    pub const SELECT: u16 = 1 << 2;
    pub const START: u16 = 1 << 3;
    pub const RIGHT: u16 = 1 << 4;
    pub const LEFT: u16 = 1 << 5;
    pub const UP: u16 = 1 << 6;
    pub const DOWN: u16 = 1 << 7;
    pub const R: u16 = 1 << 8;
    pub const L: u16 = 1 << 9;
}
