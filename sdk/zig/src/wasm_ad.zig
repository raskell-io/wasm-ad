//! Zig SDK for WASM-AD carts.
//!
//! A cart is a wasm32-freestanding executable that exports `update` (and
//! optionally `start` and `scanline`) and drives the console by writing its
//! memory-mapped hardware, which this module wraps in typed helpers. See
//! SPEC.md for the hardware.
//!
//!     const ad = @import("wasm_ad");
//!     comptime {
//!         ad.cart(.{ .title = "Hello", .author = "you" });
//!     }
//!     export fn update() void {
//!         ad.reg.DISPCNT.write(ad.dispcnt.MODE0);
//!     }
//!
//! Build carts with `addCart` from this package's build.zig, which sets the
//! linker options SPEC.md §13 requires.

const std = @import("std");
const builtin = @import("builtin");

/// Spec version this SDK targets.
pub const SPEC = "0.2";
pub const SPEC_VERSION: u16 = 0x0002;

pub const SCREEN_W = 240;
pub const SCREEN_H = 160;

/// The memory map (SPEC.md §4).
pub const addr = struct {
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
    /// Two banks of 32 4-bit samples, bank 1 at `WAVE_RAM + 16`.
    pub const WAVE_RAM: usize = 0x490;
    /// Sample channel A; channel B is at `PCM_B`.
    pub const PCM_A: usize = 0x4B0;
    pub const PCM_B: usize = 0x4C0;
    /// First address the cart may use for its own data, stack and heap.
    pub const CART: usize = 0x28000;
    /// OAM attribute 0 that hides a sprite.
    pub const OBJ_HIDDEN: u16 = 0x0200;
};

/// A 16-bit hardware register.
pub const Reg16 = struct {
    addr: usize,

    pub fn read(r: Reg16) u16 {
        return @as(*volatile u16, @ptrFromInt(r.addr)).*;
    }
    pub fn write(r: Reg16, v: u16) void {
        @as(*volatile u16, @ptrFromInt(r.addr)).* = v;
    }
};

/// A 32-bit hardware register.
pub const Reg32 = struct {
    addr: usize,

    pub fn read(r: Reg32) u32 {
        return @as(*volatile u32, @ptrFromInt(r.addr)).*;
    }
    pub fn write(r: Reg32, v: u32) void {
        @as(*volatile u32, @ptrFromInt(r.addr)).* = v;
    }
};

