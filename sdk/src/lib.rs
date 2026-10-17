//! Rust SDK for WASM-AD carts.
//!
//! A cart is a `cdylib` that exports `update` (and optionally `start` and
//! `scanline`) and drives the console by writing its memory-mapped hardware,
//! which this crate wraps in typed helpers. See SPEC.md for the hardware.
//!
//! ```ignore
//! wasm_ad::cart! { title: "Hello", author: "you" }
//!
//! #[unsafe(no_mangle)]
//! pub extern "C" fn update() {
//!     wasm_ad::reg::DISPCNT.write(wasm_ad::dispcnt::MODE0);
//! }
//! ```
//!
//! On wasm32 the hardware lives at fixed addresses in linear memory. On
//! other targets each thread gets an emulated copy, so cart logic can be
//! unit-tested natively and rendered with [`host`].

pub use wasm_ad_core::addr;
use wasm_ad_core::addr::*;

mod mem {
    #[cfg(target_arch = "wasm32")]
    pub fn ptr<T>(addr: usize) -> *mut T {
        std::ptr::with_exposed_provenance_mut(addr)
    }

    #[cfg(not(target_arch = "wasm32"))]
    thread_local! {
        pub static HW: std::cell::UnsafeCell<Vec<u8>> = std::cell::UnsafeCell::new(super::host::boot_image(0));
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn ptr<T>(addr: usize) -> *mut T {
        HW.with(|hw| unsafe { (*hw.get()).as_mut_ptr().add(addr).cast() })
    }
}

/// A view of `len` values of `T` at a hardware address.
///
/// # Safety
/// Callers must not hold two overlapping `&mut` views at once.
#[allow(clippy::mut_from_ref)]
unsafe fn view<T>(addr: usize, len: usize) -> &'static mut [T] {
    unsafe { std::slice::from_raw_parts_mut(mem::ptr::<T>(addr), len) }
}

/// A 16-bit hardware register.
#[derive(Clone, Copy)]
pub struct Reg16(pub usize);

impl Reg16 {
    pub fn read(self) -> u16 {
        unsafe { mem::ptr::<u16>(self.0).read_volatile() }
    }
    pub fn write(self, v: u16) {
        unsafe { mem::ptr::<u16>(self.0).write_volatile(v) }
    }
}

/// A 32-bit hardware register.
#[derive(Clone, Copy)]
pub struct Reg32(pub usize);

impl Reg32 {
    pub fn read(self) -> u32 {
        unsafe { mem::ptr::<u32>(self.0).read_volatile() }
    }
    pub fn write(self, v: u32) {
        unsafe { mem::ptr::<u32>(self.0).write_volatile(v) }
    }
}

/// Every register, by its SPEC.md name.
pub mod reg {
    use super::{Reg16, Reg32, addr};

    pub const DISPCNT: Reg16 = Reg16(addr::DISPCNT);
    pub const VCOUNT: Reg16 = Reg16(addr::VCOUNT);
    pub const BGCNT: [Reg16; 4] = [Reg16(addr::bgcnt(0)), Reg16(addr::bgcnt(1)), Reg16(addr::bgcnt(2)), Reg16(addr::bgcnt(3))];
    pub const BGHOFS: [Reg16; 4] =
        [Reg16(addr::bghofs(0)), Reg16(addr::bghofs(1)), Reg16(addr::bghofs(2)), Reg16(addr::bghofs(3))];
    pub const BGVOFS: [Reg16; 4] =
        [Reg16(addr::bgvofs(0)), Reg16(addr::bgvofs(1)), Reg16(addr::bgvofs(2)), Reg16(addr::bgvofs(3))];
    pub const WIN0H: Reg16 = Reg16(addr::WIN0H);
    pub const WIN1H: Reg16 = Reg16(addr::WIN1H);
    pub const WIN0V: Reg16 = Reg16(addr::WIN0V);
    pub const WIN1V: Reg16 = Reg16(addr::WIN1V);
    pub const WININ: Reg16 = Reg16(addr::WININ);
    pub const WINOUT: Reg16 = Reg16(addr::WINOUT);
    pub const MOSAIC: Reg16 = Reg16(addr::MOSAIC);
    pub const BLDCNT: Reg16 = Reg16(addr::BLDCNT);
    pub const BLDALPHA: Reg16 = Reg16(addr::BLDALPHA);
    pub const BLDY: Reg16 = Reg16(addr::BLDY);
    pub const KEYS: Reg16 = Reg16(addr::KEYS);
    pub const FRAME: Reg32 = Reg32(addr::FRAME);
    pub const SEED: Reg32 = Reg32(addr::SEED);
    pub const SAVECNT: Reg16 = Reg16(addr::SAVECNT);
    pub const SAVESTAT: Reg16 = Reg16(addr::SAVESTAT);
    pub const SPEC: Reg16 = Reg16(addr::SPEC);
}

