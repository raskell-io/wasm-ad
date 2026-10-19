/*
 * The starter cart, the same in every SDK: a rotating checkerboard (an affine
 * background) blended over a backdrop gradient drawn per scanline, a spinning
 * star you move with the D-pad, and a little tune on all four sound channels.
 * B goes faster and plays a sample; A toggles mosaic with a rising blip. A
 * second controller brings in a blue star for player 2. It uses integer math
 * only, so every language's build renders bit-identical frames and sound.
 *
 * Also valid C++; the Makefile builds it both ways.
 */
#include "../wasm_ad.h"

WAD_CART(WAD_TITLE("Starter") WAD_AUTHOR("raskell.io") WAD_DESCRIPTION("The WASM-AD starter cart, in C"));

#define MAP_BLOCK 8
/* The zap sample's length in sample RAM. */
#define ZAP_LEN 2048
/* An arpeggio for the square channel and a bass line for the wave channel, in Hz. */
static const uint32_t NOTES[8] = {262, 330, 392, 523, 392, 330, 262, 196};
static const uint32_t BASS[4] = {65, 55, 44, 49};

/* Cart state lives in linear memory, never in WebAssembly globals (SPEC.md §12). */
static struct {
    wad_buttons buttons;
    int32_t x, y;
    bool mosaic;
    wad_buttons p2;
    int32_t x2, y2;
} state = {{0, 0}, 112, 72, false, {0, 0}, 40, 72};

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

    /* A checkerboard of two solid 8bpp tiles on affine background 2, 32×32 tiles. */
    wad_bg_palette[1] = wad_rgb(255, 255, 255);
    wad_bg_palette[2] = wad_rgb(40, 40, 72);
    volatile wad_tile8 *tiles = wad_bg_tiles8(0);
    for (int i = 0; i < 64; i++) {
        tiles[1][i] = 1;
        tiles[2][i] = 2;
    }
    volatile uint8_t *map = wad_affine_map(MAP_BLOCK);
    for (int i = 0; i < 32 * 32; i++) {
        int col = i % 32, row = i / 32;
        map[i] = (col / 2 + row / 2) % 2 == 0 ? 1 : 2;
    }

    /* A 16×16 star in four 1D-mapped sprite tiles. */
    wad_obj_palette[1] = wad_rgb(255, 216, 64);
    wad_obj_palette[2] = wad_rgb(232, 96, 32);
    /* Player 2's star: the same tiles in palette bank 1. */
    wad_obj_palette[17] = wad_rgb(96, 200, 255);
    wad_obj_palette[18] = wad_rgb(40, 80, 220);
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

    WAD_DISPCNT = WAD_MODE1 | WAD_DISP_BG2 | WAD_DISP_OBJ | WAD_OBJ_1D;
    WAD_BLDCNT = wad_blend_cnt(WAD_BLEND_ALPHA, WAD_LAYER_BG2, WAD_LAYER_BACKDROP);
    WAD_BLDALPHA = wad_blend_alpha(6, 10);
    WAD_MOSAIC = 0x3333;

    /* A triangle wave for the bass, and a falling zap in sample RAM. */
    for (int i = 0; i < 16; i++) {
        int a = i * 2, b = i * 2 + 1;
        uint8_t la = (uint8_t)(a < 16 ? a : 31 - a), lb = (uint8_t)(b < 16 ? b : 31 - b);
        wad_wave_ram[i] = (uint8_t)((la << 4) | lb);
    }
    for (int32_t i = 0; i < ZAP_LEN; i++) {
        int32_t s = (wad_sin((i * i) >> 9) * 127) >> 14;
        wad_sample_ram[i] = (int8_t)((s * (ZAP_LEN - i)) >> 11);
    }
    wad_sound_enable();
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
    if (wad_pressed(&state.buttons, WAD_KEY_A)) {
        state.mosaic = !state.mosaic;
        /* A rising blip: the sweep raises the pitch until it runs out. */
        WAD_SOUND1CNT_L = wad_sound_sweep(2, 2, false);
        WAD_SOUND1CNT_H = WAD_DUTY_50 | wad_sound_envelope(12, 2, false);
        WAD_SOUND1CNT_X = WAD_SOUND_START | wad_sound_rate(440);
    }
    if (wad_pressed(&state.buttons, WAD_KEY_B)) wad_pcm_play(0, 0, ZAP_LEN, 16000, 48);

    int32_t t = (int32_t)wad_frame();
    /* The tune: a note every 16 frames, a hi-hat between them, bass every 64. */
    if (t % 16 == 0) {
        WAD_SOUND2CNT_L = WAD_DUTY_25 | wad_sound_envelope(9, 1, false);
        WAD_SOUND2CNT_H = WAD_SOUND_START | wad_sound_rate(NOTES[t / 16 % 8]);
    }
    if (t % 16 == 8) {
        WAD_SOUND4CNT_L = wad_sound_envelope(6, 1, false);
        WAD_SOUND4CNT_H = WAD_SOUND_START | wad_sound_noise(1, 1, false);
    }
    if (t % 64 == 0) {
        WAD_SOUND3CNT_L = WAD_WAVE_ON;
        WAD_SOUND3CNT_H = WAD_WAVE_50 | (256 - 48);
        WAD_SOUND3CNT_X = WAD_SOUND_START | WAD_SOUND_LENGTH | wad_sound_rate(BASS[t / 64 % 4] * 2);
    }

    /* Turn once every 1024 frames around the screen centre, scrolling diagonally. */
    uint16_t cnt = WAD_BG_SCREEN_BLOCK(MAP_BLOCK) | WAD_BG_AFFINE_32x32 | WAD_BG_WRAP | WAD_BG_PRIORITY(1);
    WAD_BGCNT(2) = state.mosaic ? (uint16_t)(cnt | WAD_BG_MOSAIC) : cnt;
    wad_affine m = wad_affine_rotate_scale(t / 4, 256, 256);
    wad_set_bg_affine(2, m, (t / 2) % 256, (t / 3) % 256, 120, 80);

    /* Spin a quarter turn every 32 frames while the size pulses ±25%. */
    int32_t scale = 256 + ((wad_sin(t * 3) * 64) >> 14);
    wad_oam_set_affine(0, wad_affine_rotate_scale(t * 2, scale, scale));
    wad_sprite star = wad_sprite_affine(wad_sprite_new(state.x, state.y, WAD_S16x16, 0), 0, true);
    wad_oam_set(0, state.mosaic ? wad_sprite_mosaic(star) : star);

    /* Player 2 joins when a second controller connects (SPEC.md §5). */
    if (wad_connected(2)) {
        wad_buttons_update_for(&state.p2, 2);
        if (wad_held(&state.p2, WAD_KEY_LEFT)) state.x2 -= 1;
        if (wad_held(&state.p2, WAD_KEY_RIGHT)) state.x2 += 1;
        if (wad_held(&state.p2, WAD_KEY_UP)) state.y2 -= 1;
        if (wad_held(&state.p2, WAD_KEY_DOWN)) state.y2 += 1;
        state.x2 = clamp32(state.x2, 0, 240 - 32);
        state.y2 = clamp32(state.y2, 0, 160 - 32);
        wad_oam_set(1, wad_sprite_affine(wad_sprite_bank(wad_sprite_new(state.x2, state.y2, WAD_S16x16, 0), 1), 0, true));
    } else {
        wad_oam_hide(1);
    }
}

/* Raster effect: a new backdrop color on every line. */
static uint8_t lerp(int32_t a, int32_t b, int32_t y) { return (uint8_t)(a + (b - a) * y / 159); }

WAD_EXPORT(scanline) void scanline(int32_t y) {
    wad_bg_palette[0] = wad_rgb(lerp(40, 255, y), lerp(60, 140, y), lerp(160, 96, y));
}
