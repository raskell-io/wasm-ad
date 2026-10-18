/*
 * C and C++ SDK for WASM-AD carts. One header, no library, no libc.
 *
 *   #include "wasm_ad.h"
 *
 *   WAD_CART(WAD_TITLE("Hello") WAD_AUTHOR("you"));
 *
 *   WAD_EXPORT(update) void update(void) {
 *       WAD_DISPCNT = WAD_MODE0;
 *   }
 *
 * A cart drives the console by writing its memory-mapped hardware, which
 * this header names after SPEC.md. Build with clang and wasm-ld; see README.md
 * for the flags that keep the cart's data and stack out of the hardware region.
 */
#ifndef WASM_AD_H
#define WASM_AD_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Cart interface (SPEC.md §2) ------------------------------------------ */

/* Exports a function under `name`: update, start or scanline. */
#define WAD_EXPORT(name) __attribute__((export_name(#name)))

/*
 * Emits the `wasm-ad` metadata section (§2.3) with `spec` filled in. Use once,
 * at file scope, with any of the key macros below. Values must not contain
 * double quotes, backslashes or newlines.
 */
#define WAD_CART(...) \
    __asm__(".section \".custom_section.wasm-ad\",\"\",@\n.ascii \"spec=0.2\\n\"\n" __VA_ARGS__)
#define WAD_META(key, value) ".ascii \"" key "=" value "\\n\"\n"
#define WAD_TITLE(v) WAD_META("title", v)
#define WAD_AUTHOR(v) WAD_META("author", v)
#define WAD_VERSION(v) WAD_META("version", v)
#define WAD_DESCRIPTION(v) WAD_META("description", v)
#define WAD_URL(v) WAD_META("url", v)

/* ---- Memory map (§4) ------------------------------------------------------- */

#define WAD_SCREEN_W 240
#define WAD_SCREEN_H 160

#define WAD_ADDR_PAL_BG 0x00800u
#define WAD_ADDR_PAL_OBJ 0x00A00u
#define WAD_ADDR_OAM 0x00C00u
#define WAD_ADDR_VRAM 0x01000u
#define WAD_VRAM_SIZE 0x18000u
#define WAD_ADDR_OBJ_VRAM (WAD_ADDR_VRAM + 0x10000u)
#define WAD_ADDR_SAVE 0x19000u
#define WAD_SAVE_SIZE 0x8000u
#define WAD_ADDR_SAMPLE_RAM 0x21000u
#define WAD_SAMPLE_RAM_SIZE 0x7000u
/* First address the cart may use for its own data, stack and heap. */
#define WAD_ADDR_CART 0x28000u

#define WAD_REG16(addr) (*(volatile uint16_t *)(uintptr_t)(addr))
#define WAD_REG32(addr) (*(volatile uint32_t *)(uintptr_t)(addr))

/* ---- Registers, by their SPEC.md names (§5, §6) ---------------------------- */