/// `DISPCNT` bits (SPEC.md §6.1).
pub mod dispcnt {
    pub const MODE0: u16 = 0;
    pub const MODE3: u16 = 3;
    pub const MODE4: u16 = 4;
    pub const MODE5: u16 = 5;
    pub const PAGE1: u16 = 1 << 4;
    pub const OBJ_1D: u16 = 1 << 6;
    pub const FORCED_BLANK: u16 = 1 << 7;
    pub const BG0: u16 = 1 << 8;
    pub const BG1: u16 = 1 << 9;
    pub const BG2: u16 = 1 << 10;
    pub const BG3: u16 = 1 << 11;
    pub const OBJ: u16 = 1 << 12;
    pub const WIN0: u16 = 1 << 13;
    pub const WIN1: u16 = 1 << 14;
    pub const OBJ_WIN: u16 = 1 << 15;
}

/// Layer bits for `BLDCNT` targets and window masks.
pub mod layer {
    pub const BG0: u16 = 1 << 0;
    pub const BG1: u16 = 1 << 1;
    pub const BG2: u16 = 1 << 2;
    pub const BG3: u16 = 1 << 3;
    pub const OBJ: u16 = 1 << 4;
    pub const BACKDROP: u16 = 1 << 5;
    /// In window masks: color effects enabled.
    pub const EFFECTS: u16 = 1 << 5;
    pub const ALL: u16 = 0x3F;
}

/// `BLDCNT` effects (SPEC.md §6.5).
pub mod blend {
    pub const NONE: u16 = 0;
    pub const ALPHA: u16 = 1 << 6;
    pub const BRIGHTEN: u16 = 2 << 6;
    pub const DARKEN: u16 = 3 << 6;

    /// `BLDCNT` value: `effect` on `first` targets, blending onto `second`.
    pub const fn cnt(effect: u16, first: u16, second: u16) -> u16 {
        effect | first | (second << 8)
    }

    /// `BLDALPHA` value; coefficients 0–16.
    pub const fn alpha(eva: u16, evb: u16) -> u16 {
        eva | (evb << 8)
    }
}

/// Background map sizes, in tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapSize {
    S32x32 = 0,
    S64x32 = 1,
    S32x64 = 2,
    S64x64 = 3,
}

/// A `BGxCNT` value (SPEC.md §6.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BgCnt(pub u16);

impl BgCnt {
    pub const fn new() -> Self {
        Self(0)
    }
    pub const fn priority(self, p: u16) -> Self {
        Self((self.0 & !3) | (p & 3))
    }
    pub const fn char_block(self, b: u16) -> Self {
        Self((self.0 & !0xC) | ((b & 3) << 2))
    }
    pub const fn screen_block(self, b: u16) -> Self {
        Self((self.0 & !0x1F00) | ((b & 31) << 8))
    }
    pub const fn size(self, s: MapSize) -> Self {
        Self((self.0 & 0x3FFF) | ((s as u16) << 14))
    }
    pub const fn bpp8(self) -> Self {
        Self(self.0 | 0x80)
    }
    pub const fn mosaic(self) -> Self {
        Self(self.0 | 0x40)
    }
}

/// Packs 8-bit channels into RGB555.
pub const fn rgb(r: u8, g: u8, b: u8) -> u16 {
    (r as u16 >> 3) | ((g as u16 >> 3) << 5) | ((b as u16 >> 3) << 10)
}

/// The 256 background colors; 16 banks of 16 for 4bpp graphics.
pub fn bg_palette() -> &'static mut [u16] {
    unsafe { view(PAL_BG, 256) }
}

/// The 256 sprite colors.
pub fn obj_palette() -> &'static mut [u16] {
    unsafe { view(PAL_OBJ, 256) }
}

/// A 4bpp tile: 8 rows of 4 bytes, left pixel in the low nibble.
pub type Tile4 = [u8; 32];

/// All of VRAM as bytes.
pub fn vram() -> &'static mut [u8] {
    unsafe { view(VRAM, VRAM_SIZE) }
}

