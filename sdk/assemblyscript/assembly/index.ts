// AssemblyScript SDK for WASM-AD carts.
//
// A cart exports `update` (and optionally `start` and `scanline`) and drives
// the console by writing its memory-mapped hardware, which this module wraps
// in typed helpers. See SPEC.md for the hardware.
//
// AssemblyScript keeps module-level `let`/`var` in WebAssembly globals, but a
// cart's state must live in linear memory (SPEC.md §12). So this SDK has no
// mutable globals, its values are plain integers (no allocation), and carts
// keep their own state in a `@unmanaged` class placed with `memory.data`:
//
//   @unmanaged class State { x: i32; y: i32 }
//   const state = changetype<State>(memory.data(offsetof<State>()));

// ---- Memory map (SPEC.md §4) ------------------------------------------------

export const SCREEN_W: i32 = 240;
export const SCREEN_H: i32 = 160;

export const IO: usize = 0x00400;
export const PAL_BG: usize = 0x00800;
export const PAL_OBJ: usize = 0x00a00;
export const OAM: usize = 0x00c00;
export const VRAM: usize = 0x01000;
export const VRAM_SIZE: usize = 0x18000;
/** Sprite tile 0. */
export const OBJ_VRAM: usize = VRAM + 0x10000;
export const SAVE: usize = 0x19000;
export const SAVE_SIZE: usize = 0x8000;
/** Sample RAM: signed 8-bit samples for the sample channels (SPEC.md §14.6). */
export const SAMPLE_RAM: usize = 0x21000;
export const SAMPLE_RAM_SIZE: usize = 0x7000;
/** Wave RAM: two banks of 16 bytes (SPEC.md §14.5). */
export const WAVE_RAM: usize = 0x490;
/** First address the cart may use for its own data, stack and heap. */
export const CART: usize = 0x28000;
/** OAM attribute 0 that hides a sprite. */
export const OBJ_HIDDEN: u16 = 0x0200;
export const SPEC_VERSION: u16 = 0x0002;

// ---- Registers (SPEC.md §5, §6, §14) ----------------------------------------

/** Every register address, by its SPEC.md name. Use with `read16`/`write16`. */
export namespace reg {
  export const DISPCNT: usize = 0x400;
  export const VCOUNT: usize = 0x406;
  export const WIN0H: usize = 0x440;
  export const WIN1H: usize = 0x442;
  export const WIN0V: usize = 0x444;
  export const WIN1V: usize = 0x446;
  export const WININ: usize = 0x448;
  export const WINOUT: usize = 0x44a;
  export const MOSAIC: usize = 0x44c;
  export const BLDCNT: usize = 0x450;
  export const BLDALPHA: usize = 0x452;
  export const BLDY: usize = 0x454;
  export const KEYS: usize = 0x700;
  export const FRAME: usize = 0x704; // u32
  export const SEED: usize = 0x708; // u32
  export const SAVECNT: usize = 0x70c;
  export const SAVESTAT: usize = 0x70e;
  export const SPEC: usize = 0x710;

  export const BG2PA: usize = 0x420;
  export const BG2PB: usize = 0x422;
  export const BG2PC: usize = 0x424;
  export const BG2PD: usize = 0x426;
  export const BG2X: usize = 0x428; // u32
  export const BG2Y: usize = 0x42c; // u32
  export const BG3PA: usize = 0x430;
  export const BG3PB: usize = 0x432;
  export const BG3PC: usize = 0x434;
  export const BG3PD: usize = 0x436;
  export const BG3X: usize = 0x438; // u32
  export const BG3Y: usize = 0x43c; // u32

  export const SOUND1CNT_L: usize = 0x460;
  export const SOUND1CNT_H: usize = 0x462;
  export const SOUND1CNT_X: usize = 0x464;
  export const SOUND2CNT_L: usize = 0x468;
  export const SOUND2CNT_H: usize = 0x46c;
  export const SOUND3CNT_L: usize = 0x470;
  export const SOUND3CNT_H: usize = 0x472;
  export const SOUND3CNT_X: usize = 0x474;
  export const SOUND4CNT_L: usize = 0x478;
  export const SOUND4CNT_H: usize = 0x47c;
  export const SOUNDCNT_L: usize = 0x480;
  export const SOUNDCNT_H: usize = 0x482;
  export const SOUNDCNT_X: usize = 0x484;