#define WAD_DISPCNT WAD_REG16(0x400)
#define WAD_VCOUNT WAD_REG16(0x406)
#define WAD_BGCNT(n) WAD_REG16(0x408 + (n) * 2)
#define WAD_BGHOFS(n) WAD_REG16(0x410 + (n) * 4)
#define WAD_BGVOFS(n) WAD_REG16(0x412 + (n) * 4)
#define WAD_WIN0H WAD_REG16(0x440)
#define WAD_WIN1H WAD_REG16(0x442)
#define WAD_WIN0V WAD_REG16(0x444)
#define WAD_WIN1V WAD_REG16(0x446)
#define WAD_WININ WAD_REG16(0x448)
#define WAD_WINOUT WAD_REG16(0x44A)
#define WAD_MOSAIC WAD_REG16(0x44C)
#define WAD_BLDCNT WAD_REG16(0x450)
#define WAD_BLDALPHA WAD_REG16(0x452)
#define WAD_BLDY WAD_REG16(0x454)
/* Affine background parameters (§8.4), for bg 2 or 3. */
#define WAD_BGPA(n) WAD_REG16(0x420 + ((n) - 2) * 0x10)
#define WAD_BGPB(n) WAD_REG16(0x422 + ((n) - 2) * 0x10)
#define WAD_BGPC(n) WAD_REG16(0x424 + ((n) - 2) * 0x10)
#define WAD_BGPD(n) WAD_REG16(0x426 + ((n) - 2) * 0x10)
#define WAD_BGX(n) WAD_REG32(0x428 + ((n) - 2) * 0x10)
#define WAD_BGY(n) WAD_REG32(0x42C + ((n) - 2) * 0x10)
#define WAD_BG2PA WAD_BGPA(2)
#define WAD_BG2PB WAD_BGPB(2)
#define WAD_BG2PC WAD_BGPC(2)
#define WAD_BG2PD WAD_BGPD(2)
#define WAD_BG2X WAD_BGX(2)
#define WAD_BG2Y WAD_BGY(2)
#define WAD_BG3PA WAD_BGPA(3)
#define WAD_BG3PB WAD_BGPB(3)
#define WAD_BG3PC WAD_BGPC(3)
#define WAD_BG3PD WAD_BGPD(3)
#define WAD_BG3X WAD_BGX(3)
#define WAD_BG3Y WAD_BGY(3)
/* Sound (§14). */
#define WAD_SOUND1CNT_L WAD_REG16(0x460)
#define WAD_SOUND1CNT_H WAD_REG16(0x462)
#define WAD_SOUND1CNT_X WAD_REG16(0x464)
#define WAD_SOUND2CNT_L WAD_REG16(0x468)
#define WAD_SOUND2CNT_H WAD_REG16(0x46C)
#define WAD_SOUND3CNT_L WAD_REG16(0x470)
#define WAD_SOUND3CNT_H WAD_REG16(0x472)
#define WAD_SOUND3CNT_X WAD_REG16(0x474)
#define WAD_SOUND4CNT_L WAD_REG16(0x478)
#define WAD_SOUND4CNT_H WAD_REG16(0x47C)
#define WAD_SOUNDCNT_L WAD_REG16(0x480)
#define WAD_SOUNDCNT_H WAD_REG16(0x482)
#define WAD_SOUNDCNT_X WAD_REG16(0x484)
#define WAD_PCMA_SAD WAD_REG16(0x4B0)
#define WAD_PCMA_LEN WAD_REG16(0x4B2)
#define WAD_PCMA_LOOP WAD_REG16(0x4B4)
#define WAD_PCMA_RATE WAD_REG16(0x4B6)
#define WAD_PCMA_CNT WAD_REG16(0x4B8)
#define WAD_PCMA_POS WAD_REG16(0x4BA)
#define WAD_PCMB_SAD WAD_REG16(0x4C0)
#define WAD_PCMB_LEN WAD_REG16(0x4C2)
#define WAD_PCMB_LOOP WAD_REG16(0x4C4)
#define WAD_PCMB_RATE WAD_REG16(0x4C6)
#define WAD_PCMB_CNT WAD_REG16(0x4C8)
#define WAD_PCMB_POS WAD_REG16(0x4CA)
#define WAD_KEYS WAD_REG16(0x700)
#define WAD_FRAME WAD_REG32(0x704)
#define WAD_SEED WAD_REG32(0x708)
#define WAD_SAVECNT WAD_REG16(0x70C)
#define WAD_SAVESTAT WAD_REG16(0x70E)
#define WAD_SPEC WAD_REG16(0x710)

/* DISPCNT bits (§6.1). */
#define WAD_MODE0 0x0000u
#define WAD_MODE1 0x0001u /* BG0 and BG1 tiled, BG2 affine */
#define WAD_MODE2 0x0002u /* BG2 and BG3 affine */
#define WAD_MODE3 0x0003u
#define WAD_MODE4 0x0004u
#define WAD_MODE5 0x0005u
#define WAD_PAGE1 0x0010u
#define WAD_OBJ_1D 0x0040u
#define WAD_FORCED_BLANK 0x0080u
#define WAD_DISP_BG0 0x0100u
#define WAD_DISP_BG1 0x0200u
#define WAD_DISP_BG2 0x0400u
#define WAD_DISP_BG3 0x0800u
#define WAD_DISP_OBJ 0x1000u
#define WAD_DISP_WIN0 0x2000u
#define WAD_DISP_WIN1 0x4000u
#define WAD_DISP_OBJ_WIN 0x8000u

