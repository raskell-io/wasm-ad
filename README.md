<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/wordmark-dark.png">
    <img src="assets/brand/wordmark-light.png" alt="WASM-AD" width="360">
  </picture>
</p>

<p align="center"><b>A fantasy handheld with Game Boy Advance–style hardware. Carts are WebAssembly.</b></p>

<p align="center">
  <a href="https://wasmad.raskell.io"><b>Play in your browser</b></a> ·
  <a href="SPEC.md">Specification</a> ·
  <a href="#writing-a-cart-in-rust">Write a cart</a> ·
  <a href="https://github.com/raskell-io/rainbow-river">Rainbow River</a>
</p>

A cart is one `.wasm` file. It exports `update()`, which the console calls
60 times a second, and draws and plays sound by writing registers, palettes,
tiles, maps, sprites and samples into fixed addresses of its own linear
memory, laid out like the GBA's. The console turns that memory into a
240×160 frame and 800 samples of stereo sound. Any language that
compiles to WebAssembly can make a cart without a library; this repo ships
SDKs for Rust, C and C++, Zig, AssemblyScript and Go.

**[Read the specification →](SPEC.md)** (v0.2 draft)

| | |
|---|---|
| Display | 240×160, 60 fps, RGB555 |
| Graphics | Modes 0–2: up to four tiled backgrounds, two of them rotating and scaling; modes 3–5: bitmaps |
| Tiles | 8×8 at 4bpp (16 banks × 16 colors) or 8bpp (256 colors) |
| Sprites | 128, 8×8 to 64×64, rotation and scaling |
| Effects | alpha blending, fades, windows, mosaic, per-scanline raster effects |
| Sound | 48 kHz stereo: two square waves, a wave channel, noise, two 8-bit sample channels |
| Input | D-pad, A, B, L, R, Start, Select |
| Save | 32 KB battery-backed save RAM |
| Determinism | no clock, one boot seed, integer sound: replays, save states and netplay come free |

## Try it

Play at **[wasmad.raskell.io](https://wasmad.raskell.io)**
([source](https://github.com/raskell-io/wasmad.raskell.io)), or locally:

```sh
make serve  # the player: http://localhost:8643/?cart=carts/twirl.wasm
```

The player at `player/web` loads any cart by file picker, drag and drop, or
`?cart=<url>`. Keys: arrows, X = A, Z = B, A = L, S = R, Enter = Start,
Shift = Select. Gamepads work too.

## Writing a cart

| Language | SDK | Get it | Toolchain |
|---|---|---|---|
| Rust | [`sdk/rust`](sdk/rust), below | `wasm-ad = { git = "https://github.com/raskell-io/wasm-ad" }` | `rustup target add wasm32-unknown-unknown` |
| C, C++ | [`sdk/c`](sdk/c): one header | Copy `sdk/c/wasm_ad.h` | clang with the WebAssembly backend, wasm-ld |
| Zig | [`sdk/zig`](sdk/zig) | Path dependency on a checkout or submodule | Zig 0.16 |
| AssemblyScript | [`sdk/assemblyscript`](sdk/assemblyscript) | `npm install @raskell/wasm-ad` ([npm](https://www.npmjs.com/package/@raskell/wasm-ad)) | `assemblyscript` 0.28 from npm |
| Go | [`sdk/tinygo`](sdk/tinygo) | `github.com/raskell-io/wasm-ad/sdk/tinygo@v0.2.0` | TinyGo 0.43 |

Every SDK has the same starter cart, which renders bit-identical frames in
all of them (`make sdk-test`), and the same fixed-point trigonometry, so cart
math is deterministic in any language (SPEC.md §12). Each SDK's README covers
building the starter and starting a new cart.

Sound is four GBA-style channels and two sample channels, driven through
registers like everything else (SPEC.md §14):

```rust
sound::enable();
reg::SOUND2CNT_L.write(sound::DUTY_50 | sound::envelope(12, 2, false)); // fade out
reg::SOUND2CNT_H.write(sound::START | sound::rate(440));                // play A4
pcm::play(0, 0, 2048, 16000, 64); // 2048 samples from sample RAM at 16 kHz
```

### Rust

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-ad = { git = "https://github.com/raskell-io/wasm-ad" }
```

```toml
# .cargo/config.toml: keep data and stack above the hardware region (SPEC.md §13)
[target.wasm32-unknown-unknown]
rustflags = ["-C", "link-arg=--no-stack-first", "-C", "link-arg=--global-base=163840", "-C", "link-arg=-zstack-size=65536"]
```

```rust
use wasm_ad::*;

wasm_ad::cart! { title: "My Cart", author: "me" }

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    bg_palette()[0] = rgb(40, 60, 160);   // backdrop
    obj_palette()[1] = rgb(255, 216, 64);
    obj_tiles()[0] = [0x11; 32];          // a solid 8×8 tile
    reg::DISPCNT.write(dispcnt::MODE0 | dispcnt::OBJ | dispcnt::OBJ_1D);
}

#[unsafe(no_mangle)]
pub extern "C" fn update() {
    let x = (frame() % 240) as i32;
    oam::set(0, Sprite::new(x, 76, SpriteSize::S8x8, 0));
}
```

```sh
cargo build --release --target wasm32-unknown-unknown
```

Off wasm32 the SDK emulates the hardware per thread, so cart logic can be
unit-tested with `cargo test` and frames rendered with `wasm_ad::host`. See
`carts/hello` for a complete example, or build the starter with
`cargo build --release --target wasm32-unknown-unknown -p wasm-ad --example starter`;
its sound comes from `wasm_ad::host::sound()` the same way.

### Checking a cart

`tools/wasmad.mjs` needs only Node 18+ and works on carts from any language:

```sh
node tools/wasmad.mjs check cart.wasm                 # validate against SPEC.md
node tools/wasmad.mjs meta cart.wasm title="My Cart"  # add the metadata section
node tools/wasmad.mjs run cart.wasm --frames 120 --keys 0-59:right --ppm out.ppm --wav out.wav
```

`check` rejects what a host would refuse and what the spec forbids: other
imports, missing exports, data or stack in the hardware region, state in
mutable globals, a start section, missing metadata. `meta` is for toolchains
that cannot write custom sections. `run` renders frames and sound headlessly
with the reference core (`make player` first).

## Repository

```
SPEC.md        the standard
core/          wasm-ad-core: reference renderer and sound, written section by section from the spec
sdk/rust/      wasm-ad: Rust SDK for carts, with a native test host
sdk/c/         C and C++ SDK: wasm_ad.h
sdk/zig/       Zig SDK
sdk/assemblyscript/  AssemblyScript SDK
sdk/tinygo/    Go SDK, for TinyGo
tools/         wasmad.mjs: check, add metadata to, and run carts from the command line
player/web/    browser player; wasm-ad.js is the embeddable console
carts/twirl/   a spinning-stick maze game: affine sprite, parallax, blending, save RAM
carts/hello/   example cart: raster gradient, blending, affine sprite, mosaic
```

Carts in the wild: [Rainbow River](https://github.com/raskell-io/rainbow-river).

## Status

v0.2 covers video (all six modes), sound, input and save. The spec is a
draft: expect breaking changes before 1.0.

WASM-AD is not affiliated with Nintendo.

## License

MIT