/// Every register, by its SPEC.md name.
pub const reg = struct {
    pub const DISPCNT: Reg16 = .{ .addr = 0x400 };
    pub const VCOUNT: Reg16 = .{ .addr = 0x406 };
    pub const BGCNT = [4]Reg16{ .{ .addr = 0x408 }, .{ .addr = 0x40A }, .{ .addr = 0x40C }, .{ .addr = 0x40E } };
    pub const BGHOFS = [4]Reg16{ .{ .addr = 0x410 }, .{ .addr = 0x414 }, .{ .addr = 0x418 }, .{ .addr = 0x41C } };
    pub const BGVOFS = [4]Reg16{ .{ .addr = 0x412 }, .{ .addr = 0x416 }, .{ .addr = 0x41A }, .{ .addr = 0x41E } };
    pub const WIN0H: Reg16 = .{ .addr = 0x440 };
    pub const WIN1H: Reg16 = .{ .addr = 0x442 };
    pub const WIN0V: Reg16 = .{ .addr = 0x444 };
    pub const WIN1V: Reg16 = .{ .addr = 0x446 };
    pub const WININ: Reg16 = .{ .addr = 0x448 };
    pub const WINOUT: Reg16 = .{ .addr = 0x44A };
    pub const MOSAIC: Reg16 = .{ .addr = 0x44C };
    pub const BLDCNT: Reg16 = .{ .addr = 0x450 };
    pub const BLDALPHA: Reg16 = .{ .addr = 0x452 };
    pub const BLDY: Reg16 = .{ .addr = 0x454 };
    pub const BG2PA: Reg16 = .{ .addr = 0x420 };
    pub const BG2PB: Reg16 = .{ .addr = 0x422 };
    pub const BG2PC: Reg16 = .{ .addr = 0x424 };
    pub const BG2PD: Reg16 = .{ .addr = 0x426 };
    pub const BG2X: Reg32 = .{ .addr = 0x428 };
    pub const BG2Y: Reg32 = .{ .addr = 0x42C };
    pub const BG3PA: Reg16 = .{ .addr = 0x430 };
    pub const BG3PB: Reg16 = .{ .addr = 0x432 };
    pub const BG3PC: Reg16 = .{ .addr = 0x434 };
    pub const BG3PD: Reg16 = .{ .addr = 0x436 };
    pub const BG3X: Reg32 = .{ .addr = 0x438 };
    pub const BG3Y: Reg32 = .{ .addr = 0x43C };
    pub const SOUND1CNT_L: Reg16 = .{ .addr = 0x460 };
    pub const SOUND1CNT_H: Reg16 = .{ .addr = 0x462 };
    pub const SOUND1CNT_X: Reg16 = .{ .addr = 0x464 };
    pub const SOUND2CNT_L: Reg16 = .{ .addr = 0x468 };
    pub const SOUND2CNT_H: Reg16 = .{ .addr = 0x46C };
    pub const SOUND3CNT_L: Reg16 = .{ .addr = 0x470 };
    pub const SOUND3CNT_H: Reg16 = .{ .addr = 0x472 };
    pub const SOUND3CNT_X: Reg16 = .{ .addr = 0x474 };
    pub const SOUND4CNT_L: Reg16 = .{ .addr = 0x478 };
    pub const SOUND4CNT_H: Reg16 = .{ .addr = 0x47C };
    pub const SOUNDCNT_L: Reg16 = .{ .addr = 0x480 };
    pub const SOUNDCNT_H: Reg16 = .{ .addr = 0x482 };
    pub const SOUNDCNT_X: Reg16 = .{ .addr = 0x484 };
    pub const PCMA_SAD: Reg16 = .{ .addr = 0x4B0 };
    pub const PCMA_LEN: Reg16 = .{ .addr = 0x4B2 };
    pub const PCMA_LOOP: Reg16 = .{ .addr = 0x4B4 };
    pub const PCMA_RATE: Reg16 = .{ .addr = 0x4B6 };
    pub const PCMA_CNT: Reg16 = .{ .addr = 0x4B8 };
    pub const PCMA_POS: Reg16 = .{ .addr = 0x4BA };
    pub const PCMB_SAD: Reg16 = .{ .addr = 0x4C0 };
    pub const PCMB_LEN: Reg16 = .{ .addr = 0x4C2 };
    pub const PCMB_LOOP: Reg16 = .{ .addr = 0x4C4 };
    pub const PCMB_RATE: Reg16 = .{ .addr = 0x4C6 };
    pub const PCMB_CNT: Reg16 = .{ .addr = 0x4C8 };
    pub const PCMB_POS: Reg16 = .{ .addr = 0x4CA };
    pub const KEYS: Reg16 = .{ .addr = 0x700 };
    pub const FRAME: Reg32 = .{ .addr = 0x704 };
    pub const SEED: Reg32 = .{ .addr = 0x708 };
    pub const SAVECNT: Reg16 = .{ .addr = 0x70C };
    pub const SAVESTAT: Reg16 = .{ .addr = 0x70E };
    pub const SPEC: Reg16 = .{ .addr = 0x710 };
};

/// `DISPCNT` bits (SPEC.md §6.1).
pub const dispcnt = struct {
    pub const MODE0: u16 = 0;
    /// BG0 and BG1 tiled, BG2 affine.
    pub const MODE1: u16 = 1;
    /// BG2 and BG3 affine.
    pub const MODE2: u16 = 2;
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
};

/// Layer bits for `BLDCNT` targets and window masks.
pub const layer = struct {
    pub const BG0: u16 = 1 << 0;
    pub const BG1: u16 = 1 << 1;
    pub const BG2: u16 = 1 << 2;
    pub const BG3: u16 = 1 << 3;
    pub const OBJ: u16 = 1 << 4;
    pub const BACKDROP: u16 = 1 << 5;
    /// In window masks: color effects enabled.
    pub const EFFECTS: u16 = 1 << 5;
    pub const ALL: u16 = 0x3F;
};

/// `BLDCNT` effects (SPEC.md §6.5).
pub const blend = struct {
    pub const NONE: u16 = 0;
    pub const ALPHA: u16 = 1 << 6;
    pub const BRIGHTEN: u16 = 2 << 6;
    pub const DARKEN: u16 = 3 << 6;

    /// `BLDCNT` value: `effect` on `first` targets, blending onto `second`.
    pub fn cnt(effect: u16, first: u16, second: u16) u16 {
        return effect | first | (second << 8);
    }

    /// `BLDALPHA` value; coefficients 0–16.
    pub fn alpha(eva: u16, evb: u16) u16 {
        return eva | (evb << 8);
    }
};

