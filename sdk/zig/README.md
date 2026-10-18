# WASM-AD Zig SDK

Write [WASM-AD](../../README.md) carts in Zig 0.16. The SDK wraps the
memory-mapped hardware in typed helpers and its `addCart` build helper sets the
linker options a cart needs.

## Build the starter

```sh
zig build            # zig-out/bin/starter.wasm (≈2.5 KB, ReleaseSmall)
zig build test       # unit tests for the SDK's pure helpers
```

Drop the `.wasm` into the player, or check it from the repository root:

```sh
node tools/wasmad.mjs check sdk/zig/zig-out/bin/starter.wasm
```

[`examples/starter.zig`](examples/starter.zig) is the cart every SDK ships, a
rotating affine checkerboard, a spinning star and a tune on every sound
channel: it renders bit-identical frames and sound to the Rust one
(`make sdk-test`).

## A new cart

Zig fetches packages from a repository's root, and this one lives in
`sdk/zig`, so add it as a path dependency to a checkout (or a git submodule,
or a copy of this directory) in `build.zig.zon`:

```zig
.dependencies = .{
    .wasm_ad = .{ .path = "../wasm-ad/sdk/zig" }, // relative to your build root
},
```

Then in `build.zig`:

```zig
const std = @import("std");
const wasm_ad = @import("wasm_ad");

pub fn build(b: *std.Build) void {
    const dep = b.dependency("wasm_ad", .{});
    b.installArtifact(wasm_ad.addCart(b, .{
        .name = "mygame",
        .root_source_file = b.path("src/main.zig"),
        .wasm_ad = dep.module("wasm_ad"),
    }));
}
```

and `src/main.zig`:

```zig
const ad = @import("wasm_ad");

comptime {
    ad.cart(.{ .title = "My Game", .author = "you", .version = "0.1.0" });
}

var x: i32 = 0; // state lives in linear memory

export fn start() void {
    ad.bgPalette()[0] = ad.rgb(40, 60, 160);
}

export fn update() void {
    if (ad.keys() & ad.key.RIGHT != 0) x += 1;
}
```

`update` is required; `start` and `scanline(y: i32)` are optional.

## What `addCart` sets, and why

SPEC.md §4 and §13 reserve linear memory below `0x28000` for the hardware, so
the cart's data and stack must sit above it:

| Setting | Value |
|---|---|
| target | `wasm32-freestanding` |
| `global_base` | `0x28000`: data starts above the hardware region |
| `stack_size` | 64 KiB, placed after the data (Zig does not put the stack first) |
| `initial_memory` | 4 pages (the spec requires at least 3) |
| `entry` | disabled; `rdynamic` exports your `export fn`s |

Keep game state in container-level `var`s: they are linear memory, so hosts
can snapshot them (§12). The SDK never adds WebAssembly globals beyond the
stack pointer.

## Metadata

`ad.cart(.{ ... })` writes the `wasm-ad` custom section (§2.3) at compile time:
`spec=0.2` first, then each field as `key=value`. `title` is required; values
must be one line. Zig has no direct way to emit a custom section, so the SDK
uses global assembly with a `.custom_section.wasm-ad` section, which LLVM's
wasm assembler turns into one. No post-build step is needed.

## Sound

`sound.enable()` turns sound on with every channel on both sides. Each channel
is driven through its registers, with builders for the fields (SPEC.md §14):

```zig
ad.sound.enable();
ad.reg.SOUND2CNT_L.write(ad.sound.DUTY_50 | ad.sound.envelope(12, 2, false));
ad.reg.SOUND2CNT_H.write(ad.sound.START | ad.sound.rate(440)); // 440 Hz
```

The wave channel plays 4-bit samples from `sound.waveRam()`; the two sample
channels play signed 8-bit samples you write into `sound.sampleRam()`, with
`pcm.play(ch, start, len, rate_hz, volume)`.

## Affine backgrounds

In modes 1 and 2, BG2 (and in mode 2 BG3) are affine: an 8bpp map of one byte
per tile, drawn through a matrix (SPEC.md §8.4).

```zig
ad.reg.DISPCNT.write(ad.dispcnt.MODE1 | ad.dispcnt.BG2);
ad.reg.BGCNT[2].write((ad.BgCnt{}).screenBlock(8).affineSize(.s32x32).wrap().bits);
// Texture pixel (128, 128) at the screen centre, rotated.
ad.setBgAffine(2, .rotateScaleFixed(angle, 256, 256), 128, 128, 120, 80);
```

## API

| | |
|---|---|
| `reg.DISPCNT`, `reg.BGCNT[i]`, `reg.BGHOFS[i]`, … | Registers by their SPEC.md names; `.read()`, `.write(v)` |
| `dispcnt`, `layer`, `blend`, `key` | Bit constants; `blend.cnt(effect, first, second)`, `blend.alpha(eva, evb)` |
| `BgCnt` | `BGxCNT` builder: `(BgCnt{}).screenBlock(31).priority(1).bits` |
| `rgb(r, g, b)` | 8-bit channels to RGB555 |
| `bgPalette()`, `objPalette()` | `*[256]u16` |
| `vram()`, `bgTiles(block)`, `objTiles()`, `screenBlock(n)` | VRAM views; `Tile4` is `[32]u8`, `ScreenEntry.init(tile, bank)` |
| `bgTiles8(block)`, `affineMap(block)` | 8bpp tiles (`Tile8` is `[64]u8`) and affine maps, one byte per tile |
| `BgCnt.affineSize(size)`, `.wrap()` | Affine map size and wrapping |
| `setBgAffine(bg, m, tex_x, tex_y, scr_x, scr_y)` | Affine background matrix, with texture pixel (tex_x, tex_y) at screen (scr_x, scr_y) |
| `Sprite.init(x, y, size, tile)` | OAM attributes: `.bank()`, `.priority()`, `.hflip()`, `.affine(slot, double)`, … |
| `oam.set(i, sprite)`, `oam.hide(i)`, `oam.setAffine(slot, m)` | OAM |
| `AffineMatrix.rotateScale(rad, sx, sy)` | Float; `.rotateScaleFixed(angle, sx, sy)` is the integer version every SDK shares |
| `fixed.sin(a)`, `fixed.cos(a)` | Angle in 1/256 turns, result in Q14 |
| `Buttons` | `.update()` each frame, then `.held(k)`, `.pressed(k)` |
| `keys()`, `frame()`, `seed()` | System registers |
| `saveRam()`, `saveRestored()`, `requestSave()` | 32 KB save RAM (§11) |
| `trace(msg)` | Host debug log; imports `env.trace` only if you call it |
| `reg.SOUND1CNT_L` … `reg.SOUNDCNT_X`, `reg.PCMA_*`, `reg.PCMB_*` | Sound registers |
| `sound.enable()`, `sound.disable()`, `sound.status()` | Master control; status bits 0–5 are channels 1–4, A, B |
| `sound.rate(hz)`, `.sweep(time, shift, down)`, `.envelope(vol, step, up)`, `.length(n)`, `.noise(div, shift, short)` | Field builders; `START`, `LENGTH`, `DUTY_*`, `WAVE_*` constants |
| `sound.waveRam()`, `sound.sampleRam()` | 32 bytes of wave RAM, 28 KB of sample RAM |
| `pcm.play(ch, start, len, rate, vol)`, `.playLooped(…, loop_at, …)`, `.stop(ch)`, `.position(ch)`, `.playing(ch)` | Sample channels A (0) and B (1) |