  export const PCMA_SAD: usize = 0x4b0;
  export const PCMA_LEN: usize = 0x4b2;
  export const PCMA_LOOP: usize = 0x4b4;
  export const PCMA_RATE: usize = 0x4b6;
  export const PCMA_CNT: usize = 0x4b8;
  export const PCMA_POS: usize = 0x4ba;
  export const PCMB_SAD: usize = 0x4c0;
  export const PCMB_LEN: usize = 0x4c2;
  export const PCMB_LOOP: usize = 0x4c4;
  export const PCMB_RATE: usize = 0x4c6;
  export const PCMB_CNT: usize = 0x4c8;
  export const PCMB_POS: usize = 0x4ca;

  /** `BGxCNT` for background 0–3. */
  // @ts-ignore: decorator
  @inline export function BGCNT(bg: i32): usize {
    return 0x408 + (<usize>bg << 1);
  }
  /** `BGxHOFS` for background 0–3. */
  // @ts-ignore: decorator
  @inline export function BGHOFS(bg: i32): usize {
    return 0x410 + (<usize>bg << 2);
  }
  /** `BGxVOFS` for background 0–3. */
  // @ts-ignore: decorator
  @inline export function BGVOFS(bg: i32): usize {
    return 0x412 + (<usize>bg << 2);
  }
}

// @ts-ignore: decorator
@inline export function read16(addr: usize): u16 {
  return load<u16>(addr);
}
// @ts-ignore: decorator
@inline export function write16(addr: usize, v: u16): void {
  store<u16>(addr, v);
}
// @ts-ignore: decorator
@inline export function read32(addr: usize): u32 {
  return load<u32>(addr);
}
// @ts-ignore: decorator
@inline export function write32(addr: usize, v: u32): void {
  store<u32>(addr, v);
}

/** `DISPCNT` bits (SPEC.md §6.1). */
export namespace dispcnt {
  export const MODE0: u16 = 0;
  /** BG0 and BG1 tiled, BG2 affine. */
  export const MODE1: u16 = 1;
  /** BG2 and BG3 affine. */
  export const MODE2: u16 = 2;
  export const MODE3: u16 = 3;
  export const MODE4: u16 = 4;
  export const MODE5: u16 = 5;
  export const PAGE1: u16 = 1 << 4;
  export const OBJ_1D: u16 = 1 << 6;
  export const FORCED_BLANK: u16 = 1 << 7;
  export const BG0: u16 = 1 << 8;
  export const BG1: u16 = 1 << 9;
  export const BG2: u16 = 1 << 10;
  export const BG3: u16 = 1 << 11;
  export const OBJ: u16 = 1 << 12;
  export const WIN0: u16 = 1 << 13;
  export const WIN1: u16 = 1 << 14;
  export const OBJ_WIN: u16 = 1 << 15;
}

/** Layer bits for `BLDCNT` targets and window masks. */
export namespace layer {
  export const BG0: u16 = 1 << 0;
  export const BG1: u16 = 1 << 1;
  export const BG2: u16 = 1 << 2;
  export const BG3: u16 = 1 << 3;
  export const OBJ: u16 = 1 << 4;
  export const BACKDROP: u16 = 1 << 5;
  /** In window masks: color effects enabled. */
  export const EFFECTS: u16 = 1 << 5;
  export const ALL: u16 = 0x3f;
}

/** `BLDCNT` effects (SPEC.md §6.5). */
export namespace blend {
  export const NONE: u16 = 0;
  export const ALPHA: u16 = 1 << 6;
  export const BRIGHTEN: u16 = 2 << 6;
  export const DARKEN: u16 = 3 << 6;

  /** `BLDCNT` value: `effect` on `first` targets, blending onto `second`. */
  // @ts-ignore: decorator
  @inline export function cnt(effect: u16, first: u16, second: u16): u16 {
    return effect | first | (second << 8);
  }
  /** `BLDALPHA` value; coefficients 0–16. */
  // @ts-ignore: decorator
  @inline export function alpha(eva: u16, evb: u16): u16 {
    return eva | (evb << 8);
  }
}