/* Layer bits for BLDCNT targets and window masks. */
#define WAD_LAYER_BG0 0x01u
#define WAD_LAYER_BG1 0x02u
#define WAD_LAYER_BG2 0x04u
#define WAD_LAYER_BG3 0x08u
#define WAD_LAYER_OBJ 0x10u
#define WAD_LAYER_BACKDROP 0x20u
#define WAD_LAYER_EFFECTS 0x20u /* in window masks: color effects enabled */
#define WAD_LAYER_ALL 0x3Fu

/* BLDCNT effects (§6.5). */
#define WAD_BLEND_NONE 0x0000u
#define WAD_BLEND_ALPHA 0x0040u
#define WAD_BLEND_BRIGHTEN 0x0080u
#define WAD_BLEND_DARKEN 0x00C0u

/* BLDCNT value: `effect` on `first` targets, blending onto `second`. */
static inline uint16_t wad_blend_cnt(uint16_t effect, uint16_t first, uint16_t second) {
    return (uint16_t)(effect | first | (second << 8));
}

/* BLDALPHA value; coefficients 0–16. */
static inline uint16_t wad_blend_alpha(uint16_t eva, uint16_t evb) {
    return (uint16_t)(eva | (evb << 8));
}

/* BGxCNT fields (§6.2), OR-ed together. */
#define WAD_BG_PRIORITY(p) ((uint16_t)((p) & 3))
#define WAD_BG_CHAR_BLOCK(b) ((uint16_t)(((b) & 3) << 2))
#define WAD_BG_MOSAIC 0x0040u
#define WAD_BG_8BPP 0x0080u
#define WAD_BG_SCREEN_BLOCK(b) ((uint16_t)(((b) & 31) << 8))
#define WAD_BG_SIZE_32x32 0x0000u
#define WAD_BG_SIZE_64x32 0x4000u
#define WAD_BG_SIZE_32x64 0x8000u
#define WAD_BG_SIZE_64x64 0xC000u
/* Affine backgrounds (§8.4): map sizes in tiles, and repeat outside the map. */
#define WAD_BG_WRAP 0x2000u
#define WAD_BG_AFFINE_16x16 0x0000u
#define WAD_BG_AFFINE_32x32 0x4000u
#define WAD_BG_AFFINE_64x64 0x8000u
#define WAD_BG_AFFINE_128x128 0xC000u

/* KEYS bits (§5). */
#define WAD_KEY_A 0x0001u
#define WAD_KEY_B 0x0002u
#define WAD_KEY_SELECT 0x0004u
#define WAD_KEY_START 0x0008u
#define WAD_KEY_RIGHT 0x0010u
#define WAD_KEY_LEFT 0x0020u
#define WAD_KEY_UP 0x0040u
#define WAD_KEY_DOWN 0x0080u
#define WAD_KEY_R 0x0100u
#define WAD_KEY_L 0x0200u

/* ---- Palette, tiles and maps (§7, §8) -------------------------------------- */

/* Packs 8-bit channels into RGB555. */
static inline uint16_t wad_rgb(uint8_t r, uint8_t g, uint8_t b) {
    return (uint16_t)((r >> 3) | ((g >> 3) << 5) | ((b >> 3) << 10));
}

/* The 256 background colors (16 banks of 16 for 4bpp) and 256 sprite colors. */
#define wad_bg_palette ((volatile uint16_t *)(uintptr_t)WAD_ADDR_PAL_BG)
#define wad_obj_palette ((volatile uint16_t *)(uintptr_t)WAD_ADDR_PAL_OBJ)

/* All of VRAM as bytes. */
#define wad_vram ((volatile uint8_t *)(uintptr_t)WAD_ADDR_VRAM)

/* A 4bpp tile: 8 rows of 4 bytes, left pixel in the low nibble. */
typedef uint8_t wad_tile4[32];

/* 4bpp background tiles from character block `block` (0–3). */
static inline volatile wad_tile4 *wad_bg_tiles(unsigned block) {
    return (volatile wad_tile4 *)(uintptr_t)(WAD_ADDR_VRAM + block * 0x4000u);
}