/// `KEYS` bits (SPEC.md §5).
pub const key = struct {
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
};

/// Background map sizes, in tiles.
pub const MapSize = enum(u16) { s32x32 = 0, s64x32 = 1, s32x64 = 2, s64x64 = 3 };

/// Affine background map sizes, in tiles (SPEC.md §8.4).
pub const AffineSize = enum(u16) { s16x16 = 0, s32x32 = 1, s64x64 = 2, s128x128 = 3 };

/// A `BGxCNT` value (SPEC.md §6.2).
pub const BgCnt = struct {
    bits: u16 = 0,

    pub fn priority(c: BgCnt, p: u16) BgCnt {
        return .{ .bits = (c.bits & ~@as(u16, 3)) | (p & 3) };
    }
    pub fn charBlock(c: BgCnt, b: u16) BgCnt {
        return .{ .bits = (c.bits & ~@as(u16, 0xC)) | ((b & 3) << 2) };
    }
    pub fn screenBlock(c: BgCnt, b: u16) BgCnt {
        return .{ .bits = (c.bits & ~@as(u16, 0x1F00)) | ((b & 31) << 8) };
    }
    pub fn size(c: BgCnt, s: MapSize) BgCnt {
        return .{ .bits = (c.bits & 0x3FFF) | (@intFromEnum(s) << 14) };
    }
    pub fn bpp8(c: BgCnt) BgCnt {
        return .{ .bits = c.bits | 0x80 };
    }
    pub fn mosaic(c: BgCnt) BgCnt {
        return .{ .bits = c.bits | 0x40 };
    }
    /// Map size of an affine background.
    pub fn affineSize(c: BgCnt, s: AffineSize) BgCnt {
        return .{ .bits = (c.bits & 0x3FFF) | (@intFromEnum(s) << 14) };
    }
    /// An affine background repeats instead of being transparent outside its map.
    pub fn wrap(c: BgCnt) BgCnt {
        return .{ .bits = c.bits | 0x2000 };
    }
};

/// Packs 8-bit channels into RGB555.
pub fn rgb(r: u8, g: u8, b: u8) u16 {
    return @as(u16, r >> 3) | (@as(u16, g >> 3) << 5) | (@as(u16, b >> 3) << 10);
}

/// The 256 background colors; 16 banks of 16 for 4bpp graphics.
pub fn bgPalette() *[256]u16 {
    return @ptrFromInt(addr.PAL_BG);
}

/// The 256 sprite colors.
pub fn objPalette() *[256]u16 {
    return @ptrFromInt(addr.PAL_OBJ);
}

/// A 4bpp tile: 8 rows of 4 bytes, left pixel in the low nibble.
pub const Tile4 = [32]u8;

/// All of VRAM as bytes.
pub fn vram() *[addr.VRAM_SIZE]u8 {
    return @ptrFromInt(addr.VRAM);
}

/// 4bpp background tiles from character block `block` (0–3) to the end of
/// background VRAM.
pub fn bgTiles(block: usize) []Tile4 {
    std.debug.assert(block < 4);
    const tiles: [*]Tile4 = @ptrFromInt(addr.VRAM + block * 0x4000);
    return tiles[0 .. (4 - block) * 512];
}

/// An 8bpp tile: 8 rows of 8 bytes. Affine backgrounds always use these.
pub const Tile8 = [64]u8;

/// 8bpp background tiles from character block `block` (0–3) to the end of
/// background VRAM.
pub fn bgTiles8(block: usize) []Tile8 {
    std.debug.assert(block < 4);
    const tiles: [*]Tile8 = @ptrFromInt(addr.VRAM + block * 0x4000);
    return tiles[0 .. (4 - block) * 256];
}

/// An affine background's map from screen block `block` (0–31) to the end of
/// background VRAM: one tile index per byte, row-major (SPEC.md §8.4).
pub fn affineMap(block: usize) []u8 {
    std.debug.assert(block < 32);
    const map: [*]u8 = @ptrFromInt(addr.VRAM + block * 0x800);
    return map[0 .. 0x10000 - block * 0x800];
}

/// The 1024 4bpp sprite tiles.
pub fn objTiles() *[1024]Tile4 {
    return @ptrFromInt(addr.OBJ_VRAM);
}

/// A background map entry (SPEC.md §8.2).
pub const ScreenEntry = extern struct {
    bits: u16 = 0,

    pub const EMPTY: ScreenEntry = .{};

    pub fn init(tile: u16, bank: u16) ScreenEntry {
        return .{ .bits = (tile & 0x3FF) | ((bank & 0xF) << 12) };
    }
    pub fn hflip(e: ScreenEntry) ScreenEntry {
        return .{ .bits = e.bits | 1 << 10 };
    }
    pub fn vflip(e: ScreenEntry) ScreenEntry {
        return .{ .bits = e.bits | 1 << 11 };
    }
};