/** `KEYS` bits (SPEC.md §5). */
export namespace key {
  export const A: u16 = 1 << 0;
  export const B: u16 = 1 << 1;
  export const SELECT: u16 = 1 << 2;
  export const START: u16 = 1 << 3;
  export const RIGHT: u16 = 1 << 4;
  export const LEFT: u16 = 1 << 5;
  export const UP: u16 = 1 << 6;
  export const DOWN: u16 = 1 << 7;
  export const R: u16 = 1 << 8;
  export const L: u16 = 1 << 9;
}

// ---- Backgrounds (SPEC.md §6.2, §8) -----------------------------------------

/** Background map sizes, in tiles. */
export enum MapSize {
  S32x32 = 0,
  S64x32 = 1,
  S32x64 = 2,
  S64x64 = 3,
}

/** Affine background map sizes, in tiles (SPEC.md §8.4). */
export enum AffineSize {
  S16x16 = 0,
  S32x32 = 1,
  S64x64 = 2,
  S128x128 = 3,
}

/** Builders for a `BGxCNT` value (SPEC.md §6.2), e.g. `bgcnt.priority(bgcnt.screenBlock(0, 31), 1)`. */
export namespace bgcnt {
  // @ts-ignore: decorator
  @inline export function priority(c: u16, p: u16): u16 {
    return (c & ~3) | (p & 3);
  }
  // @ts-ignore: decorator
  @inline export function charBlock(c: u16, b: u16): u16 {
    return (c & ~0xc) | ((b & 3) << 2);
  }
  // @ts-ignore: decorator
  @inline export function screenBlock(c: u16, b: u16): u16 {
    return (c & ~0x1f00) | ((b & 31) << 8);
  }
  // @ts-ignore: decorator
  @inline export function size(c: u16, s: MapSize): u16 {
    return (c & 0x3fff) | (<u16>s << 14);
  }
  // @ts-ignore: decorator
  @inline export function bpp8(c: u16): u16 {
    return c | 0x80;
  }
  // @ts-ignore: decorator
  @inline export function mosaic(c: u16): u16 {
    return c | 0x40;
  }
  /** Map size of an affine background. */
  // @ts-ignore: decorator
  @inline export function affineSize(c: u16, s: AffineSize): u16 {
    return (c & 0x3fff) | (<u16>s << 14);
  }
  /** An affine background repeats instead of being transparent outside its map. */
  // @ts-ignore: decorator
  @inline export function wrap(c: u16): u16 {
    return c | 0x2000;
  }
}

/** Packs 8-bit channels into RGB555. */
// @ts-ignore: decorator
@inline export function rgb(r: i32, g: i32, b: i32): u16 {
  return <u16>(((r & 255) >> 3) | (((g & 255) >> 3) << 5) | (((b & 255) >> 3) << 10));
}

/** Sets background color `i` (0–255; 16 banks of 16 for 4bpp graphics). */
// @ts-ignore: decorator
@inline export function setBgColor(i: i32, c: u16): void {
  store<u16>(PAL_BG + (<usize>i << 1), c);
}
/** Sets sprite color `i` (0–255). */
// @ts-ignore: decorator
@inline export function setObjColor(i: i32, c: u16): void {
  store<u16>(PAL_OBJ + (<usize>i << 1), c);
}

/** Address of 4bpp background tile `index` counted from character block `block` (0–3). Tiles are 32 bytes. */
// @ts-ignore: decorator
@inline export function bgTile(block: i32, index: i32): usize {
  return VRAM + <usize>block * 0x4000 + (<usize>index << 5);
}
/** Address of 8bpp background tile `index` counted from character block `block` (0–3). Tiles are 64 bytes;
 *  affine backgrounds always use these. */
// @ts-ignore: decorator
@inline export function bgTile8(block: i32, index: i32): usize {
  return VRAM + <usize>block * 0x4000 + (<usize>index << 6);
}
/** Address of an affine background's map at screen block `block` (0–31): one tile index per byte,
 *  row-major (SPEC.md §8.4). */
// @ts-ignore: decorator
@inline export function affineMap(block: i32): usize {
  return VRAM + <usize>block * 0x800;
}

