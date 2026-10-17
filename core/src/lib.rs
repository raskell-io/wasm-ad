//! Reference renderer for WASM-AD, written to follow SPEC.md section by section.
//!
//! The renderer is a pure function of memory: give it the bottom
//! `VIDEO_END` bytes of a cart's linear memory and a line number, and it
//! returns that line's pixels. Hosts call it once per line (SPEC.md §3.2).

pub mod addr;

use addr::*;

/// The size of a sprite by shape (square, wide, tall) and size (SPEC.md §9.2).
const SPRITE_SIZES: [[(i32, i32); 4]; 3] = [
    [(8, 8), (16, 16), (32, 32), (64, 64)],
    [(16, 8), (32, 8), (32, 16), (64, 32)],
    [(8, 16), (8, 32), (16, 32), (32, 64)],
];

/// Layer bits, as in BLDCNT and the window masks.
const OBJ_BIT: u8 = 1 << 4;
const BACKDROP_BIT: u8 = 1 << 5;
const EFFECTS_BIT: u8 = 1 << 5;

fn r16(mem: &[u8], a: usize) -> u16 {
    u16::from_le_bytes([mem[a], mem[a + 1]])
}

fn nibble(byte: u8, x: usize) -> u8 {
    if x % 2 == 0 { byte & 0xF } else { byte >> 4 }
}

/// Converts RGB555 to RGBA8 packed as 0xAABBGGRR (bytes R, G, B, A in memory).
pub fn rgba(c: u16) -> u32 {
    let expand = |v: u16| ((v << 3) | (v >> 2)) as u32;
    let (r, g, b) = (expand(c & 31), expand((c >> 5) & 31), expand((c >> 10) & 31));
    0xFF00_0000 | (b << 16) | (g << 8) | r
}

/// Renders a whole frame without scanline callbacks.
pub fn render_frame(mem: &[u8], out: &mut [u32]) {
    for y in 0..SCREEN_H {
        render_line(mem, y, &mut out[y * SCREEN_W..(y + 1) * SCREEN_W]);
    }
}

/// Renders line `y` of the frame described by `mem` into `out` (240 pixels, RGBA8).
pub fn render_line(mem: &[u8], y: usize, out: &mut [u32]) {
    assert!(mem.len() >= VIDEO_END, "memory must cover the video region");
    assert!(y < SCREEN_H && out.len() >= SCREEN_W);

    let dispcnt = r16(mem, DISPCNT);
    let mode = dispcnt & 7;
    if dispcnt & 0x80 != 0 || !matches!(mode, 0 | 3 | 4 | 5) {
        out[..SCREEN_W].fill(0xFFFF_FFFF);
        return;
    }

    let mut objs = [ObjPixel::NONE; SCREEN_W];
    let mut obj_window = [false; SCREEN_W];
    if dispcnt & 0x1000 != 0 {
        sprite_line(mem, y, &mut objs, &mut obj_window);
    }

    let windows = Windows::new(mem, dispcnt, y);
    let bgs: [Option<Background>; 4] = std::array::from_fn(|i| {
        let exists = if mode == 0 { true } else { i == 2 };
        (exists && dispcnt & (0x100 << i) != 0).then(|| Background::new(mem, i))
    });

    let bldcnt = r16(mem, BLDCNT);
    let effect = (bldcnt >> 6) & 3;
    let first = (bldcnt & 0x3F) as u8;
    let second = ((bldcnt >> 8) & 0x3F) as u8;
    let bldalpha = r16(mem, BLDALPHA);
    let eva = (bldalpha & 31).min(16);
    let evb = ((bldalpha >> 8) & 31).min(16);
    let evy = (r16(mem, BLDY) & 31).min(16);
    let backdrop = r16(mem, PAL_BG) & 0x7FFF;

    for x in 0..SCREEN_W {
        let mask = windows.mask(x, obj_window[x]);

        // §10.1–10.2: candidates in tie-break order; strict `<` keeps the
        // earlier candidate on equal priority.
        let backdrop = Hit { layer: BACKDROP_BIT, color: backdrop, priority: 4, semi: false };
        let (mut top, mut beneath) = (backdrop, backdrop);
        let mut consider = |hit: Hit| {
            if hit.priority < top.priority {
                beneath = top;
                top = hit;
            } else if hit.priority < beneath.priority {
                beneath = hit;
            }
        };
        let obj = objs[x];
        if obj.opaque && mask & OBJ_BIT != 0 {
            consider(Hit { layer: OBJ_BIT, color: obj.color, priority: obj.priority, semi: obj.semi });
        }
        for (i, bg) in bgs.iter().enumerate() {
            if let Some(bg) = bg {
                if mask & (1 << i) != 0 {
                    if let Some(color) = bg.sample(mem, x, y, mode, dispcnt) {
                        consider(Hit { layer: 1 << i, color, priority: bg.priority, semi: false });
                    }
                }
            }
        }

        // §10.3
        let blend = || per_channel(top.color, beneath.color, |i, b| ((i * eva + b * evb) >> 4).min(31));
        let color = if mask & EFFECTS_BIT == 0 {
            top.color
        } else if top.semi && second & beneath.layer != 0 {
            blend()
        } else if first & top.layer != 0 {
            match effect {
                1 if second & beneath.layer != 0 => blend(),
                2 => per_channel(top.color, 0, |i, _| i + (((31 - i) * evy) >> 4)),
                3 => per_channel(top.color, 0, |i, _| i - ((i * evy) >> 4)),
                _ => top.color,
            }
        } else {
            top.color
        };
        out[x] = rgba(color);
    }
}