/* An 8bpp tile: 8 rows of 8 bytes. Affine backgrounds always use these. */
typedef uint8_t wad_tile8[64];

/* 8bpp background tiles from character block `block` (0–3). */
static inline volatile wad_tile8 *wad_bg_tiles8(unsigned block) {
    return (volatile wad_tile8 *)(uintptr_t)(WAD_ADDR_VRAM + block * 0x4000u);
}

/* An affine background's map from screen block `block` (0–31): one tile index
 * per byte, row-major (§8.4). */
static inline volatile uint8_t *wad_affine_map(unsigned block) {
    return (volatile uint8_t *)(uintptr_t)(WAD_ADDR_VRAM + block * 0x800u);
}

/* The 1024 4bpp sprite tiles. */
#define wad_obj_tiles ((volatile wad_tile4 *)(uintptr_t)WAD_ADDR_OBJ_VRAM)

/* A background map entry (§8.2), OR-ed with WAD_SE_HFLIP / WAD_SE_VFLIP. */
static inline uint16_t wad_screen_entry(uint16_t tile, uint16_t bank) {
    return (uint16_t)((tile & 0x3FF) | ((bank & 0xF) << 12));
}
#define WAD_SE_HFLIP 0x0400u
#define WAD_SE_VFLIP 0x0800u

/* Screen block `block` (0–31): a 32×32 map, row-major. */
static inline volatile uint16_t *wad_screen_block(unsigned block) {
    return (volatile uint16_t *)(uintptr_t)(WAD_ADDR_VRAM + block * 0x800u);
}

/* ---- Sprites (§9) ------------------------------------------------------------ */

/* Sprite sizes as shape × 4 + size (§9.2). */
typedef enum {
    WAD_S8x8, WAD_S16x16, WAD_S32x32, WAD_S64x64,
    WAD_W16x8, WAD_W32x8, WAD_W32x16, WAD_W64x32,
    WAD_T8x16, WAD_T8x32, WAD_T16x32, WAD_T32x64,
} wad_sprite_size;

/* One OAM entry's three attributes (§9.1). */
typedef struct {
    uint16_t attr0, attr1, attr2;
} wad_sprite;

#define WAD_OBJ_HIDDEN 0x0200u

/* A sprite with its top-left at screen (x, y), using `tile` and bank 0. */
static inline wad_sprite wad_sprite_new(int32_t x, int32_t y, wad_sprite_size size, uint16_t tile) {
    wad_sprite s;
    s.attr0 = (uint16_t)((y & 0xFF) | ((size / 4) << 14));
    s.attr1 = (uint16_t)((x & 0x1FF) | ((size % 4) << 14));
    s.attr2 = (uint16_t)(tile & 0x3FF);
    return s;
}
static inline wad_sprite wad_sprite_bank(wad_sprite s, uint16_t bank) {
    s.attr2 = (uint16_t)((s.attr2 & 0x0FFF) | ((bank & 0xF) << 12));
    return s;
}
static inline wad_sprite wad_sprite_priority(wad_sprite s, uint16_t p) {
    s.attr2 = (uint16_t)((s.attr2 & ~0x0C00) | ((p & 3) << 10));
    return s;
}
static inline wad_sprite wad_sprite_hflip(wad_sprite s, bool on) {
    s.attr1 = (uint16_t)(on ? s.attr1 | 0x1000 : s.attr1 & ~0x1000);
    return s;
}
static inline wad_sprite wad_sprite_vflip(wad_sprite s, bool on) {
    s.attr1 = (uint16_t)(on ? s.attr1 | 0x2000 : s.attr1 & ~0x2000);
    return s;
}
static inline wad_sprite wad_sprite_semi_transparent(wad_sprite s) {
    s.attr0 = (uint16_t)((s.attr0 & ~0x0C00) | 0x0400);
    return s;
}
static inline wad_sprite wad_sprite_window(wad_sprite s) {
    s.attr0 = (uint16_t)((s.attr0 & ~0x0C00) | 0x0800);
    return s;
}
static inline wad_sprite wad_sprite_mosaic(wad_sprite s) {
    s.attr0 = (uint16_t)(s.attr0 | 0x1000);
    return s;
}
static inline wad_sprite wad_sprite_8bpp(wad_sprite s) {
    s.attr0 = (uint16_t)(s.attr0 | 0x2000);
    return s;
}
/* Uses affine matrix `slot` (0–31). With `double_size` the bounding box
 * doubles so rotated or enlarged pixels are not clipped. */
