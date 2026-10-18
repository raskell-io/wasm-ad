/*
 * The starter cart, the same in every SDK: a scrolling checkerboard blended
 * over a backdrop gradient drawn per scanline, and a spinning star you move
 * with the D-pad (B to go faster, A to toggle mosaic). It uses integer math
 * only, so every language's build renders bit-identical frames.
 *
 * Also valid C++; the Makefile builds it both ways.
 */
#include "../wasm_ad.h"

WAD_CART(WAD_TITLE("Starter") WAD_AUTHOR("raskell.io") WAD_DESCRIPTION("The WASM-AD starter cart, in C"));

#define MAP_BLOCK 31

/* Cart state lives in linear memory, never in WebAssembly globals (SPEC.md §12). */
static struct {
    wad_buttons buttons;
    int32_t x, y;
    bool mosaic;
} state = {{0, 0}, 112, 72, false};

static int32_t abs32(int32_t v) { return v < 0 ? -v : v; }
static int32_t min32(int32_t a, int32_t b) { return a < b ? a : b; }
static int32_t max32(int32_t a, int32_t b) { return a > b ? a : b; }
static int32_t clamp32(int32_t v, int32_t lo, int32_t hi) { return min32(max32(v, lo), hi); }

WAD_EXPORT(start) void start(void) {
#ifdef __cplusplus
    wad_trace("starter: hello from C++");
#else
    wad_trace("starter: hello from C");
#endif

    /* A checkerboard of two solid tiles on background 0. */
    wad_bg_palette[1] = wad_rgb(255, 255, 255);
    wad_bg_palette[2] = wad_rgb(40, 40, 72);
    volatile wad_tile4 *tiles = wad_bg_tiles(0);
    for (int i = 0; i < 32; i++) {
        tiles[1][i] = 0x11;
        tiles[2][i] = 0x22;
    }
    volatile uint16_t *map = wad_screen_block(MAP_BLOCK);
    for (int i = 0; i < 1024; i++) {
        int col = i % 32, row = i / 32;
        map[i] = wad_screen_entry((col / 2 + row / 2) % 2 == 0 ? 1 : 2, 0);
    }

    /* A 16×16 star in four 1D-mapped sprite tiles. */
    wad_obj_palette[1] = wad_rgb(255, 216, 64);
    wad_obj_palette[2] = wad_rgb(232, 96, 32);
    for (int32_t y = 0; y < 16; y++) {
        for (int32_t x = 0; x < 16; x++) {
            int32_t dx = abs32(x * 2 - 15), dy = abs32(y * 2 - 15);
            int32_t c = 0;
            if (dx + dy < 14 || (min32(dx, dy) < 3 && max32(dx, dy) < 15)) c = dx + dy < 7 ? 2 : 1;
            int32_t tile = y / 8 * 2 + x / 8;
            int32_t at = (y % 8) * 4 + (x % 8) / 2;
            wad_obj_tiles[tile][at] |= (uint8_t)(c << (4 * (x % 2)));
        }
    }

    WAD_DISPCNT = WAD_MODE0 | WAD_DISP_BG0 | WAD_DISP_OBJ | WAD_OBJ_1D;
    WAD_BLDCNT = wad_blend_cnt(WAD_BLEND_ALPHA, WAD_LAYER_BG0, WAD_LAYER_BACKDROP);
    WAD_BLDALPHA = wad_blend_alpha(6, 10);
    WAD_MOSAIC = 0x3333;
}

WAD_EXPORT(update) void update(void) {
    wad_buttons_update(&state.buttons);
    int32_t speed = wad_held(&state.buttons, WAD_KEY_B) ? 3 : 1;
    if (wad_held(&state.buttons, WAD_KEY_LEFT)) state.x -= speed;
    if (wad_held(&state.buttons, WAD_KEY_RIGHT)) state.x += speed;
    if (wad_held(&state.buttons, WAD_KEY_UP)) state.y -= speed;
    if (wad_held(&state.buttons, WAD_KEY_DOWN)) state.y += speed;
    state.x = clamp32(state.x, 0, 240 - 32);
    state.y = clamp32(state.y, 0, 160 - 32);
    if (wad_pressed(&state.buttons, WAD_KEY_A)) state.mosaic = !state.mosaic;

    int32_t t = (int32_t)wad_frame();
    WAD_BGHOFS(0) = (uint16_t)(t / 2);
    WAD_BGVOFS(0) = (uint16_t)(t / 3);
    uint16_t cnt = WAD_BG_SCREEN_BLOCK(MAP_BLOCK) | WAD_BG_PRIORITY(1);
    WAD_BGCNT(0) = state.mosaic ? (uint16_t)(cnt | WAD_BG_MOSAIC) : cnt;

    /* Spin a quarter turn every 32 frames while the size pulses ±25%. */
    int32_t scale = 256 + ((wad_sin(t * 3) * 64) >> 14);
    wad_oam_set_affine(0, wad_affine_rotate_scale(t * 2, scale, scale));
    wad_sprite star = wad_sprite_affine(wad_sprite_new(state.x, state.y, WAD_S16x16, 0), 0, true);
    wad_oam_set(0, state.mosaic ? wad_sprite_mosaic(star) : star);
}

/* Raster effect: a new backdrop color on every line. */
static uint8_t lerp(int32_t a, int32_t b, int32_t y) { return (uint8_t)(a + (b - a) * y / 159); }

WAD_EXPORT(scanline) void scanline(int32_t y) {
    wad_bg_palette[0] = wad_rgb(lerp(40, 255, y), lerp(60, 140, y), lerp(160, 96, y));
}