fn per_channel(a: u16, b: u16, f: impl Fn(u16, u16) -> u16) -> u16 {
    [0, 5, 10].iter().fold(0, |out, &s| out | (f((a >> s) & 31, (b >> s) & 31) << s))
}

#[derive(Clone, Copy)]
struct Hit {
    layer: u8,
    color: u16,
    priority: u8,
    semi: bool,
}

#[derive(Clone, Copy)]
struct ObjPixel {
    color: u16,
    priority: u8,
    semi: bool,
    opaque: bool,
}

impl ObjPixel {
    const NONE: Self = Self { color: 0, priority: 4, semi: false, opaque: false };
}

/// §6.3
struct Windows {
    any: bool,
    win0: Option<(u8, u8)>,
    win1: Option<(u8, u8)>,
    obj: bool,
    winin: u16,
    winout: u16,
}

fn inside(lo: u8, hi: u8, v: u8) -> bool {
    if lo <= hi { v >= lo && v < hi } else { v >= lo || v < hi }
}

impl Windows {
    fn new(mem: &[u8], dispcnt: u16, y: usize) -> Self {
        let y = y as u8;
        let window = |enable: u16, h: usize, v: usize| {
            let (h, v) = (r16(mem, h), r16(mem, v));
            (dispcnt & enable != 0 && inside((v >> 8) as u8, v as u8, y)).then_some(((h >> 8) as u8, h as u8))
        };
        Self {
            any: dispcnt & 0xE000 != 0,
            win0: window(0x2000, WIN0H, WIN0V),
            win1: window(0x4000, WIN1H, WIN1V),
            obj: dispcnt & 0x8000 != 0,
            winin: r16(mem, WININ),
            winout: r16(mem, WINOUT),
        }
    }

    /// The layer/effect mask governing pixel `x`.
    fn mask(&self, x: usize, in_obj_window: bool) -> u8 {
        let x = x as u8;
        let bits = if !self.any {
            0x3F
        } else if self.win0.is_some_and(|(l, r)| inside(l, r, x)) {
            self.winin
        } else if self.win1.is_some_and(|(l, r)| inside(l, r, x)) {
            self.winin >> 8
        } else if self.obj && in_obj_window {
            self.winout >> 8
        } else {
            self.winout
        };
        (bits & 0x3F) as u8
    }
}

/// §6.2, §8
struct Background {
    priority: u8,
    char_base: usize,
    screen_base: usize,
    bpp8: bool,
    size: u16,
    mosaic: Option<(usize, usize)>,
    hofs: usize,
    vofs: usize,
}

impl Background {
    fn new(mem: &[u8], i: usize) -> Self {
        let cnt = r16(mem, bgcnt(i));
        let m = r16(mem, MOSAIC);
        Self {
            priority: (cnt & 3) as u8,
            char_base: ((cnt >> 2) & 3) as usize * 0x4000,
            screen_base: ((cnt >> 8) & 31) as usize * 0x800,
            bpp8: cnt & 0x80 != 0,
            size: cnt >> 14,
            mosaic: (cnt & 0x40 != 0).then_some(((m & 15) as usize + 1, ((m >> 4) & 15) as usize + 1)),
            hofs: (r16(mem, bghofs(i)) & 0x1FF) as usize,
            vofs: (r16(mem, bgvofs(i)) & 0x1FF) as usize,
        }
    }