/// Screen block `block` (0–31): a 32×32 map, row-major.
pub fn screenBlock(block: usize) *[1024]ScreenEntry {
    std.debug.assert(block < 32);
    return @ptrFromInt(addr.VRAM + block * 0x800);
}

/// Sprite sizes (SPEC.md §9.2); the value is shape × 4 + size.
pub const SpriteSize = enum(u16) {
    s8x8,
    s16x16,
    s32x32,
    s64x64,
    w16x8,
    w32x8,
    w32x16,
    w64x32,
    t8x16,
    t8x32,
    t16x32,
    t32x64,

    fn shape(s: SpriteSize) u16 {
        return @intFromEnum(s) / 4;
    }
    fn sizeBits(s: SpriteSize) u16 {
        return @intFromEnum(s) % 4;
    }

    /// Width and height in pixels.
    pub fn dims(s: SpriteSize) [2]i32 {
        return switch (s) {
            .s8x8 => .{ 8, 8 },
            .s16x16 => .{ 16, 16 },
            .s32x32 => .{ 32, 32 },
            .s64x64 => .{ 64, 64 },
            .w16x8 => .{ 16, 8 },
            .w32x8 => .{ 32, 8 },
            .w32x16 => .{ 32, 16 },
            .w64x32 => .{ 64, 32 },
            .t8x16 => .{ 8, 16 },
            .t8x32 => .{ 8, 32 },
            .t16x32 => .{ 16, 32 },
            .t32x64 => .{ 32, 64 },
        };
    }
};

/// One OAM entry's three attributes (SPEC.md §9.1), built fluently.
pub const Sprite = struct {
    attr0: u16,
    attr1: u16,
    attr2: u16,

    pub const HIDDEN: Sprite = .{ .attr0 = addr.OBJ_HIDDEN, .attr1 = 0, .attr2 = 0 };

    /// A sprite with its top-left at screen (x, y), using `tile` and bank 0.
    pub fn init(x: i32, y: i32, sz: SpriteSize, tile: u16) Sprite {
        return .{
            .attr0 = @as(u16, @intCast(y & 0xFF)) | (sz.shape() << 14),
            .attr1 = @as(u16, @intCast(x & 0x1FF)) | (sz.sizeBits() << 14),
            .attr2 = tile & 0x3FF,
        };
    }
    pub fn bank(s: Sprite, b: u16) Sprite {
        var r = s;
        r.attr2 = (s.attr2 & 0x0FFF) | ((b & 0xF) << 12);
        return r;
    }
    pub fn priority(s: Sprite, p: u16) Sprite {
        var r = s;
        r.attr2 = (s.attr2 & ~@as(u16, 0x0C00)) | ((p & 3) << 10);
        return r;
    }
    pub fn hflip(s: Sprite, on: bool) Sprite {
        var r = s;
        r.attr1 = if (on) s.attr1 | 0x1000 else s.attr1 & ~@as(u16, 0x1000);
        return r;
    }
    pub fn vflip(s: Sprite, on: bool) Sprite {
        var r = s;
        r.attr1 = if (on) s.attr1 | 0x2000 else s.attr1 & ~@as(u16, 0x2000);
        return r;
    }
    pub fn semiTransparent(s: Sprite) Sprite {
        var r = s;
        r.attr0 = (s.attr0 & ~@as(u16, 0x0C00)) | 0x0400;
        return r;
    }
    pub fn window(s: Sprite) Sprite {
        var r = s;
        r.attr0 = (s.attr0 & ~@as(u16, 0x0C00)) | 0x0800;
        return r;
    }
    pub fn mosaic(s: Sprite) Sprite {
        var r = s;
        r.attr0 = s.attr0 | 0x1000;
        return r;
    }
    pub fn bpp8(s: Sprite) Sprite {
        var r = s;
        r.attr0 = s.attr0 | 0x2000;
        return r;
    }
    /// Uses affine matrix `slot` (0–31). With `double_size` the bounding box
    /// doubles so rotated or enlarged pixels are not clipped.
    pub fn affine(s: Sprite, slot: u16, double_size: bool) Sprite {
        var r = s;
        r.attr0 = s.attr0 | 0x0100 | @as(u16, if (double_size) 0x0200 else 0);
        r.attr1 = (s.attr1 & ~@as(u16, 0x3E00)) | ((slot & 31) << 9);
        return r;
    }
};

