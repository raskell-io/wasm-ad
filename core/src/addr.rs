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
/// Reserved for audio (0.2).
pub const AUDIO: usize = 0x21000;
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

// System registers.
pub const KEYS: usize = 0x700;
pub const FRAME: usize = 0x704;
pub const SEED: usize = 0x708;
pub const SAVECNT: usize = 0x70C;
pub const SAVESTAT: usize = 0x70E;
pub const SPEC: usize = 0x710;

/// Version this crate implements, as major × 256 + minor.
pub const SPEC_VERSION: u16 = 0x0001;

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