    fn sample(&self, mem: &[u8], x: usize, y: usize, mode: u16, dispcnt: u16) -> Option<u16> {
        let (x, y) = match self.mosaic {
            Some((h, v)) => (x - x % h, y - y % v),
            None => (x, y),
        };
        let page = if dispcnt & 0x10 != 0 { 0xA000 } else { 0 };
        match mode {
            0 => self.sample_text(mem, x, y),
            3 => {
                let (px, py) = (x + self.hofs, y + self.vofs);
                (px < 240 && py < 160).then(|| r16(mem, VRAM + (py * 240 + px) * 2) & 0x7FFF)
            }
            4 => {
                let (px, py) = (x + self.hofs, y + self.vofs);
                if px >= 240 || py >= 160 {
                    return None;
                }
                match mem[VRAM + page + py * 240 + px] {
                    0 => None,
                    idx => Some(r16(mem, PAL_BG + idx as usize * 2) & 0x7FFF),
                }
            }
            5 => {
                let (px, py) = (x + self.hofs, y + self.vofs);
                (px < 160 && py < 128).then(|| r16(mem, VRAM + page + (py * 160 + px) * 2) & 0x7FFF)
            }
            _ => None,
        }
    }

    fn sample_text(&self, mem: &[u8], x: usize, y: usize) -> Option<u16> {
        let (cols, rows) = match self.size {
            0 => (32, 32),
            1 => (64, 32),
            2 => (32, 64),
            _ => (64, 64),
        };
        let px = (x + self.hofs) % (cols * 8);
        let py = (y + self.vofs) % (rows * 8);
        let (tx, ty) = (px / 8, py / 8);
        let block = match self.size {
            0 => 0,
            1 => tx / 32,
            2 => ty / 32,
            _ => (ty / 32) * 2 + tx / 32,
        };
        let entry_at = self.screen_base + block * 0x800 + ((ty % 32) * 32 + tx % 32) * 2;
        if entry_at + 1 >= VRAM_SIZE {
            return None;
        }
        let entry = r16(mem, VRAM + entry_at);
        let ix = if entry & 0x400 != 0 { 7 - px % 8 } else { px % 8 };
        let iy = if entry & 0x800 != 0 { 7 - py % 8 } else { py % 8 };
        let tile = (entry & 0x3FF) as usize;
        let index = if self.bpp8 {
            let at = self.char_base + tile * 64;
            if at >= 0x10000 {
                return None;
            }
            mem[VRAM + at + iy * 8 + ix] as usize
        } else {
            let at = self.char_base + tile * 32;
            if at >= 0x10000 {
                return None;
            }
            let bank = (entry >> 12) as usize;
            match nibble(mem[VRAM + at + iy * 4 + ix / 2], ix) {
                0 => 0,
                i => bank * 16 + i as usize,
            }
        };
        (index != 0).then(|| r16(mem, PAL_BG + index * 2) & 0x7FFF)
    }
}