/// An affine matrix in 8.8 fixed point, mapping screen to texture space.
pub const AffineMatrix = struct {
    pa: i16,
    pb: i16,
    pc: i16,
    pd: i16,

    pub const IDENTITY: AffineMatrix = .{ .pa = 256, .pb = 0, .pc = 0, .pd = 256 };

    /// Rotates counter-clockwise by `angle` radians and scales by (sx, sy).
    pub fn rotateScale(angle: f32, sx: f32, sy: f32) AffineMatrix {
        const s = @sin(angle);
        const c = @cos(angle);
        return .{ .pa = fx(c / sx), .pb = fx(-s / sx), .pc = fx(s / sy), .pd = fx(c / sy) };
    }

    fn fx(v: f32) i16 {
        return @intFromFloat(std.math.clamp(@round(v * 256.0), -32768.0, 32767.0));
    }

    /// Like `rotateScale` in integer math, so every SDK computes the same
    /// matrix: `angle` in 1/256 turns (see `fixed`), scale factors in 8.8
    /// fixed point (256 = 1×, must be nonzero).
    pub fn rotateScaleFixed(angle: i32, sx: i32, sy: i32) AffineMatrix {
        const s = fixed.sin(angle);
        const c = fixed.cos(angle);
        return .{
            .pa = clampI16(@divTrunc(c * 4, sx)),
            .pb = clampI16(@divTrunc(-s * 4, sx)),
            .pc = clampI16(@divTrunc(s * 4, sy)),
            .pd = clampI16(@divTrunc(c * 4, sy)),
        };
    }

    fn clampI16(v: i32) i16 {
        return @intCast(std.math.clamp(v, std.math.minInt(i16), std.math.maxInt(i16)));
    }
};

/// Sets affine background `bg` (2 or 3) to matrix `m`, placed so that
/// texture pixel (`tex_x`, `tex_y`) appears at screen pixel (`scr_x`, `scr_y`).
/// Rotation and scaling then happen around that point.
pub fn setBgAffine(bg: usize, m: AffineMatrix, tex_x: i32, tex_y: i32, scr_x: i32, scr_y: i32) void {
    std.debug.assert(bg == 2 or bg == 3);
    const at: usize = 0x420 + (bg - 2) * 0x10;
    const pa: i32 = m.pa;
    const pb: i32 = m.pb;
    const pc: i32 = m.pc;
    const pd: i32 = m.pd;
    (Reg16{ .addr = at }).write(@bitCast(m.pa));
    (Reg16{ .addr = at + 2 }).write(@bitCast(m.pb));
    (Reg16{ .addr = at + 4 }).write(@bitCast(m.pc));
    (Reg16{ .addr = at + 6 }).write(@bitCast(m.pd));
    (Reg32{ .addr = at + 8 }).write(@bitCast((tex_x << 8) -% pa *% scr_x -% pb *% scr_y));
    (Reg32{ .addr = at + 12 }).write(@bitCast((tex_y << 8) -% pc *% scr_x -% pd *% scr_y));
}

/// OAM: 128 sprites, plus affine matrices stored in the gaps.
pub const oam = struct {
    pub fn set(i: usize, s: Sprite) void {
        std.debug.assert(i < 128);
        const at = addr.OAM + i * 8;
        (Reg16{ .addr = at }).write(s.attr0);
        (Reg16{ .addr = at + 2 }).write(s.attr1);
        (Reg16{ .addr = at + 4 }).write(s.attr2);
    }

    pub fn hide(i: usize) void {
        set(i, Sprite.HIDDEN);
    }

    pub fn hideAll() void {
        for (0..128) |i| hide(i);
    }

    pub fn setAffine(slot: usize, m: AffineMatrix) void {
        std.debug.assert(slot < 32);
        const at = addr.OAM + slot * 32;
        for ([4]i16{ m.pa, m.pb, m.pc, m.pd }, 0..) |v, i| {
            (Reg16{ .addr = at + 6 + i * 8 }).write(@bitCast(v));
        }
    }
};