static inline wad_sprite wad_sprite_affine(wad_sprite s, uint16_t slot, bool double_size) {
    s.attr0 = (uint16_t)(s.attr0 | 0x0100 | (double_size ? 0x0200 : 0));
    s.attr1 = (uint16_t)((s.attr1 & ~0x3E00) | ((slot & 31) << 9));
    return s;
}

static inline void wad_oam_set(unsigned i, wad_sprite s) {
    uintptr_t at = WAD_ADDR_OAM + (i & 127) * 8;
    WAD_REG16(at) = s.attr0;
    WAD_REG16(at + 2) = s.attr1;
    WAD_REG16(at + 4) = s.attr2;
}
static inline void wad_oam_hide(unsigned i) {
    wad_sprite s = {WAD_OBJ_HIDDEN, 0, 0};
    wad_oam_set(i, s);
}
static inline void wad_oam_hide_all(void) {
    for (unsigned i = 0; i < 128; i++) wad_oam_hide(i);
}

/* An affine matrix in 8.8 fixed point, mapping screen to texture space (§9.4). */
typedef struct {
    int16_t pa, pb, pc, pd;
} wad_affine;

#define WAD_AFFINE_IDENTITY ((wad_affine){256, 0, 0, 256})

static inline void wad_oam_set_affine(unsigned slot, wad_affine m) {
    uintptr_t at = WAD_ADDR_OAM + (slot & 31) * 32 + 6;
    WAD_REG16(at) = (uint16_t)m.pa;
    WAD_REG16(at + 8) = (uint16_t)m.pb;
    WAD_REG16(at + 16) = (uint16_t)m.pc;
    WAD_REG16(at + 24) = (uint16_t)m.pd;
}

/* ---- Fixed-point trigonometry, shared by all SDKs -------------------------- */

/* sin over a quarter turn in 1/256-turn steps, scaled by 16384 (Q14). */
static const int32_t wad_sin_q14[65] = {
    0, 402, 804, 1205, 1606, 2006, 2404, 2801, 3196, 3590, 3981, 4370, 4756, 5139, 5520, 5897, 6270,
    6639, 7005, 7366, 7723, 8076, 8423, 8765, 9102, 9434, 9760, 10080, 10394, 10702, 11003, 11297, 11585,
    11866, 12140, 12406, 12665, 12916, 13160, 13395, 13623, 13842, 14053, 14256, 14449, 14635, 14811, 14978, 15137,
    15286, 15426, 15557, 15679, 15791, 15893, 15986, 16069, 16143, 16207, 16261, 16305, 16340, 16364, 16379, 16384,
};

/* sin of `angle` 1/256 turns (only the low 8 bits count), in Q14. */
static inline int32_t wad_sin(int32_t angle) {
    int32_t a = angle & 255;
    if (a < 64) return wad_sin_q14[a];
    if (a < 128) return wad_sin_q14[128 - a];
    if (a < 192) return -wad_sin_q14[a - 128];
    return -wad_sin_q14[256 - a];
}

/* cos of `angle` 1/256 turns, in Q14. */
static inline int32_t wad_cos(int32_t angle) {
    return wad_sin(angle + 64);
}

static inline int16_t wad__clamp16(int32_t v) {
    return (int16_t)(v < -32768 ? -32768 : v > 32767 ? 32767 : v);
}

/* Rotates counter-clockwise by `angle` 1/256 turns and scales by (sx, sy) in
 * 8.8 fixed point (256 = 1×, nonzero). The same integer math in every SDK. */
static inline wad_affine wad_affine_rotate_scale(int32_t angle, int32_t sx, int32_t sy) {
    int32_t s = wad_sin(angle), c = wad_cos(angle);
    wad_affine m;
    m.pa = wad__clamp16(c * 4 / sx);
    m.pb = wad__clamp16(-s * 4 / sx);
    m.pc = wad__clamp16(s * 4 / sy);
    m.pd = wad__clamp16(c * 4 / sy);
    return m;
}