/// Rasterizes every sprite touching line `y` (SPEC.md §9).
fn sprite_line(mem: &[u8], y: usize, objs: &mut [ObjPixel; SCREEN_W], obj_window: &mut [bool; SCREEN_W]) {
    let dispcnt = r16(mem, DISPCNT);
    let one_d = dispcnt & 0x40 != 0;
    let bitmap = dispcnt & 7 != 0;
    let m = r16(mem, MOSAIC);
    let (mh, mv) = (((m >> 8) & 15) as i32 + 1, ((m >> 12) & 15) as i32 + 1);

    for i in 0..128 {
        let at = OAM + i * 8;
        let (a0, a1, a2) = (r16(mem, at), r16(mem, at + 2), r16(mem, at + 4));
        let affine = a0 & 0x100 != 0;
        let bit9 = a0 & 0x200 != 0;
        let mode = (a0 >> 10) & 3;
        let shape = (a0 >> 14) as usize;
        if (!affine && bit9) || mode == 3 || shape == 3 {
            continue;
        }
        let (w, h) = SPRITE_SIZES[shape][(a1 >> 14) as usize];
        let (bw, bh) = if affine && bit9 { (w * 2, h * 2) } else { (w, h) };
        let mut top = (a0 & 0xFF) as i32;
        if top >= 160 {
            top -= 256;
        }
        let mut left = (a1 & 0x1FF) as i32;
        if left >= 240 {
            left -= 512;
        }
        let ly = y as i32 - top;
        if !(0..bh).contains(&ly) {
            continue;
        }
        let tile = (a2 & 0x3FF) as i32;
        if bitmap && tile < 512 {
            continue;
        }
        let priority = ((a2 >> 10) & 3) as u8;
        let bank = (a2 >> 12) as usize;
        let bpp8 = a0 & 0x2000 != 0;
        let mosaic = a0 & 0x1000 != 0;
        let matrix = affine.then(|| {
            let m = OAM + ((a1 >> 9) & 31) as usize * 32;
            [6, 14, 22, 30].map(|o| r16(mem, m + o) as i16 as i32)
        });
        let vy = if mosaic { ly - ly % mv } else { ly };
        let step = if bpp8 { 2 } else { 1 };

        for bx in 0..bw {
            let lx = left + bx;
            if !(0..SCREEN_W as i32).contains(&lx) {
                continue;
            }
            let lx = lx as usize;
            if mode != 2 && objs[lx].opaque && objs[lx].priority <= priority {
                continue;
            }
            let ux = if mosaic { bx - bx % mh } else { bx };
            let (u, v) = match matrix {
                Some([pa, pb, pc, pd]) => {
                    let (dx, dy) = (ux - bw / 2, vy - bh / 2);
                    let u = ((pa * dx + pb * dy) >> 8) + w / 2;
                    let v = ((pc * dx + pd * dy) >> 8) + h / 2;
                    if !(0..w).contains(&u) || !(0..h).contains(&v) {
                        continue;
                    }
                    (u, v)
                }
                None => (
                    if a1 & 0x1000 != 0 { w - 1 - ux } else { ux },
                    if a1 & 0x2000 != 0 { h - 1 - vy } else { vy },
                ),
            };
            let (c, r) = (u / 8, v / 8);
            let t = if one_d { tile + (r * (w / 8) + c) * step } else { tile + r * 32 + c * step };
            let base = OBJ_VRAM + (t as usize % 1024) * 32;
            let (u, v) = ((u % 8) as usize, (v % 8) as usize);
            let index = if bpp8 {
                let a = base + v * 8 + u;
                if a >= VIDEO_END { 0 } else { mem[a] as usize }
            } else {
                match nibble(mem[base + v * 4 + u / 2], u) {
                    0 => 0,
                    i => bank * 16 + i as usize,
                }
            };
            if index == 0 {
                continue;
            }
            if mode == 2 {
                obj_window[lx] = true;
                continue;
            }
            objs[lx] = ObjPixel {
                color: r16(mem, PAL_OBJ + index * 2) & 0x7FFF,
                priority,
                semi: mode == 1,
                opaque: true,
            };
        }
    }
}

/// The C ABI used by the web player (player/web): it copies the cart's
/// video memory into `ad_mem`, then calls `ad_render_line` per line.
#[cfg(feature = "player")]
mod player {
    use super::*;
    use std::cell::UnsafeCell;

    struct Shared<T>(UnsafeCell<T>);
    // SAFETY: wasm32-unknown-unknown is single-threaded.
    unsafe impl<T> Sync for Shared<T> {}

    static MEM: Shared<[u8; VIDEO_END]> = Shared(UnsafeCell::new([0; VIDEO_END]));
    static FB: Shared<[u32; SCREEN_W * SCREEN_H]> = Shared(UnsafeCell::new([0; SCREEN_W * SCREEN_H]));

    #[unsafe(no_mangle)]
    pub extern "C" fn ad_mem() -> *mut u8 {
        MEM.0.get().cast()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ad_mem_len() -> u32 {
        VIDEO_END as u32
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ad_framebuffer() -> *const u8 {
        FB.0.get().cast()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ad_render_line(y: u32) {
        let y = y as usize;
        // SAFETY: single-threaded; no references outlive this call.
        let (mem, fb) = unsafe { (&*MEM.0.get(), &mut *FB.0.get()) };
        render_line(mem, y, &mut fb[y * SCREEN_W..(y + 1) * SCREEN_W]);
    }
}

#[cfg(test)]
mod tests;