/// 4bpp background tiles from character block `block` (0–3) to the end of
/// background VRAM.
pub fn bg_tiles(block: usize) -> &'static mut [Tile4] {
    assert!(block < 4);
    unsafe { view(VRAM + block * 0x4000, (4 - block) * 512) }
}

/// The 1024 4bpp sprite tiles.
pub fn obj_tiles() -> &'static mut [Tile4] {
    unsafe { view(OBJ_VRAM, 1024) }
}

/// A background map entry (SPEC.md §8.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct ScreenEntry(pub u16);

impl ScreenEntry {
    pub const EMPTY: Self = Self(0);

    pub const fn new(tile: u16, bank: u16) -> Self {
        Self((tile & 0x3FF) | ((bank & 0xF) << 12))
    }
    pub const fn hflip(self) -> Self {
        Self(self.0 | 1 << 10)
    }
    pub const fn vflip(self) -> Self {
        Self(self.0 | 1 << 11)
    }
}

/// Screen block `block` (0–31): a 32×32 map, row-major.
pub fn screen_block(block: usize) -> &'static mut [ScreenEntry] {
    assert!(block < 32);
    unsafe { view(VRAM + block * 0x800, 1024) }
}

/// Sprite sizes as (shape, size) pairs (SPEC.md §9.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteSize {
    S8x8,
    S16x16,
    S32x32,
    S64x64,
    W16x8,
    W32x8,
    W32x16,
    W64x32,
    T8x16,
    T8x32,
    T16x32,
    T32x64,
}

impl SpriteSize {
    const fn bits(self) -> (u16, u16) {
        let i = self as u16;
        (i / 4, i % 4)
    }

    pub const fn dims(self) -> (i32, i32) {
        let (shape, size) = self.bits();
        match (shape, size) {
            (0, s) => (8 << s, 8 << s),
            (1, 0) => (16, 8),
            (1, 1) => (32, 8),
            (1, 2) => (32, 16),
            (1, _) => (64, 32),
            (_, 0) => (8, 16),
            (_, 1) => (8, 32),
            (_, 2) => (16, 32),
            _ => (32, 64),
        }
    }
}

/// One OAM entry's three attributes (SPEC.md §9.1), built fluently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sprite {
    pub attr0: u16,
    pub attr1: u16,
    pub attr2: u16,
}

impl Sprite {
    pub const HIDDEN: Self = Self { attr0: OBJ_HIDDEN, attr1: 0, attr2: 0 };

    /// A sprite with its top-left at screen (x, y), using `tile` and bank 0.
    pub const fn new(x: i32, y: i32, size: SpriteSize, tile: u16) -> Self {
        let (shape, sz) = size.bits();
        Self {
            attr0: ((y & 0xFF) as u16) | (shape << 14),
            attr1: ((x & 0x1FF) as u16) | (sz << 14),
            attr2: tile & 0x3FF,
        }
    }
    pub const fn bank(self, bank: u16) -> Self {
        Self { attr2: (self.attr2 & 0x0FFF) | ((bank & 0xF) << 12), ..self }
    }
    pub const fn priority(self, p: u16) -> Self {
        Self { attr2: (self.attr2 & !0x0C00) | ((p & 3) << 10), ..self }
    }
    pub const fn hflip(self, on: bool) -> Self {
        Self { attr1: if on { self.attr1 | 0x1000 } else { self.attr1 & !0x1000 }, ..self }
    }
    pub const fn vflip(self, on: bool) -> Self {
        Self { attr1: if on { self.attr1 | 0x2000 } else { self.attr1 & !0x2000 }, ..self }
    }
    pub const fn semi_transparent(self) -> Self {
        Self { attr0: (self.attr0 & !0x0C00) | 0x0400, ..self }
    }
    pub const fn window(self) -> Self {
        Self { attr0: (self.attr0 & !0x0C00) | 0x0800, ..self }
    }
    pub const fn mosaic(self) -> Self {
        Self { attr0: self.attr0 | 0x1000, ..self }
    }
    pub const fn bpp8(self) -> Self {
        Self { attr0: self.attr0 | 0x2000, ..self }
    }
    /// Uses affine matrix `slot` (0–31). With `double_size` the bounding box
    /// doubles so rotated or enlarged pixels are not clipped.
    pub const fn affine(self, slot: u16, double_size: bool) -> Self {
        let attr0 = self.attr0 | 0x0100 | if double_size { 0x0200 } else { 0 };
        let attr1 = (self.attr1 & !0x3E00) | ((slot & 31) << 9);
        Self { attr0, attr1, ..self }
    }
}