/* Sets affine background `bg` (2 or 3) to matrix `m`, placed so that texture
 * pixel (tex_x, tex_y) appears at screen pixel (scr_x, scr_y). Rotation and
 * scaling then happen around that point. */
static inline void wad_set_bg_affine(unsigned bg, wad_affine m, int32_t tex_x, int32_t tex_y, int32_t scr_x,
                                     int32_t scr_y) {
    WAD_BGPA(bg) = (uint16_t)m.pa;
    WAD_BGPB(bg) = (uint16_t)m.pb;
    WAD_BGPC(bg) = (uint16_t)m.pc;
    WAD_BGPD(bg) = (uint16_t)m.pd;
    WAD_BGX(bg) = (uint32_t)(tex_x * 256 - m.pa * scr_x - m.pb * scr_y);
    WAD_BGY(bg) = (uint32_t)(tex_y * 256 - m.pc * scr_x - m.pd * scr_y);
}

/* ---- Sound (§14) ---------------------------------------------------------------- */

/* Starts (or restarts) a channel; the host clears it. */
#define WAD_SOUND_START 0x8000u
/* Stops the channel when its length runs out. */
#define WAD_SOUND_LENGTH 0x4000u
#define WAD_DUTY_12 0x0000u
#define WAD_DUTY_25 0x0040u
#define WAD_DUTY_50 0x0080u
#define WAD_DUTY_75 0x00C0u
/* SOUND3CNT_L: on, play bank 1 (first), play both banks as one 64-sample wave. */
#define WAD_WAVE_ON 0x0080u
#define WAD_WAVE_BANK1 0x0040u
#define WAD_WAVE_64 0x0020u
/* SOUND3CNT_H volumes. */
#define WAD_WAVE_100 0x2000u
#define WAD_WAVE_50 0x4000u
#define WAD_WAVE_25 0x6000u
#define WAD_WAVE_75 0x8000u

/* Turns sound on with every channel on both sides at full volume. */
static inline void wad_sound_enable(void) {
    WAD_SOUNDCNT_X = 0x80;
    WAD_SOUNDCNT_L = 0xFF77;
    WAD_SOUNDCNT_H = 0x330E;
}

/* Turns sound off; every channel stops. */
static inline void wad_sound_disable(void) { WAD_SOUNDCNT_X = 0; }

/* The rate field for a square channel playing `hz` (64 Hz and up); the wave
 * channel plays an octave lower at the same rate. */
static inline uint16_t wad_sound_rate(uint32_t hz) {
    return hz < 64 ? 0 : (uint16_t)(2048 - 131072 / hz);
}

/* SOUND1CNT_L: every `time` 128ths of a second (1–7, 0 = off), the rate
 * changes by rate >> `shift` (1–7), down if `decrease`. */
static inline uint16_t wad_sound_sweep(uint16_t time, uint16_t shift, bool decrease) {
    return (uint16_t)(((time & 7) << 4) | (shift & 7) | (decrease ? 8 : 0));
}

/* The envelope fields: start at `volume` (0–15), stepping by one every `step`
 * 64ths of a second (0 = constant), up if `increase`. */
static inline uint16_t wad_sound_envelope(uint16_t volume, uint16_t step, bool increase) {
    return (uint16_t)(((volume & 15) << 12) | ((step & 7) << 8) | (increase ? 0x800 : 0));
}

/* The length field of channels 1, 2 and 4 for `n` 256ths of a second (1–64);
 * it only counts with WAD_SOUND_LENGTH set. Channel 3 takes 256 − n. */
static inline uint16_t wad_sound_length(uint16_t n) { return (uint16_t)((64 - n) & 63); }

/* SOUND4CNT_H: noise from a divider (0–7) and shift (0–13), with the 7-bit
 * register if `short_mode` (more tonal). */
static inline uint16_t wad_sound_noise(uint16_t divider, uint16_t shift, bool short_mode) {
    return (uint16_t)((divider & 7) | ((shift & 15) << 4) | (short_mode ? 8 : 0));
}

/* Wave RAM: two banks of 16 bytes, each 32 4-bit samples, high nibble first. */
#define wad_wave_ram ((volatile uint8_t *)(uintptr_t)0x490)

