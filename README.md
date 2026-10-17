# WASM-AD

A fantasy console with Game Boy Advance–style hardware, whose cartridges are
WebAssembly modules.

A cart is one `.wasm` file. It exports `update()`, which the console calls
60 times a second, and draws by writing registers, palettes, tiles, maps and
sprites into fixed addresses of its own linear memory, laid out like the
GBA's. The console turns that memory into a 240×160 frame. Any language that
compiles to WebAssembly can make a cart without a library; this repo ships a
Rust SDK.

**[Read the specification →](SPEC.md)** (v0.1 draft)

| | |
|---|---|
| Display | 240×160, 60 fps, RGB555 |
| Graphics | Mode 0: four tiled backgrounds; modes 3–5: bitmaps |
| Tiles | 8×8 at 4bpp (16 banks × 16 colors) or 8bpp (256 colors) |
| Sprites | 128, 8×8 to 64×64, rotation and scaling |
| Effects | alpha blending, fades, windows, mosaic, per-scanline raster effects |
| Input | D-pad, A, B, L, R, Start, Select |
| Save | 32 KB battery-backed save RAM |
| Determinism | no clock, one boot seed: replays, save states and netplay come free |

## Try it

```sh
make serve     # http://localhost:8643/?cart=carts/hello.wasm
```

The player at `player/web` loads any cart by file picker, drag and drop, or
`?cart=<url>`. Keys: arrows, X = A, Z = B, A = L, S = R, Enter = Start,
Shift = Select. Gamepads work too.

## Writing a cart in Rust

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
`carts/hello` for a complete example.

## Repository

```
SPEC.md        the standard
core/          wasm-ad-core: reference renderer, written section by section from the spec
sdk/           wasm-ad: Rust SDK for carts, with a native test host
player/web/    browser player (host side of the spec)
carts/hello/   example cart: raster gradient, blending, affine sprite, mosaic
```

Carts in the wild: [Rainbow River](https://github.com/raskell-io/rainbow-river).

## Status

v0.1 covers video, input and save. Audio (four tone/noise channels and two
PCM channels) is planned for 0.2, along with affine backgrounds (modes 1 and
2). The spec is a draft: expect breaking changes before 1.0.

WASM-AD is not affiliated with Nintendo.

## License

MIT