/// Fixed-point trigonometry shared by all WASM-AD SDKs, so carts in any
/// language compute bit-identical results (SPEC.md §12).
pub const fixed = struct {
    /// sin over a quarter turn in 1/256-turn steps, scaled by 16384 (Q14).
    pub const SIN_Q14 = [65]i32{
        0,     402,   804,   1205,  1606,  2006,  2404,  2801,  3196,  3590,  3981,  4370,  4756,  5139,  5520,  5897,  6270,
        6639,  7005,  7366,  7723,  8076,  8423,  8765,  9102,  9434,  9760,  10080, 10394, 10702, 11003, 11297, 11585, 11866,
        12140, 12406, 12665, 12916, 13160, 13395, 13623, 13842, 14053, 14256, 14449, 14635, 14811, 14978, 15137, 15286, 15426,
        15557, 15679, 15791, 15893, 15986, 16069, 16143, 16207, 16261, 16305, 16340, 16364, 16379, 16384,
    };

    /// sin of `angle` 1/256 turns (any integer; only the low 8 bits count), in Q14.
    pub fn sin(angle: i32) i32 {
        const a: usize = @intCast(angle & 255);
        return if (a < 64) SIN_Q14[a] else if (a < 128) SIN_Q14[128 - a] else if (a < 192) -SIN_Q14[a - 128] else -SIN_Q14[256 - a];
    }

    /// cos of `angle` 1/256 turns, in Q14.
    pub fn cos(angle: i32) i32 {
        return sin(angle +% 64);
    }
};

/// Sound (SPEC.md §14): four GBA-style channels written through
/// `reg.SOUND*`, with builders for their fields, and setup.
///
///     ad.sound.enable();
///     ad.reg.SOUND2CNT_L.write(ad.sound.DUTY_50 | ad.sound.envelope(12, 2, false));
///     ad.reg.SOUND2CNT_H.write(ad.sound.START | ad.sound.rate(440));
pub const sound = struct {
    /// Starts (or restarts) a channel; the host clears it.
    pub const START: u16 = 1 << 15;
    /// Stops the channel when its length runs out.
    pub const LENGTH: u16 = 1 << 14;

    pub const DUTY_12: u16 = 0 << 6;
    pub const DUTY_25: u16 = 1 << 6;
    pub const DUTY_50: u16 = 2 << 6;
    pub const DUTY_75: u16 = 3 << 6;

    /// `SOUND3CNT_L`: the wave channel is on.
    pub const WAVE_ON: u16 = 1 << 7;
    /// `SOUND3CNT_L`: play bank 1 (or start with it, with `WAVE_64`).
    pub const WAVE_BANK1: u16 = 1 << 6;
    /// `SOUND3CNT_L`: play both banks as one 64-sample wave.
    pub const WAVE_64: u16 = 1 << 5;
    /// `SOUND3CNT_H` volumes.
    pub const WAVE_100: u16 = 1 << 13;
    pub const WAVE_50: u16 = 2 << 13;
    pub const WAVE_25: u16 = 3 << 13;
    pub const WAVE_75: u16 = 1 << 15;

    /// Turns sound on with every channel on both sides at full volume.
    pub fn enable() void {
        reg.SOUNDCNT_X.write(0x80);
        reg.SOUNDCNT_L.write(0xFF77);
        reg.SOUNDCNT_H.write(0x330E);
    }

    /// Turns sound off; every channel stops.
    pub fn disable() void {
        reg.SOUNDCNT_X.write(0);
    }

    /// The rate field for a square channel playing `hz` (64 Hz and up); the
    /// wave channel plays an octave lower at the same rate.
    pub fn rate(hz: u32) u16 {
        return if (hz < 64) 0 else @intCast(2048 - 131072 / hz);
    }

    /// `SOUND1CNT_L`: every `time` 128ths of a second (1–7, 0 = off), the
    /// rate changes by rate >> `shift` (1–7), down if `decrease`.
    pub fn sweep(time: u16, shift: u16, decrease: bool) u16 {
        return ((time & 7) << 4) | (shift & 7) | @as(u16, if (decrease) 8 else 0);
    }

    /// The envelope fields: start at `volume` (0–15), stepping by one every
    /// `step` 64ths of a second (0 = constant), up if `increase`.
    pub fn envelope(volume: u16, step: u16, increase: bool) u16 {
        return ((volume & 15) << 12) | ((step & 7) << 8) | @as(u16, if (increase) 1 << 11 else 0);
    }

    /// The length field of channels 1, 2 and 4 for `n` 256ths of a second
    /// (1–64); it only counts with `LENGTH` set. Channel 3 takes 256 − n.
    pub fn length(n: u16) u16 {
        return (64 -% n) & 63;
    }

    /// `SOUND4CNT_H`: noise from a divider (0–7) and shift (0–13), with the
    /// 7-bit register if `short` (more tonal).
    pub fn noise(divider: u16, shift: u16, short: bool) u16 {
        return (divider & 7) | ((shift & 15) << 4) | @as(u16, if (short) 8 else 0);
    }

    /// Wave RAM: two banks of 16 bytes, each 32 4-bit samples, high nibble first.
    pub fn waveRam() *[32]u8 {
        return @ptrFromInt(addr.WAVE_RAM);
    }

    /// The 28 KB of sample RAM that the sample channels play from.
    pub fn sampleRam() *[addr.SAMPLE_RAM_SIZE]i8 {
        return @ptrFromInt(addr.SAMPLE_RAM);
    }

    /// Which channels are playing: bits 0–3 channels 1–4, bits 4 and 5 sample
    /// channels A and B, as of the end of the last frame.
    pub fn status() u16 {
        return reg.SOUNDCNT_X.read() & 0x3F;
    }
};

