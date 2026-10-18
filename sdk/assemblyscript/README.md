# WASM-AD for AssemblyScript

An AssemblyScript SDK for [WASM-AD](https://github.com/raskell-io/wasm-ad) carts: register
addresses, typed builders for backgrounds (tiled and affine), sprites, affine matrices and
sound, input, save RAM and `trace`, with no runtime allocation and no mutable globals.

## The starter cart

```sh
npm install
npm run build            # build/starter.wasm
node ../../tools/wasmad.mjs check build/starter.wasm
```

[`examples/starter.ts`](examples/starter.ts) is the same cart as every other SDK's starter: a
rotating affine checkerboard, a spinning star and a little tune on every sound channel, with
bit-identical frames and sound. Drop `build/starter.wasm` into the player.

## A new cart

```sh
npm install @raskell/wasm-ad assemblyscript@0.28
echo '{ "extends": "./node_modules/@raskell/wasm-ad/cart.asconfig.json" }' > asconfig.json
```

```ts
import { write16, reg, dispcnt, keys, key } from "@raskell/wasm-ad/assembly";
```

```sh
npx asc cart.ts --config asconfig.json -o cart.wasm
npx wasmad meta cart.wasm title="My Cart" author=you
npx wasmad check cart.wasm
```

`cart.asconfig.json` is the configuration below with `abort` pointing into the
installed package; extend it rather than copying it. `wasmad` is the cart
checker from `tools/wasmad.mjs`, shipped with this package.

`asconfig.json` sets what the spec requires:

| Option | Why |
|---|---|
| `memoryBase: 163840` | Static data starts at 0x28000, above the hardware region (SPEC.md §4). |
| `initialMemory: 4` | At least 3 pages; 4 leaves room above the data. |
| `runtime: "stub"` | No garbage collector; carts do not allocate. |
| `use: abort=assembly/index/abort` | Traps instead of importing `env.abort`; a cart may only import `env.trace`. |

AssemblyScript cannot write custom sections, so `wasmad meta` adds the `wasm-ad` metadata
(`spec` is filled in). Run `wasmad check` before sharing a cart.

## Rules

A cart exports `update()`, and optionally `start()` and `scanline(y: i32)`. Do the setup in
`start`, not at the top level of a module, which would run before the host clears the hardware.

**Keep state in linear memory.** AssemblyScript compiles a module-level `let` or `var` to a
mutable WebAssembly global, which SPEC.md §12 forbids (hosts snapshot linear memory for save
states and netplay), and `wasmad check` rejects it. Put the cart's state in an
`@unmanaged` class placed in a static block of memory:

```ts
@unmanaged
class State {
  x: i32;
  y: i32;
}
const state = changetype<State>(memory.data(offsetof<State>()));

export function start(): void {
  state.x = 112; // memory starts zeroed; set initial values here
}
```

`const` values are fine. Avoid `new`, string concatenation, arrays and anything else that
allocates; use `memory.data` for static buffers and tables, e.g.
`const NOTES = memory.data<u32>([262, 330, 392])` read with `load<u32>(NOTES + (i << 2))`.

## Sound

`sound.enable()` turns sound on with every channel on both sides. A channel plays when its
start bit is written; the host clears it after the frame (SPEC.md §14):

```ts
write16(reg.SOUND2CNT_L, sound.DUTY_50 | sound.envelope(12, 2, false));
write16(reg.SOUND2CNT_H, sound.START | sound.rate(440));
```

Sample channels play signed 8-bit samples you write to `sound.sampleRam()`:
`pcm.play(0, offset, length, 16000, 64)`.

## Affine backgrounds

In modes 1 and 2, BG2 (and BG3 in mode 2) is drawn through a matrix. Its tiles are 8bpp
(`bgTile8`) and its map is one byte per tile (`affineMap`). `setBgAffine(2, m, tx, ty, sx, sy)`
puts texture pixel (tx, ty) at screen pixel (sx, sy) and rotates and scales around it.

## API

All values are plain integers; builders take a value and return a new one.

| | |
|---|---|
| `read16`, `write16`, `read32`, `write32` | Raw register access, e.g. `write16(reg.DISPCNT, dispcnt.MODE0 \| dispcnt.BG0)` |
| `reg.*`, `reg.BGCNT(i)`, `reg.BGHOFS(i)`, `reg.BGVOFS(i)` | Register addresses by their SPEC.md names |
| `dispcnt`, `layer`, `blend`, `key` | Bit constants; `blend.cnt(effect, first, second)`, `blend.alpha(eva, evb)` |
| `bgcnt.priority/charBlock/screenBlock/size/bpp8/mosaic/affineSize/wrap` | `BGxCNT` builders |
| `rgb(r, g, b)` | 8-bit channels to RGB555 |
| `setBgColor(i, c)`, `setObjColor(i, c)` | Palette RAM |
| `bgTile(block, i)`, `objTile(i)`, `fillTile(addr, byte)` | Addresses of 32-byte 4bpp tiles |
| `screenBlock(b)`, `setScreenEntry(b, i, e)`, `screenEntry.make/hflip/vflip` | Background maps |
| `bgTile8(block, i)`, `affineMap(b)`, `setBgAffine(bg, m, tx, ty, sx, sy)` | Affine backgrounds (modes 1 and 2) |
| `sprite.make(x, y, SpriteSize.S16x16, tile)` then `.bank/.priority/.hflip/.vflip/.semiTransparent/.window/.mosaic/.bpp8/.affine` | Sprites, packed in a `u64` |
| `oam.set(i, s)`, `oam.hide(i)`, `oam.hideAll()`, `oam.setAffine(slot, m)` | OAM |
| `affine.rotateScaleFixed(angle, sx, sy)`, `affine.IDENTITY`, `affine.make` | Affine matrices; angle in 1/256 turns, scale in 8.8 |
| `fixed.sin(a)`, `fixed.cos(a)` | Q14 trigonometry, identical in every SDK |
| `buttons.update()`, `buttons.held(k)`, `buttons.pressed(k)`, `keys()` | Input |
| `frame()`, `seed()` | Time and the only entropy source |
| `saveRam()`, `saveRestored()`, `requestSave()` | 32 KB of save RAM at 0x19000 |
| `sound.enable/disable`, `sound.rate(hz)`, `sound.sweep/envelope/length/noise`, `sound.START/LENGTH/DUTY_*/WAVE_*` | Sound channels 1–4, through `reg.SOUND*` |
| `sound.waveRam()`, `sound.sampleRam()`, `sound.status()` | Wave RAM, sample RAM, which channels are playing |
| `pcm.play(ch, start, len, rate, vol)`, `pcm.playLooped`, `pcm.stop`, `pcm.position`, `pcm.playing` | Sample channels A (0) and B (1) |
| `trace(msg)` | UTF-8 to the host's debug log (at most 1 KB) |