/* The 28 KB of sample RAM that the sample channels play from. */
#define wad_sample_ram ((volatile int8_t *)(uintptr_t)WAD_ADDR_SAMPLE_RAM)

/* Which channels are playing: bits 0–3 channels 1–4, bits 4 and 5 sample
 * channels A and B, as of the end of the last frame. */
static inline uint16_t wad_sound_status(void) { return (uint16_t)(WAD_SOUNDCNT_X & 0x3F); }

/* Sample channels A (0) and B (1), playing from wad_sample_ram (§14.6). */
static inline uintptr_t wad__pcm(unsigned ch) { return ch == 0 ? 0x4B0 : 0x4C0; }

/* Plays `len` samples from sample RAM offset `start` at `rate` Hz and
 * `volume` 0–64, once. */
static inline void wad_pcm_play(unsigned ch, uint16_t start, uint16_t len, uint16_t rate, uint16_t volume) {
    uintptr_t b = wad__pcm(ch);
    WAD_REG16(b) = start;
    WAD_REG16(b + 2) = len;
    WAD_REG16(b + 6) = rate;
    WAD_REG16(b + 8) = (uint16_t)(WAD_SOUND_START | (volume < 64 ? volume : 64));
}

/* Like wad_pcm_play, then repeats from sample `loop_at` until stopped. */
static inline void wad_pcm_play_looped(unsigned ch, uint16_t start, uint16_t len, uint16_t loop_at, uint16_t rate,
                                       uint16_t volume) {
    WAD_REG16(wad__pcm(ch) + 4) = loop_at;
    wad_pcm_play(ch, start, len, rate, volume);
    WAD_REG16(wad__pcm(ch) + 8) = (uint16_t)(WAD_REG16(wad__pcm(ch) + 8) | 0x2000);
}

static inline void wad_pcm_stop(unsigned ch) { WAD_REG16(wad__pcm(ch) + 8) = 0x4000; }

/* The playback position, in samples from the start, as of the end of the last frame. */
static inline uint16_t wad_pcm_position(unsigned ch) { return WAD_REG16(wad__pcm(ch) + 10); }

static inline bool wad_pcm_playing(unsigned ch) { return (wad_sound_status() & (1u << (4 + ch))) != 0; }

/* ---- Input, time and entropy (§5, §12) -------------------------------------- */

/* Edge detection over KEYS; keep one in your cart state and call
 * wad_buttons_update at the start of every frame. */
typedef struct {
    uint16_t held, prev;
} wad_buttons;

static inline void wad_buttons_update(wad_buttons *b) {
    b->prev = b->held;
    b->held = WAD_KEYS;
}
static inline bool wad_held(const wad_buttons *b, uint16_t keys) {
    return (b->held & keys) != 0;
}
/* True only on the frame the button went down. */
static inline bool wad_pressed(const wad_buttons *b, uint16_t keys) {
    return (b->held & ~b->prev & keys) != 0;
}

static inline uint32_t wad_frame(void) { return WAD_FRAME; }

/* The boot-time random seed, the cart's only entropy (§12). */
static inline uint32_t wad_seed(void) { return WAD_SEED; }

/* ---- Save RAM (§11) ------------------------------------------------------------ */

/* The 32 KB battery-backed save RAM. */
#define wad_save_ram ((volatile uint8_t *)(uintptr_t)WAD_ADDR_SAVE)

/* Whether the host restored save RAM from storage at boot. */
static inline bool wad_save_restored(void) { return (WAD_SAVESTAT & 1) != 0; }

/* Asks the host to persist save RAM after this frame. */
static inline void wad_request_save(void) { WAD_SAVECNT = (uint16_t)(WAD_SAVECNT | 1); }

/* ---- Debug log (§2.2) ------------------------------------------------------------ */

__attribute__((import_module("env"), import_name("trace"))) void wad__host_trace(const char *ptr, uint32_t len);

/* Writes a NUL-terminated message to the host's debug log. */
static inline void wad_trace(const char *msg) {
    uint32_t len = 0;
    while (msg[len]) len++;
    wad__host_trace(msg, len);
}

#ifdef __cplusplus
}
#endif

#endif /* WASM_AD_H */
