# Host conformance suite

Test carts with known output, for checking that a WASM-AD host (an
emulator, a player, a port to another platform) follows [SPEC.md](../SPEC.md).
**A host implements spec 0.3 when it reproduces every frame and sound hash in
[`expected.json`](expected.json).**

```sh
make host-test                                        # build the carts, check the reference host
node conformance/run.mjs --host conformance/hosts/web.mjs   # check the browser host (player/web)
```

## Tests

Each test is a cart in [`wasm/`](wasm) (source in [`carts/`](carts)) played
from a replay in [`replays/`](replays) (FORMATS.md §2). The replays are not
tied to a cart, so they only carry the seed, the save RAM at boot and the
buttons for each frame. Most carts run a script of 30-frame scenes keyed off
`FRAME`; the tests hash frames 0, 11 and 29 of each scene, and the sound of
every frame.

| Test | Covers (SPEC.md) |
|---|---|
| `tiles` | Four tiled layers and their order (§10.2), flips and palette banks, 8bpp tiles, every map size with 9-bit scroll wrap, mosaic, tile data past VRAM 0x10000 (§6.1, §6.2, §6.4, §8.1, §8.2) |
| `effects` | Alpha with coefficients past 16, brighten and darken, WIN0 and WIN1 with wrap-around bounds, the sprite window, semi-transparent sprites, the backdrop as a target, forced blank (§6.3, §6.5, §9.1, §10) |
| `sprites` | All twelve sizes, 1D and 2D mapping, 4bpp banks and 8bpp, flips, priority and overlap against a background, affine and double-size, coordinate wrap, sprite mosaic, hidden encodings, tile indices wrapping at 1024 (§9) |
| `bitmap` | Modes 3, 4 and 5, page flips, scroll and mosaic on BG2, index 0 transparent, the area outside a mode 5 bitmap, low sprite tiles hidden (§8.3) |
| `affine` | Modes 1 and 2, clipping and wrap at every map size, shear, mosaic, negative reference points and the ignored bits 28–31, a "mode 7" floor from `scanline`, a map running past VRAM 0x10000 (§6.1, §6.2, §8.4) |
| `sound` | Square duties, envelopes up and down, live rate changes, sweep up to overflow and down, length, retriggering, the wave channel with one and two banks, every volume and the DAC off, 15- and 7-bit noise with divider 0 and a stopped shift, routing, side and PSG volumes, sound switched off and on. Draws `SOUNDCNT_X`, so status is checked too (§14.1–14.5, §14.7, §14.8) |
| `samples` | One-shots at several rates and volumes, 50% and side routing, loops with a moving loop point, pitch slides, the stop bit, streaming through a ring buffer that wraps sample RAM, and edge cases: length 0, a loop point past the end, rate 0, start and stop together, volume above 64. Draws `PCMx_POS` (§14.6, §14.7) |
| `system` | The boot state (§3.1), `FRAME`, `SEED`, four players' `KEYS` with `PADS` changing, `VCOUNT` in `update` and in `scanline`, a per-line backdrop, `SAVECNT` (§3.2, §5, §11) |
| `system-saved` | The same cart booted with save RAM and another seed: `SAVESTAT` and restored save RAM (§3.1, §11) |

The carts draw what they read: the system cart shows its boot checks and
every register as rows of squares, and traces the boot checks too, so a
failing hash can be narrowed down by looking at the frame.

## Hashes

- **Frame**: SHA-256 of the 240×160 frame as RGBA8 bytes, rows top to bottom,
  alpha 255, after the frame has finished (SPEC.md §3.2). The color conversion
  is SPEC.md §7's.
- **Sound**: SHA-256 of the sound of every frame of the test, in order, as
  16-bit little-endian stereo pairs, left first: 800 pairs a frame (§14).

Both are the first 16 hex digits.

## Checking another host

Write an adapter module that exports one function:

```js
// createHost may be async. cart is the .wasm bytes; seed is SEED; save is
// 32 KB of save RAM to restore at boot (SAVESTAT bit 0 set), or null.
export async function createHost(cart, { seed, save }) {
  // ...boot the cart (SPEC.md §3.1)...
  return {
    // Runs one frame (§3.2) with input = [KEYS, KEYS2, KEYS3, KEYS4, PADS]
    // and returns its pixels as RGBA8 (a Uint8Array of 240 × 160 × 4).
    frame(input) { /* ... */ },
    // The frame's sound: 1600 values, an Int16Array, left then right.
    get sound() { /* ... */ },
  };
}
```

then run `node conformance/run.mjs --host path/to/adapter.mjs`.
[`hosts/web.mjs`](hosts/web.mjs) is the adapter for the browser host. A host
that isn't written in JavaScript can instead play each replay itself and
compare against `expected.json` directly: everything it needs is in the
replay, the cart and the hash definitions above.

## Changing the suite

[`tests.json`](tests.json) lists the tests: the cart, the frames to run, the
buttons (in the `wasmad run --keys` syntax) and, where it matters, the
seed, `PADS` and boot save RAM. After changing a cart or a test:

```sh
make host-test                           # rebuild the carts into wasm/
node conformance/run.mjs --update        # rewrite replays/ and expected.json with the reference
```

Review the diff of `expected.json` before committing it: an update makes the
reference core's output the expectation, so a changed hash must come from a
deliberate change to a cart or to the spec.