/** Address of 4bpp sprite tile `index` (0–1023). */
// @ts-ignore: decorator
@inline export function objTile(index: i32): usize {
  return OBJ_VRAM + (<usize>index << 5);
}
/** Fills a 32-byte 4bpp tile with one byte value, e.g. 0x11 for solid color 1. */
// @ts-ignore: decorator
@inline export function fillTile(tile: usize, byte: u8): void {
  memory.fill(tile, byte, 32);
}

/** Address of screen block `block` (0–31): a 32×32 map of u16 entries, row-major. */
// @ts-ignore: decorator
@inline export function screenBlock(block: i32): usize {
  return VRAM + <usize>block * 0x800;
}
/** Sets entry `i` (0–1023) of screen block `block`. */
// @ts-ignore: decorator
@inline export function setScreenEntry(block: i32, i: i32, e: u16): void {
  store<u16>(screenBlock(block) + (<usize>i << 1), e);
}

/** Background map entries (SPEC.md §8.2). */
export namespace screenEntry {
  // @ts-ignore: decorator
  @inline export function make(tile: u16, bank: u16): u16 {
    return (tile & 0x3ff) | ((bank & 0xf) << 12);
  }
  // @ts-ignore: decorator
  @inline export function hflip(e: u16): u16 {
    return e | (1 << 10);
  }
  // @ts-ignore: decorator
  @inline export function vflip(e: u16): u16 {
    return e | (1 << 11);
  }
}

// ---- Sprites (SPEC.md §9) ------------------------------------------------------

/** Sprite sizes as shape × 4 + size (SPEC.md §9.2). */
export enum SpriteSize {
  S8x8 = 0,
  S16x16 = 1,
  S32x32 = 2,
  S64x64 = 3,
  W16x8 = 4,
  W32x8 = 5,
  W32x16 = 6,
  W64x32 = 7,
  T8x16 = 8,
  T8x32 = 9,
  T16x32 = 10,
  T32x64 = 11,
}

/** Width in pixels of a sprite size. */
export function spriteWidth(s: SpriteSize): i32 {
  const shape = <i32>s >> 2, size = <i32>s & 3;
  if (shape == 0) return 8 << size;
  if (shape == 1) return size == 0 ? 16 : size == 3 ? 64 : 32;
  return size == 3 ? 32 : size == 2 ? 16 : 8;
}
/** Height in pixels of a sprite size. */
export function spriteHeight(s: SpriteSize): i32 {
  const shape = <i32>s >> 2, size = <i32>s & 3;
  if (shape == 0) return 8 << size;
  if (shape == 1) return size == 3 ? 32 : size == 2 ? 16 : 8;
  return size == 0 ? 16 : size == 3 ? 64 : 32;
}

/** One OAM entry's three attributes, packed as attr0 | attr1 << 16 | attr2 << 32. */
export type Sprite = u64;

/** Builders for sprites (SPEC.md §9.1), e.g. `sprite.affine(sprite.make(x, y, SpriteSize.S16x16, 0), 0, true)`. */
export namespace sprite {
  export const HIDDEN: Sprite = <u64>OBJ_HIDDEN;

  // @ts-ignore: decorator
  @inline export function attr0(s: Sprite): u16 {
    return <u16>s;
  }
  // @ts-ignore: decorator
  @inline export function attr1(s: Sprite): u16 {
    return <u16>(s >> 16);
  }
  // @ts-ignore: decorator
  @inline export function attr2(s: Sprite): u16 {
    return <u16>(s >> 32);
  }
  // @ts-ignore: decorator
  @inline export function pack(a0: u16, a1: u16, a2: u16): Sprite {
    return <u64>a0 | (<u64>a1 << 16) | (<u64>a2 << 32);
  }