/// OAM: 128 sprites, plus affine matrices stored in the gaps.
pub mod oam {
    use super::*;

    pub fn set(i: usize, s: Sprite) {
        assert!(i < 128);
        let at = OAM + i * 8;
        Reg16(at).write(s.attr0);
        Reg16(at + 2).write(s.attr1);
        Reg16(at + 4).write(s.attr2);
    }

    pub fn hide(i: usize) {
        set(i, Sprite::HIDDEN);
    }

    pub fn hide_all() {
        (0..128).for_each(hide);
    }

    pub fn set_affine(slot: usize, m: AffineMatrix) {
        assert!(slot < 32);
        let at = OAM + slot * 32;
        for (i, v) in [m.pa, m.pb, m.pc, m.pd].into_iter().enumerate() {
            Reg16(at + 6 + i * 8).write(v as u16);
        }
    }
}

/// An affine matrix in 8.8 fixed point, mapping screen to texture space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineMatrix {
    pub pa: i16,
    pub pb: i16,
    pub pc: i16,
    pub pd: i16,
}

impl AffineMatrix {
    pub const IDENTITY: Self = Self { pa: 256, pb: 0, pc: 0, pd: 256 };

    /// Rotates counter-clockwise by `angle` radians and scales by (sx, sy).
    pub fn rotate_scale(angle: f32, sx: f32, sy: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        let fx = |v: f32| (v * 256.0).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        Self { pa: fx(cos / sx), pb: fx(-sin / sx), pc: fx(sin / sy), pd: fx(cos / sy) }
    }
}

/// Buttons held this frame.
pub fn keys() -> u16 {
    reg::KEYS.read()
}

pub use wasm_ad_core::addr::key;

/// Edge detection over `KEYS`; keep one in your game state and call
/// `update` at the start of every frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Buttons {
    held: u16,
    prev: u16,
}

impl Buttons {
    pub const fn new() -> Self {
        Self { held: 0, prev: 0 }
    }
    pub fn update(&mut self) {
        self.prev = self.held;
        self.held = keys();
    }
    pub fn held(&self, k: u16) -> bool {
        self.held & k != 0
    }
    /// True only on the frame the button went down.
    pub fn pressed(&self, k: u16) -> bool {
        self.held & !self.prev & k != 0
    }
}

pub fn frame() -> u32 {
    reg::FRAME.read()
}

/// The boot-time random seed, the cart's only entropy (SPEC.md §12).
pub fn seed() -> u32 {
    reg::SEED.read()
}

/// The 32 KB battery-backed save RAM.
pub fn save_ram() -> &'static mut [u8] {
    unsafe { view(SAVE, SAVE_SIZE) }
}

/// Whether the host restored save RAM from storage at boot.
pub fn save_restored() -> bool {
    reg::SAVESTAT.read() & 1 != 0
}

/// Asks the host to persist save RAM after this frame.
pub fn request_save() {
    reg::SAVECNT.write(reg::SAVECNT.read() | 1);
}