/// The two sample channels, A (0) and B (1), playing signed 8-bit samples
/// from `sound.sampleRam()` (SPEC.md §14.6).
pub const pcm = struct {
    fn base(ch: usize) usize {
        std.debug.assert(ch < 2);
        return if (ch == 0) addr.PCM_A else addr.PCM_B;
    }

    /// Plays `len` samples from sample RAM offset `start` at `rate` Hz and
    /// `volume` 0–64, once.
    pub fn play(ch: usize, start: u16, len: u16, rate: u16, volume: u16) void {
        const b = base(ch);
        (Reg16{ .addr = b }).write(start);
        (Reg16{ .addr = b + 2 }).write(len);
        (Reg16{ .addr = b + 6 }).write(rate);
        (Reg16{ .addr = b + 8 }).write(sound.START | @min(volume, 64));
    }

    /// Like `play`, then repeats from sample `loop_at` until stopped.
    pub fn playLooped(ch: usize, start: u16, len: u16, loop_at: u16, rate: u16, volume: u16) void {
        (Reg16{ .addr = base(ch) + 4 }).write(loop_at);
        play(ch, start, len, rate, volume);
        const cnt = Reg16{ .addr = base(ch) + 8 };
        cnt.write(cnt.read() | 1 << 13);
    }

    pub fn stop(ch: usize) void {
        (Reg16{ .addr = base(ch) + 8 }).write(1 << 14);
    }

    /// The playback position, in samples from the start, as of the end of the last frame.
    pub fn position(ch: usize) u16 {
        return (Reg16{ .addr = base(ch) + 0xA }).read();
    }

    pub fn playing(ch: usize) bool {
        return sound.status() & (@as(u16, 1) << @intCast(4 + ch)) != 0;
    }
};

/// Buttons held this frame.
pub fn keys() u16 {
    return reg.KEYS.read();
}

/// Edge detection over `KEYS`; keep one in your game state and call
/// `update` at the start of every frame.
pub const Buttons = struct {
    now: u16 = 0,
    prev: u16 = 0,

    pub fn update(b: *Buttons) void {
        b.prev = b.now;
        b.now = keys();
    }
    pub fn held(b: Buttons, k: u16) bool {
        return b.now & k != 0;
    }
    /// True only on the frame the button went down.
    pub fn pressed(b: Buttons, k: u16) bool {
        return b.now & ~b.prev & k != 0;
    }
};

pub fn frame() u32 {
    return reg.FRAME.read();
}

/// The boot-time random seed, the cart's only entropy (SPEC.md §12).
pub fn seed() u32 {
    return reg.SEED.read();
}

/// The 32 KB battery-backed save RAM.
pub fn saveRam() *[addr.SAVE_SIZE]u8 {
    return @ptrFromInt(addr.SAVE);
}

/// Whether the host restored save RAM from storage at boot.
pub fn saveRestored() bool {
    return reg.SAVESTAT.read() & 1 != 0;
}

/// Asks the host to persist save RAM after this frame.
pub fn requestSave() void {
    reg.SAVECNT.write(reg.SAVECNT.read() | 1);
}

const host = struct {
    extern "env" fn trace(ptr: [*]const u8, len: usize) void;
};

/// Writes a message to the host's debug log.
pub fn trace(msg: []const u8) void {
    if (comptime builtin.cpu.arch.isWasm()) {
        host.trace(msg.ptr, msg.len);
    } else {
        std.debug.print("[cart] {s}\n", .{msg});
    }
}

/// Declares the cart's metadata (SPEC.md §2.3) as the `wasm-ad` custom
/// section. Call it from a container-level `comptime` block. `title` is
/// required; every other field is passed through as `key=value`; `spec` is
/// filled in.
///
///     comptime {
///         ad.cart(.{ .title = "Rainbow River", .author = "raskell.io", .version = "0.1.0" });
///     }
pub fn cart(comptime meta: anytype) void {
    _ = Metadata(meta);
}