  /** A sprite with its top-left at screen (x, y), using `tile` and bank 0. */
  export function make(x: i32, y: i32, size: SpriteSize, tile: u16): Sprite {
    const shape = <u16>(<i32>size >> 2), sz = <u16>(<i32>size & 3);
    return pack(<u16>(y & 0xff) | (shape << 14), <u16>(x & 0x1ff) | (sz << 14), tile & 0x3ff);
  }
  export function bank(s: Sprite, b: u16): Sprite {
    return pack(attr0(s), attr1(s), (attr2(s) & 0x0fff) | ((b & 0xf) << 12));
  }
  export function priority(s: Sprite, p: u16): Sprite {
    return pack(attr0(s), attr1(s), (attr2(s) & ~0x0c00) | ((p & 3) << 10));
  }
  export function hflip(s: Sprite, on: bool): Sprite {
    return pack(attr0(s), on ? attr1(s) | 0x1000 : attr1(s) & ~0x1000, attr2(s));
  }
  export function vflip(s: Sprite, on: bool): Sprite {
    return pack(attr0(s), on ? attr1(s) | 0x2000 : attr1(s) & ~0x2000, attr2(s));
  }
  export function semiTransparent(s: Sprite): Sprite {
    return pack((attr0(s) & ~0x0c00) | 0x0400, attr1(s), attr2(s));
  }
  export function window(s: Sprite): Sprite {
    return pack((attr0(s) & ~0x0c00) | 0x0800, attr1(s), attr2(s));
  }
  export function mosaic(s: Sprite): Sprite {
    return pack(attr0(s) | 0x1000, attr1(s), attr2(s));
  }
  export function bpp8(s: Sprite): Sprite {
    return pack(attr0(s) | 0x2000, attr1(s), attr2(s));
  }
  /** Uses affine matrix `slot` (0–31). With `doubleSize` the bounding box
   *  doubles so rotated or enlarged pixels are not clipped. */
  export function affine(s: Sprite, slot: u16, doubleSize: bool): Sprite {
    const a0 = attr0(s) | 0x0100 | (doubleSize ? 0x0200 : 0);
    const a1 = (attr1(s) & ~0x3e00) | ((slot & 31) << 9);
    return pack(<u16>a0, a1, attr2(s));
  }
}

/** An affine matrix in 8.8 fixed point, mapping screen to texture space,
 *  packed as pa | pb << 16 | pc << 32 | pd << 48. */
export type AffineMatrix = u64;

export namespace affine {
  export const IDENTITY: AffineMatrix = 256 | (<u64>256 << 48);

  // @ts-ignore: decorator
  @inline export function make(pa: i16, pb: i16, pc: i16, pd: i16): AffineMatrix {
    return <u64><u16>pa | (<u64><u16>pb << 16) | (<u64><u16>pc << 32) | (<u64><u16>pd << 48);
  }
  // @ts-ignore: decorator
  @inline export function pa(m: AffineMatrix): i16 {
    return <i16>m;
  }
  // @ts-ignore: decorator
  @inline export function pb(m: AffineMatrix): i16 {
    return <i16>(m >> 16);
  }
  // @ts-ignore: decorator
  @inline export function pc(m: AffineMatrix): i16 {
    return <i16>(m >> 32);
  }
  // @ts-ignore: decorator
  @inline export function pd(m: AffineMatrix): i16 {
    return <i16>(m >> 48);
  }

  function clamp16(v: i32): i16 {
    return <i16>(v < -32768 ? -32768 : v > 32767 ? 32767 : v);
  }

  /** Rotates counter-clockwise by `angle` 1/256 turns and scales by (sx, sy)
   *  in 8.8 fixed point (256 = 1×, nonzero). Integer math, identical in
   *  every WASM-AD SDK. */
  export function rotateScaleFixed(angle: i32, sx: i32, sy: i32): AffineMatrix {
    const s = fixed.sin(angle), c = fixed.cos(angle);
    return make(clamp16((c * 4) / sx), clamp16((-s * 4) / sx), clamp16((s * 4) / sy), clamp16((c * 4) / sy));
  }
}

/** OAM: 128 sprites, plus affine matrices stored in the gaps. */
export namespace oam {
  export function set(i: i32, s: Sprite): void {
    const at = OAM + (<usize>i << 3);
    store<u16>(at, sprite.attr0(s));
    store<u16>(at + 2, sprite.attr1(s));
    store<u16>(at + 4, sprite.attr2(s));
  }
  export function hide(i: i32): void {
    set(i, sprite.HIDDEN);
  }
  export function hideAll(): void {
    for (let i = 0; i < 128; i++) hide(i);
  }
  export function setAffine(slot: i32, m: AffineMatrix): void {
    const at = OAM + <usize>slot * 32 + 6;
    store<u16>(at, <u16>affine.pa(m));
    store<u16>(at + 8, <u16>affine.pb(m));
    store<u16>(at + 16, <u16>affine.pc(m));
    store<u16>(at + 24, <u16>affine.pd(m));
  }
}