/// Writes a message to the host's debug log.
pub fn trace(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        #[link(wasm_import_module = "env")]
        unsafe extern "C" {
            #[link_name = "trace"]
            fn host_trace(ptr: *const u8, len: usize);
        }
        unsafe { host_trace(msg.as_ptr(), msg.len()) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("[cart] {msg}");
}

/// Declares the cart's metadata (SPEC.md §2.3). `title` is required; any
/// other `key: "value"` pairs are passed through. `spec` is filled in.
///
/// ```ignore
/// wasm_ad::cart! { title: "Rainbow River", author: "raskell.io", version: "0.1.0" }
/// ```
#[macro_export]
macro_rules! cart {
    (title: $title:literal $(, $key:ident : $value:literal)* $(,)?) => {
        #[cfg(target_arch = "wasm32")]
        const _: () = {
            const META: &str = concat!("spec=0.1\ntitle=", $title, "\n" $(, stringify!($key), "=", $value, "\n")*);
            #[unsafe(link_section = "wasm-ad")]
            #[used]
            static WASM_AD_META: [u8; META.len()] = $crate::__bytes::<{ META.len() }>(META);
        };
    };
}

#[doc(hidden)]
pub const fn __bytes<const N: usize>(s: &str) -> [u8; N] {
    let b = s.as_bytes();
    let mut out = [0u8; N];
    let mut i = 0;
    while i < N {
        out[i] = b[i];
        i += 1;
    }
    out
}

/// A native stand-in for the console, for tests: drives frames and renders
/// with the reference renderer. Each thread has its own hardware.
#[cfg(not(target_arch = "wasm32"))]
pub mod host {
    use super::*;

    /// Hardware memory as a host leaves it at boot (SPEC.md §3.1).
    pub(crate) fn boot_image(seed: u32) -> Vec<u8> {
        let mut hw = vec![0u8; CART];
        for i in 0..128 {
            hw[OAM + i * 8..][..2].copy_from_slice(&OBJ_HIDDEN.to_le_bytes());
        }
        hw[SEED..SEED + 4].copy_from_slice(&seed.to_le_bytes());
        hw[SPEC..SPEC + 2].copy_from_slice(&SPEC_VERSION.to_le_bytes());
        hw
    }

    /// Resets this thread's hardware to the boot state.
    pub fn boot(seed: u32) {
        mem::HW.with(|hw| unsafe { *hw.get() = boot_image(seed) });
    }

    /// Runs one frame: writes `KEYS`/`FRAME`/`VCOUNT` as the host would,
    /// then calls `update`.
    pub fn frame(frame: u32, keys: u16, update: impl FnOnce()) {
        reg::KEYS.write(keys);
        reg::FRAME.write(frame);
        reg::VCOUNT.write(160);
        update();
    }

    /// Renders the current hardware state (no scanline callback) as RGBA8.
    pub fn render() -> Vec<u32> {
        render_with_scanline(|_| {})
    }

    /// Renders like a host would for a cart exporting `scanline`: before each
    /// line, sets `VCOUNT` and calls `scanline(y)`.
    pub fn render_with_scanline(mut scanline: impl FnMut(i32)) -> Vec<u32> {
        let mut out = vec![0; SCREEN_W * SCREEN_H];
        for y in 0..SCREEN_H {
            reg::VCOUNT.write(y as u16);
            scanline(y as i32);
            let line = &mut out[y * SCREEN_W..(y + 1) * SCREEN_W];
            mem::HW.with(|hw| wasm_ad_core::render_line(unsafe { &*hw.get() }, y, line));
        }
        out
    }

    /// Writes an RGBA8 frame from [`render`] as a binary PPM image.
    pub fn write_ppm(path: impl AsRef<std::path::Path>, frame: &[u32]) -> std::io::Result<()> {
        let mut out = format!("P6 {SCREEN_W} {SCREEN_H} 255\n").into_bytes();
        for px in frame {
            out.extend(&px.to_le_bytes()[..3]);
        }
        std::fs::write(path, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_builder_matches_spec_bits() {
        let s = Sprite::new(-2, 150, SpriteSize::T16x32, 7).bank(3).priority(2).hflip(true);
        assert_eq!(s.attr0, 150 | 2 << 14);
        assert_eq!(s.attr1, 510 | 0x1000 | 2 << 14);
        assert_eq!(s.attr2, 7 | 2 << 10 | 3 << 12);
        assert_eq!(SpriteSize::W64x32.dims(), (64, 32));
        assert_eq!(SpriteSize::T8x32.dims(), (8, 32));
        let a = Sprite::new(0, 0, SpriteSize::S16x16, 0).affine(5, true);
        assert_eq!(a.attr0 & 0x300, 0x300);
        assert_eq!((a.attr1 >> 9) & 31, 5);
    }

    #[test]
    fn host_renders_what_the_cart_wrote() {
        host::boot(1);
        bg_palette()[0] = rgb(0, 0, 255);
        obj_palette()[1] = rgb(255, 0, 0);
        obj_tiles()[0] = [0x11; 32];
        oam::set(0, Sprite::new(10, 20, SpriteSize::S8x8, 0));
        reg::DISPCNT.write(dispcnt::OBJ | dispcnt::OBJ_1D);
        let fb = host::render();
        assert_eq!(fb[20 * SCREEN_W + 10], wasm_ad_core::rgba(rgb(255, 0, 0)));
        assert_eq!(fb[0], wasm_ad_core::rgba(rgb(0, 0, 255)));
    }

    #[test]
    fn buttons_detect_edges() {
        host::boot(0);
        let mut b = Buttons::default();
        host::frame(0, key::A, || b.update());
        assert!(b.pressed(key::A));
        host::frame(1, key::A, || b.update());
        assert!(b.held(key::A) && !b.pressed(key::A));
    }
}