fn Metadata(comptime meta: anytype) type {
    if (!@hasField(@TypeOf(meta), "title")) @compileError("wasm_ad.cart: .title is required");
    return struct {
        // LLVM's wasm assembler turns `.custom_section.NAME` into a custom section.
        comptime {
            asm (".section \".custom_section.wasm-ad\",\"\",@\n.ascii \"" ++ metaText(meta) ++ "\"\n");
        }
    };
}

/// The metadata as `key=value` lines, escaped for an assembler string.
fn metaText(comptime meta: anytype) []const u8 {
    comptime var out: []const u8 = "spec=" ++ SPEC ++ "\\n";
    inline for (@typeInfo(@TypeOf(meta)).@"struct".fields) |f| {
        const value: []const u8 = @field(meta, f.name);
        out = out ++ f.name ++ "=" ++ comptime escape(f.name, value) ++ "\\n";
    }
    return out;
}

fn escape(comptime name: []const u8, comptime value: []const u8) []const u8 {
    comptime var out: []const u8 = "";
    inline for (value) |c| {
        out = out ++ switch (c) {
            '\n', '\r' => @compileError("wasm_ad.cart: ." ++ name ++ " must be one line"),
            '"' => "\\\"",
            '\\' => "\\\\",
            else => &[_]u8{c},
        };
    }
    return out;
}

test "sprite builder matches spec bits" {
    const s = Sprite.init(-2, 150, .t16x32, 7).bank(3).priority(2).hflip(true);
    try std.testing.expectEqual(@as(u16, 150 | 2 << 14), s.attr0);
    try std.testing.expectEqual(@as(u16, 510 | 0x1000 | 2 << 14), s.attr1);
    try std.testing.expectEqual(@as(u16, 7 | 2 << 10 | 3 << 12), s.attr2);
    try std.testing.expectEqual([2]i32{ 64, 32 }, SpriteSize.w64x32.dims());
    const a = Sprite.init(0, 0, .s16x16, 0).affine(5, true);
    try std.testing.expectEqual(@as(u16, 0x300), a.attr0 & 0x300);
    try std.testing.expectEqual(@as(u16, 5), (a.attr1 >> 9) & 31);
}

test "fixed trig matches float" {
    var a: i32 = 0;
    while (a < 256) : (a += 1) {
        const t = @as(f64, @floatFromInt(a)) * std.math.tau / 256.0;
        try std.testing.expect(@abs(@as(f64, @floatFromInt(fixed.sin(a))) / 16384.0 - @sin(t)) < 1e-4);
        try std.testing.expect(@abs(@as(f64, @floatFromInt(fixed.cos(a))) / 16384.0 - @cos(t)) < 1e-4);
    }
    try std.testing.expectEqual(@as(i32, -16384), fixed.sin(-64));
    try std.testing.expectEqual(AffineMatrix.IDENTITY, AffineMatrix.rotateScaleFixed(0, 256, 256));
    try std.testing.expectEqual(AffineMatrix{ .pa = 0, .pb = -128, .pc = 256, .pd = 0 }, AffineMatrix.rotateScaleFixed(64, 512, 256));
}

test "metadata text" {
    try std.testing.expectEqualStrings("spec=0.2\\ntitle=A \\\"B\\\"\\nauthor=me\\n", metaText(.{ .title = "A \"B\"", .author = "me" }));
}

test "sound field builders match spec bits" {
    try std.testing.expectEqual(@as(u16, 1751), sound.rate(440));
    try std.testing.expectEqual(@as(u16, 0), sound.rate(63));
    try std.testing.expectEqual(@as(u16, 0x2A), sound.sweep(2, 2, true));
    try std.testing.expectEqual(@as(u16, 0xC200), sound.envelope(12, 2, false));
    try std.testing.expectEqual(@as(u16, 0x0900), sound.envelope(0, 1, true));
    try std.testing.expectEqual(@as(u16, 0), sound.length(64));
    try std.testing.expectEqual(@as(u16, 63), sound.length(1));
    try std.testing.expectEqual(@as(u16, 0x1B), sound.noise(3, 1, true));
}

test "affine background control bits" {
    const c = (BgCnt{}).screenBlock(8).affineSize(.s32x32).wrap();
    try std.testing.expectEqual(@as(u16, 8 << 8 | 0x2000 | 1 << 14), c.bits);
}