/** Sets affine background `bg` (2 or 3) to matrix `m`, placed so that texture
 *  pixel (texX, texY) appears at screen pixel (scrX, scrY). Rotation and
 *  scaling then happen around that point. */
export function setBgAffine(bg: i32, m: AffineMatrix, texX: i32, texY: i32, scrX: i32, scrY: i32): void {
  const at = reg.BG2PA + <usize>(bg - 2) * 0x10;
  const pa = <i32>affine.pa(m), pb = <i32>affine.pb(m), pc = <i32>affine.pc(m), pd = <i32>affine.pd(m);
  store<u16>(at, <u16>pa);
  store<u16>(at + 2, <u16>pb);
  store<u16>(at + 4, <u16>pc);
  store<u16>(at + 6, <u16>pd);
  store<u32>(at + 8, <u32>((texX << 8) - pa * scrX - pb * scrY));
  store<u32>(at + 12, <u32>((texY << 8) - pc * scrX - pd * scrY));
}

// ---- Sound (SPEC.md §14) -------------------------------------------------------

/** Sound: four GBA-style channels written through `reg.SOUND*`, with
 *  builders for their fields, and setup:
 *
 *    sound.enable();
 *    write16(reg.SOUND2CNT_L, sound.DUTY_50 | sound.envelope(12, 2, false));
 *    write16(reg.SOUND2CNT_H, sound.START | sound.rate(440));
 */
export namespace sound {
  /** Starts (or restarts) a channel; the host clears it. */
  export const START: u16 = 1 << 15;
  /** Stops the channel when its length runs out. */
  export const LENGTH: u16 = 1 << 14;

  export const DUTY_12: u16 = 0 << 6;
  export const DUTY_25: u16 = 1 << 6;
  export const DUTY_50: u16 = 2 << 6;
  export const DUTY_75: u16 = 3 << 6;

  /** `SOUND3CNT_L`: the wave channel is on. */
  export const WAVE_ON: u16 = 1 << 7;
  /** `SOUND3CNT_L`: play bank 1 (or start with it, with `WAVE_64`). */
  export const WAVE_BANK1: u16 = 1 << 6;
  /** `SOUND3CNT_L`: play both banks as one 64-sample wave. */
  export const WAVE_64: u16 = 1 << 5;
  /** `SOUND3CNT_H` volumes. */
  export const WAVE_100: u16 = 1 << 13;
  export const WAVE_50: u16 = 2 << 13;
  export const WAVE_25: u16 = 3 << 13;
  export const WAVE_75: u16 = 1 << 15;

  /** Turns sound on with every channel on both sides at full volume. */
  export function enable(): void {
    store<u16>(reg.SOUNDCNT_X, 0x80);
    store<u16>(reg.SOUNDCNT_L, 0xff77);
    store<u16>(reg.SOUNDCNT_H, 0x330e);
  }
  /** Turns sound off; every channel stops. */
  export function disable(): void {
    store<u16>(reg.SOUNDCNT_X, 0);
  }

  /** The rate field for a square channel playing `hz` (64 Hz and up); the
   *  wave channel plays an octave lower at the same rate. */
  // @ts-ignore: decorator
  @inline export function rate(hz: u32): u16 {
    return hz < 64 ? 0 : <u16>(2048 - 131072 / hz);
  }
  /** `SOUND1CNT_L`: every `time` 128ths of a second (1–7, 0 = off), the
   *  rate changes by rate >> `shift` (1–7), down if `decrease`. */
  // @ts-ignore: decorator
  @inline export function sweep(time: u16, shift: u16, decrease: bool): u16 {
    return ((time & 7) << 4) | (shift & 7) | (decrease ? 8 : 0);
  }
  /** The envelope fields: start at `volume` (0–15), stepping by one every
   *  `step` 64ths of a second (0 = constant), up if `increase`. */
  // @ts-ignore: decorator
  @inline export function envelope(volume: u16, step: u16, increase: bool): u16 {
    return ((volume & 15) << 12) | ((step & 7) << 8) | (increase ? 1 << 11 : 0);
  }
  /** The length field of channels 1, 2 and 4 for `n` 256ths of a second
   *  (1–64); it only counts with `LENGTH` set. Channel 3 takes 256 − n. */
  // @ts-ignore: decorator
  @inline export function length(n: u16): u16 {
    return (64 - n) & 63;
  }
  /** `SOUND4CNT_H`: noise from a divider (0–7) and shift (0–13), with the
   *  7-bit register if `short` (more tonal). */
  // @ts-ignore: decorator
  @inline export function noise(divider: u16, shift: u16, short: bool): u16 {
    return (divider & 7) | ((shift & 15) << 4) | (short ? 8 : 0);
  }

