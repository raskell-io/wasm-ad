# WASM-AD SDK for C and C++

[`wasm_ad.h`](wasm_ad.h) is the whole SDK: register names from SPEC.md, bit
constants, sprite and affine helpers, input, save RAM and `trace`. It needs no
library and no libc, and compiles as C11 or C++17.

## Build the starter

You need clang with the WebAssembly backend and `wasm-ld` (LLVM's lld). Apple's
clang has no WebAssembly backend; on macOS, `brew install llvm lld`. The
Makefile finds Homebrew's LLVM on its own.

```sh
make            # build/starter.wasm (C) and build/starter-cpp.wasm (C++)
node ../../tools/wasmad.mjs check build/starter.wasm
```

[`examples/starter.c`](examples/starter.c) is the same cart as every other
SDK's starter and renders bit-identical frames. Drop the `.wasm` into the player.

## A new cart

```c
#include "wasm_ad.h"

WAD_CART(WAD_TITLE("My Cart") WAD_AUTHOR("you") WAD_VERSION("0.1.0"));

static int32_t x = 100; /* state: ordinary C variables, in linear memory */

WAD_EXPORT(start) void start(void) {
    wad_bg_palette[0] = wad_rgb(40, 60, 160);
    WAD_DISPCNT = WAD_MODE0 | WAD_DISP_BG0;
}

WAD_EXPORT(update) void update(void) {
    if (WAD_KEYS & WAD_KEY_RIGHT) x++;
    WAD_BGHOFS(0) = (uint16_t)x;
}
```

```sh
clang --target=wasm32 -O2 -nostdlib -ffreestanding \
  -Wl,--no-entry -Wl,--global-base=163840 -Wl,-z,stack-size=65536 -Wl,--strip-debug \
  cart.c -o cart.wasm
```

| Flag | Why |
|---|---|
| `--target=wasm32 -nostdlib -ffreestanding` | No libc and no WASI; the only import is `env.trace` (§2.2) |
| `--no-entry` | A cart has no `main`; the host calls `start`, `update` and `scanline` |
| `--global-base=163840` | Data starts at 0x28000, above the hardware region (§4, §13) |
| `-z stack-size=65536` | A 64 KB stack, placed after the data |

wasm-ld places the stack after the data unless told `--stack-first`. LLVM 22's
wasm-ld also accepts `--no-stack-first`; older ones reject it, and it is not
needed with plain clang.

`WAD_CART` writes the `wasm-ad` metadata section (§2.3) with `spec` filled in,
through an assembler directive, so no post-processing is needed. Keep values
free of double quotes and backslashes.

Without libc, `memcpy`, `memset`, `printf` and `malloc` are not available.
clang lowers struct copies and zeroing to WebAssembly bulk-memory instructions,
which every host supports. Run `wasmad check` before sharing a cart.

## API

Registers are lvalues: `WAD_DISPCNT = …`, `uint16_t k = WAD_KEYS;`.

| | |
|---|---|
| Registers | `WAD_DISPCNT`, `WAD_BGCNT(n)`, `WAD_BGHOFS(n)`, `WAD_BGVOFS(n)`, `WAD_WIN0H` … `WAD_BLDY`, `WAD_KEYS`, `WAD_FRAME`, `WAD_SEED`, `WAD_SAVECNT`, `WAD_SAVESTAT`, `WAD_SPEC` |
| Bits | `WAD_MODE0` … `WAD_DISP_OBJ_WIN`, `WAD_LAYER_*`, `WAD_BLEND_*`, `wad_blend_cnt`, `wad_blend_alpha`, `WAD_KEY_*` |
| Backgrounds | `WAD_BG_PRIORITY(p) \| WAD_BG_SCREEN_BLOCK(b) \| …`, `wad_screen_entry(tile, bank)`, `wad_screen_block(n)`, `wad_bg_tiles(block)`, `wad_vram` |
| Colors | `wad_rgb(r, g, b)`, `wad_bg_palette[i]`, `wad_obj_palette[i]` |
| Sprites | `wad_sprite_new(x, y, WAD_S16x16, tile)`, then `wad_sprite_bank`, `_priority`, `_hflip`, `_vflip`, `_semi_transparent`, `_window`, `_mosaic`, `_8bpp`, `_affine`; `wad_oam_set`, `wad_oam_hide`, `wad_oam_hide_all`, `wad_oam_set_affine`, `wad_obj_tiles` |
| Math | `wad_sin`, `wad_cos` (angle in 1/256 turns, result in Q14), `wad_affine_rotate_scale(angle, sx, sy)`: the same tables and rounding as every other SDK |
| Input | `wad_buttons` with `wad_buttons_update`, `wad_held`, `wad_pressed` |
| System | `wad_frame()`, `wad_seed()`, `wad_save_ram`, `wad_save_restored()`, `wad_request_save()`, `wad_trace(msg)` |
| Exports and metadata | `WAD_EXPORT(name)`, `WAD_CART(...)` with `WAD_TITLE`, `WAD_AUTHOR`, `WAD_VERSION`, `WAD_DESCRIPTION`, `WAD_URL`, `WAD_META(key, value)` |