  /** Address of wave RAM: two banks of 16 bytes, each 32 4-bit samples, high nibble first. */
  // @ts-ignore: decorator
  @inline export function waveRam(): usize {
    return WAVE_RAM;
  }
  /** Address of the 28 KB of sample RAM (signed 8-bit) that the sample channels play from. */
  // @ts-ignore: decorator
  @inline export function sampleRam(): usize {
    return SAMPLE_RAM;
  }
  /** Which channels are playing: bits 0–3 channels 1–4, bits 4 and 5 sample
   *  channels A and B, as of the end of the last frame. */
  // @ts-ignore: decorator
  @inline export function status(): u16 {
    return load<u16>(reg.SOUNDCNT_X) & 0x3f;
  }
}

/** The two sample channels, A (0) and B (1), playing signed 8-bit samples
 *  from sample RAM (SPEC.md §14.6). */
export namespace pcm {
  // @ts-ignore: decorator
  @inline function base(ch: i32): usize {
    return ch == 0 ? reg.PCMA_SAD : reg.PCMB_SAD;
  }

  /** Plays `len` samples from sample RAM offset `start` at `rate` Hz and `volume` 0–64, once. */
  export function play(ch: i32, start: u16, len: u16, rate: u16, volume: u16): void {
    const b = base(ch);
    store<u16>(b, start);
    store<u16>(b + 2, len);
    store<u16>(b + 6, rate);
    store<u16>(b + 8, sound.START | (volume < 64 ? volume : 64));
  }
  /** Like `play`, then repeats from sample `loopAt` until stopped. */
  export function playLooped(ch: i32, start: u16, len: u16, loopAt: u16, rate: u16, volume: u16): void {
    store<u16>(base(ch) + 4, loopAt);
    play(ch, start, len, rate, volume);
    store<u16>(base(ch) + 8, load<u16>(base(ch) + 8) | (1 << 13));
  }
  export function stop(ch: i32): void {
    store<u16>(base(ch) + 8, 1 << 14);
  }
  /** The playback position, in samples from the start, as of the end of the last frame. */
  export function position(ch: i32): u16 {
    return load<u16>(base(ch) + 10);
  }
  export function playing(ch: i32): bool {
    return (sound.status() & (1 << (4 + ch))) != 0;
  }
}

// ---- Fixed-point trigonometry --------------------------------------------------

/** Fixed-point trigonometry shared by all WASM-AD SDKs, so carts in any
 *  language compute bit-identical results (SPEC.md §12). */
export namespace fixed {
  /** sin over a quarter turn in 1/256-turn steps, scaled by 16384 (Q14). */
  export const SIN_Q14: usize = memory.data<i32>([
    0, 402, 804, 1205, 1606, 2006, 2404, 2801, 3196, 3590, 3981, 4370, 4756, 5139, 5520, 5897, 6270, 6639, 7005,
    7366, 7723, 8076, 8423, 8765, 9102, 9434, 9760, 10080, 10394, 10702, 11003, 11297, 11585, 11866, 12140, 12406,
    12665, 12916, 13160, 13395, 13623, 13842, 14053, 14256, 14449, 14635, 14811, 14978, 15137, 15286, 15426, 15557,
    15679, 15791, 15893, 15986, 16069, 16143, 16207, 16261, 16305, 16340, 16364, 16379, 16384,
  ]);

  // @ts-ignore: decorator
  @inline function q(i: i32): i32 {
    return load<i32>(SIN_Q14 + (<usize>i << 2));
  }

  /** sin of `angle` 1/256 turns (only the low 8 bits count), in Q14. */
  export function sin(angle: i32): i32 {
    const a = angle & 255;
    if (a < 64) return q(a);
    if (a < 128) return q(128 - a);
    if (a < 192) return -q(a - 128);
    return -q(256 - a);
  }

  /** cos of `angle` 1/256 turns, in Q14. */
  export function cos(angle: i32): i32 {
    return sin(angle + 64);
  }
}

// ---- Input, time, save RAM -------------------------------------------------------

/** Buttons held this frame. */
// @ts-ignore: decorator
@inline export function keys(): u16 {
  return load<u16>(reg.KEYS);
}

/** Edge detection over `KEYS`: call `buttons.update()` at the start of every
 *  frame. Its two words of state live in linear memory. */
export namespace buttons {
  const STATE: usize = memory.data(4); // held: u16, prev: u16

  export function update(): void {
    store<u16>(STATE + 2, load<u16>(STATE));
    store<u16>(STATE, keys());
  }
  export function held(k: u16): bool {
    return (load<u16>(STATE) & k) != 0;
  }
  /** True only on the frame the button went down. */
  export function pressed(k: u16): bool {
    return (load<u16>(STATE) & ~load<u16>(STATE + 2) & k) != 0;
  }
}

/** Frames since boot. */
// @ts-ignore: decorator
@inline export function frame(): u32 {
  return load<u32>(reg.FRAME);
}

/** The boot-time random seed, the cart's only entropy (SPEC.md §12). */
// @ts-ignore: decorator
@inline export function seed(): u32 {
  return load<u32>(reg.SEED);
}

/** Address of the 32 KB battery-backed save RAM. */
// @ts-ignore: decorator
@inline export function saveRam(): usize {
  return SAVE;
}

/** Whether the host restored save RAM from storage at boot. */
// @ts-ignore: decorator
@inline export function saveRestored(): bool {
  return (load<u16>(reg.SAVESTAT) & 1) != 0;
}

/** Asks the host to persist save RAM after this frame. */
// @ts-ignore: decorator
@inline export function requestSave(): void {
  store<u16>(reg.SAVECNT, load<u16>(reg.SAVECNT) | 1);
}

// ---- Debug log -------------------------------------------------------------------

// @ts-ignore: decorator
@external("env", "trace")
declare function hostTrace(ptr: usize, len: i32): void;

/** Writes a message to the host's debug log, as UTF-8 (at most 1 KB). */
export function trace(msg: string): void {
  const buf = memory.data(1024);
  let n: usize = 0;
  const len = msg.length;
  for (let i = 0; i < len; i++) {
    let c = <u32>msg.charCodeAt(i);
    if (c >= 0xd800 && c < 0xdc00 && i + 1 < len) {
      const lo = <u32>msg.charCodeAt(i + 1);
      if (lo >= 0xdc00 && lo < 0xe000) {
        c = 0x10000 + ((c - 0xd800) << 10) + (lo - 0xdc00);
        i++;
      }
    }
    const need: usize = c < 0x80 ? 1 : c < 0x800 ? 2 : c < 0x10000 ? 3 : 4;
    if (n + need > 1024) break;
    if (need == 1) store<u8>(buf + n, c);
    else if (need == 2) {
      store<u8>(buf + n, 0xc0 | (c >> 6));
      store<u8>(buf + n + 1, 0x80 | (c & 63));
    } else if (need == 3) {
      store<u8>(buf + n, 0xe0 | (c >> 12));
      store<u8>(buf + n + 1, 0x80 | ((c >> 6) & 63));
      store<u8>(buf + n + 2, 0x80 | (c & 63));
    } else {
      store<u8>(buf + n, 0xf0 | (c >> 18));
      store<u8>(buf + n + 1, 0x80 | ((c >> 12) & 63));
      store<u8>(buf + n + 2, 0x80 | ((c >> 6) & 63));
      store<u8>(buf + n + 3, 0x80 | (c & 63));
    }
    n += need;
  }
  hostTrace(buf, <i32>n);
}

/** Replaces AssemblyScript's `env.abort` import (`--use abort=...` in asconfig.json):
 *  a failed assertion or bounds check traps. */
export function abort(message: string | null, file: string | null, line: u32, column: u32): void {
  unreachable();
}
