# WASM-AD Specification, version 0.1 (draft)

WASM-AD is a fantasy console whose games are WebAssembly modules. Its video
hardware follows the Game Boy Advance: a 240×160 display composed from tiled
backgrounds and sprites, 15-bit color, palette banks, rotation and scaling,
alpha blending, windows and mosaic. A cart drives the hardware the way a GBA
cartridge did, by writing registers and video memory, except that this
memory lives inside the cart's own WebAssembly linear memory.

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119.

WASM-AD is not affiliated with Nintendo. Where this document says "as on
the GBA", the formats are bit-compatible with the GBA's, and GBATEK and Tonc
are useful further reading.

## Contents

1. [Overview](#1-overview)
2. [Cart format](#2-cart-format)
3. [Lifecycle and timing](#3-lifecycle-and-timing)
4. [Memory map](#4-memory-map)
5. [System registers](#5-system-registers)
6. [Display registers](#6-display-registers)
7. [Palette RAM](#7-palette-ram)
8. [VRAM: tiles, maps and bitmaps](#8-vram-tiles-maps-and-bitmaps)
9. [OAM: sprites](#9-oam-sprites)
10. [Composition](#10-composition)
11. [Save RAM](#11-save-ram)
12. [Determinism and save states](#12-determinism-and-save-states)
13. [Toolchain notes](#13-toolchain-notes)
14. [Reserved for future versions](#14-reserved-for-future-versions)

## 1. Overview

| | |
|---|---|
| Display | 240×160 pixels, 60 frames per second |
| Color | RGB555 (32,768 colors) |
| Palettes | 256 background + 256 sprite colors, as 16 banks of 16 or as one 256-color palette |
| Video modes | 0 (four tiled backgrounds), 3, 4 and 5 (bitmaps) |
| Tiles | 8×8, 4 or 8 bits per pixel; 64 KB background VRAM, 32 KB sprite VRAM |
| Sprites | 128, sizes 8×8 to 64×64, 32 affine matrices |
| Effects | alpha blending, brightness fades, two rectangular windows and a sprite window, mosaic |
| Raster effects | optional per-scanline callback |
| Input | 10 buttons: D-pad, A, B, L, R, Start, Select |
| Save | 32 KB battery-backed save RAM |
| Cart | one `.wasm` file, at most 32 MiB |

## 2. Cart format

A cart is a WebAssembly module (core specification 2.0; threads, exceptions,
GC and memory64 MUST NOT be used).

### 2.1 Exports

| Name | Type | |
|---|---|---|
| `memory` | memory | Required. Initial size at least 3 pages (192 KiB). |
| `update` | `() -> ()` | Required. Called once per frame. |
| `start` | `() -> ()` | Optional. Called once before the first `update`. |
| `scanline` | `(i32) -> ()` | Optional. Called before each visible line is drawn (§3.2). |

The cart MUST NOT use the WebAssembly start section to run game code, because
the host initializes the hardware region after instantiation.

### 2.2 Imports

A cart MAY import exactly one function, and nothing else:

| Module | Name | Type | |
|---|---|---|---|
| `env` | `trace` | `(ptr: i32, len: i32) -> ()` | Writes the UTF-8 string at `ptr` to the host's debug log. |

A host MUST refuse a cart with any other import.

### 2.3 Metadata

A cart MUST contain a custom section named `wasm-ad` holding UTF-8 text, one
`key=value` pair per line. Keys are lowercase; unknown keys MUST be ignored.

| Key | | |
|---|---|---|
| `spec` | required | Spec version the cart targets, e.g. `0.1`. |
| `title` | required | Display name. Hosts key save data by `title` and `author`. |
| `author` | optional | |
| `version` | optional | The cart's own version. |
| `description` | optional | One line. |
| `url` | optional | Homepage or source repository. |

### 2.4 Limits

A cart file MUST NOT exceed 32 MiB. Hosts MUST allow linear memory to grow to
at least 16 MiB and MAY refuse growth beyond that.

## 3. Lifecycle and timing

### 3.1 Boot

1. The host instantiates the cart, providing `env.trace` if imported.
2. The host zero-fills the hardware region (0x00000–0x27FFF) and hides all
   sprites by setting attribute 0 of every OAM entry to 0x0200. It then
   writes `SEED` and `SPEC`, loads save RAM (§11) and sets `SAVESTAT`.
3. If `start` is exported, the host calls it.

### 3.2 Frame

The host runs 60 frames per second. Each frame:

1. The host writes `KEYS` and `FRAME`, and sets `VCOUNT` to 160.
2. The host calls `update()`.
3. The host latches VRAM and OAM. Writes to them after this point take
   effect on the next frame.
4. For each line `y` from 0 to 159: the host sets `VCOUNT` to `y`; calls
   `scanline(y)` if exported; then draws line `y` using the current display
   registers and palette RAM and the latched VRAM and OAM. A `scanline`
   callback can therefore change scroll offsets, blend coefficients, window
   bounds or colors per line.
5. If bit 0 of `SAVECNT` is set, the host persists save RAM and clears the bit.

`FRAME` is 0 during the first `update`. A host that cannot keep up MAY skip
presenting frames but MUST NOT skip calls to `update`.

## 4. Memory map

All addresses are byte offsets into the cart's linear memory. Multi-byte
values are little-endian.

| Range | Size | Region |
|---|---|---|
| 0x00000–0x003FF | 1 KB | Null page: never read or written by the host |
| 0x00400–0x007FF | 1 KB | I/O: display registers at 0x400 (§6), system registers at 0x700 (§5) |
| 0x00800–0x00BFF | 1 KB | Palette RAM: background 0x800, sprites 0xA00 (§7) |
| 0x00C00–0x00FFF | 1 KB | OAM (§9) |
| 0x01000–0x18FFF | 96 KB | VRAM (§8) |
| 0x19000–0x20FFF | 32 KB | Save RAM (§11) |
| 0x21000–0x27FFF | 28 KB | Reserved (audio, §14). Carts MUST NOT use it. |
| 0x28000– | | Free for the cart: data, stack, heap |

The cart's data segments, stack and heap MUST lie at or above 0x28000. A
display register's address is 0x400 plus its GBA I/O offset.

## 5. System registers

These registers have no GBA counterpart.

| Address | Name | Size | Written by | |
|---|---|---|---|---|
| 0x700 | `KEYS` | u16 | host | Buttons held this frame, 1 = pressed. |
| 0x704 | `FRAME` | u32 | host | Frames since boot. |
| 0x708 | `SEED` | u32 | host | Random value fixed at boot; the only entropy source (§12). |
| 0x70C | `SAVECNT` | u16 | cart | Bit 0: request that save RAM be persisted after this frame. The host clears it. |
| 0x70E | `SAVESTAT` | u16 | host | Bit 0: save RAM was restored from storage at boot. |
| 0x710 | `SPEC` | u16 | host | Spec version the host implements: major × 256 + minor (0.1 → 0x0001). |

`KEYS` bits, in GBA order: 0 A, 1 B, 2 Select, 3 Start, 4 Right, 5 Left,
6 Up, 7 Down, 8 R, 9 L. Unlike the GBA's `KEYINPUT`, a set bit means pressed.

## 6. Display registers

| Address | Name | |
|---|---|---|
| 0x400 | `DISPCNT` | Display control |
| 0x406 | `VCOUNT` | Current line, written by the host (§3.2) |
| 0x408–0x40E | `BG0CNT`–`BG3CNT` | Background control |
| 0x410–0x41E | `BG0HOFS`, `BG0VOFS` … `BG3VOFS` | Background scroll, 9 bits each |
| 0x420–0x43E | | Reserved: affine background parameters (§14) |
| 0x440, 0x442 | `WIN0H`, `WIN1H` | Window horizontal bounds |
| 0x444, 0x446 | `WIN0V`, `WIN1V` | Window vertical bounds |
| 0x448 | `WININ` | Layers inside WIN0 (bits 0–5) and WIN1 (bits 8–13) |
| 0x44A | `WINOUT` | Layers outside all windows (bits 0–5) and inside the sprite window (bits 8–13) |
| 0x44C | `MOSAIC` | Mosaic sizes |
| 0x450 | `BLDCNT` | Blend control |
| 0x452 | `BLDALPHA` | Alpha coefficients |
| 0x454 | `BLDY` | Brightness coefficient |

All other addresses in 0x400–0x6FF are reserved; carts SHOULD leave them zero.

### 6.1 DISPCNT

| Bits | |
|---|---|
| 0–2 | Video mode: 0, 3, 4 or 5. Modes 1, 2, 6 and 7 are reserved and display as forced blank. |
| 4 | Bitmap page select (modes 4 and 5) |
| 6 | Sprite tile mapping: 0 = 2D, 1 = 1D (§9.3) |
| 7 | Forced blank: the display shows white |
| 8–11 | Enable BG0–BG3 |
| 12 | Enable sprites |
| 13, 14 | Enable WIN0, WIN1 |
| 15 | Enable the sprite window |

In mode 0, BG0–BG3 are tiled backgrounds. In modes 3–5 only BG2 exists and
it shows the bitmap.

### 6.2 BGxCNT

| Bits | |
|---|---|
| 0–1 | Priority, 0 (front) to 3 (back) |
| 2–3 | Character base block: tiles start at VRAM + block × 0x4000 |
| 6 | Mosaic |
| 7 | 0 = 4bpp tiles with 16 banks of 16 colors, 1 = 8bpp tiles with 256 colors |
| 8–12 | Screen base block: the map starts at VRAM + block × 0x800 |
| 14–15 | Map size: 0 = 32×32 tiles, 1 = 64×32, 2 = 32×64, 3 = 64×64 |

### 6.3 Windows

`WINxH` holds the left edge in bits 8–15 and the right edge (exclusive) in
bits 0–7; `WINxV` the same for top and bottom. If left > right the window
wraps around the screen edge horizontally; vertically likewise.

Each layer-mask byte enables BG0–BG3 (bits 0–3), sprites (bit 4) and color
effects (bit 5) for the pixels it governs. A pixel is governed by the first
of: WIN0 if enabled and the pixel is inside it; WIN1 likewise; the sprite
window if enabled and an opaque pixel of a sprite in sprite-window mode
(§9.1) covers it; otherwise `WINOUT` bits 0–5. If no window is enabled, every
layer and effect is enabled everywhere.

### 6.4 MOSAIC

Bits 0–3 background horizontal size − 1, 4–7 background vertical size − 1,
8–11 sprite horizontal size − 1, 12–15 sprite vertical size − 1.

A background with mosaic enabled samples screen pixel (x, y) at
(x − x mod h, y − y mod v). A sprite with mosaic enabled samples its local
pixel (u, v) at (u − u mod h, v − v mod v), where local coordinates are
measured from the top-left of the sprite's bounding box.

### 6.5 BLDCNT, BLDALPHA, BLDY

`BLDCNT` bits 0–5 select the first target layers (BG0, BG1, BG2, BG3,
sprites, backdrop), bits 6–7 the effect, bits 8–13 the second target layers.

| Effect | |
|---|---|
| 0 | None |
| 1 | Alpha blend: first target with the layer directly beneath it, if that layer is a second target |
| 2 | Brightness increase of first-target pixels |
| 3 | Brightness decrease of first-target pixels |

`BLDALPHA` holds EVA in bits 0–4 and EVB in bits 8–12; `BLDY` holds EVY in
bits 0–4. Coefficients above 16 are treated as 16. See §10.3 for the arithmetic.

## 7. Palette RAM

256 background colors at 0x800 and 256 sprite colors at 0xA00, each a u16:
red in bits 0–4, green in 5–9, blue in 10–14; bit 15 is ignored.

Color index 0 is transparent in every tiled layer and sprite. The first
background color, at 0x800, is the backdrop, shown wherever no layer is
opaque. With 4bpp graphics, bank n occupies colors 16n to 16n + 15.

Output conversion to 8 bits per channel is `c8 = (c5 << 3) | (c5 >> 2)`.

## 8. VRAM: tiles, maps and bitmaps

VRAM spans 96 KB at 0x01000. Offsets in this section are relative to its start.

| Range | Mode 0 | Modes 3–5 |
|---|---|---|
| 0x00000–0x0FFFF | Background tiles and maps | Bitmap |
| 0x10000–0x13FFF | Sprite tiles 0–511 | Bitmap (mode 3) or unused |
| 0x14000–0x17FFF | Sprite tiles 512–1023 | Sprite tiles 512–1023 |

### 8.1 Tiles

A tile is 8×8 pixels stored row by row. A 4bpp tile is 32 bytes, two pixels
per byte, left pixel in the low nibble. An 8bpp tile is 64 bytes, one byte
per pixel.

### 8.2 Background maps

A map is built from 32×32-entry screen blocks of 2 KB. A 64-wide map uses
two consecutive blocks side by side, a 64-tall map two blocks stacked, and a
64×64 map four blocks in the order top-left, top-right, bottom-left,
bottom-right. Maps wrap in both directions.

Each entry is a u16: tile index in bits 0–9, horizontal flip in bit 10,
vertical flip in bit 11, and palette bank in bits 12–15 (ignored for 8bpp).
A tile's data is at character base + index × 32 (4bpp) or × 64 (8bpp).
Tile data at or beyond VRAM offset 0x10000 reads as transparent.

A background's screen pixel (x, y) maps to map pixel
((x + HOFS) mod width, (y + VOFS) mod height), where HOFS and VOFS use their
low 9 bits.

### 8.3 Bitmap modes

| Mode | Layout | Page 0 | Page 1 |
|---|---|---|---|
| 3 | 240×160, u16 RGB555 per pixel | 0x0000 | none |
| 4 | 240×160, u8 index into background palette | 0x0000 | 0xA000 |
| 5 | 160×128, u16 RGB555 per pixel | 0x0000 | 0xA000 |

`DISPCNT` bit 4 selects the page. BG2's priority, mosaic, scroll offsets and
window/blend participation apply. In mode 4, index 0 is transparent. Pixels
outside a mode 5 bitmap are transparent. In bitmap modes only sprite tiles
512–1023 are displayed; sprites using lower tile indices are not drawn.

## 9. OAM: sprites

OAM holds 128 sprite entries of 8 bytes at 0xC00. Each entry is three u16
attributes followed by one u16 of affine parameter storage.

### 9.1 Attributes

**Attribute 0**

| Bits | |
|---|---|
| 0–7 | Y coordinate of the top edge; values ≥ 160 are taken as Y − 256 |
| 8 | Affine |
| 9 | Affine: double-size. Not affine: disable (sprite hidden) |
| 10–11 | Mode: 0 normal, 1 semi-transparent, 2 sprite window, 3 reserved (hidden) |
| 12 | Mosaic |
| 13 | 0 = 4bpp, 1 = 8bpp |
| 14–15 | Shape: 0 square, 1 wide, 2 tall, 3 reserved (hidden) |

**Attribute 1**

| Bits | |
|---|---|
| 0–8 | X coordinate of the left edge; values ≥ 240 are taken as X − 512 |
| 9–13 | Affine: matrix index 0–31 |
| 12, 13 | Not affine: horizontal flip, vertical flip |
| 14–15 | Size |

**Attribute 2**

| Bits | |
|---|---|
| 0–9 | Tile index, in units of 32 bytes from sprite tile 0 |
| 10–11 | Priority, 0 (front) to 3 (back) |
| 12–15 | Palette bank (4bpp only) |

### 9.2 Sizes

| Size \ Shape | Square | Wide | Tall |
|---|---|---|---|
| 0 | 8×8 | 16×8 | 8×16 |
| 1 | 16×16 | 32×8 | 8×32 |
| 2 | 32×32 | 32×16 | 16×32 |
| 3 | 64×64 | 64×32 | 32×64 |

### 9.3 Tile mapping

For local pixel (u, v) of a sprite W pixels wide, tile column c = u / 8 and
row r = v / 8. With 4bpp, the tile is index + r × (W / 8) + c in 1D mapping,
or index + r × 32 + c in 2D mapping. With 8bpp each tile spans two indices:
index + (r × (W / 8) + c) × 2 in 1D mapping, or index + r × 32 + c × 2 in 2D.
Tile indices wrap modulo 1024.

### 9.4 Affine sprites

Affine matrix n is stored in the fourth u16 of OAM entries 4n (pa),
4n + 1 (pb), 4n + 2 (pc) and 4n + 3 (pd), as signed 8.8 fixed point. The
matrix maps screen space to texture space, so it is the inverse of the
visible transform: (pa, pb, pc, pd) = (cos θ / sx, −sin θ / sx, sin θ / sy,
cos θ / sy) rotates counter-clockwise by θ and scales by (sx, sy).

An affine sprite of size W×H occupies a bounding box of W×H, or 2W×2H with
double-size, anchored at its X and Y. For each pixel of the box with offset
(dx, dy) from the box's centre, the texture coordinate is
u = ((pa × dx + pb × dy) >> 8) + W/2, v = ((pc × dx + pd × dy) >> 8) + H/2,
using arithmetic shifts. Pixels with u or v outside the sprite are
transparent. Flip bits do not apply.

### 9.5 Overlap

Where opaque pixels of several sprites overlap, the one with the lowest
priority number is drawn; among equal priorities, the lowest OAM index.
There is no per-line sprite limit. Sprite-window pixels (mode 2) only shape
the sprite window and are never drawn.

## 10. Composition

Each pixel of a visible line is composed in three steps.

### 10.1 Layer candidates

The candidates are each enabled background whose pixel is not transparent,
the sprite pixel chosen in §9.5, and the backdrop. A layer or sprites
disabled by the governing window (§6.3) contribute nothing.

### 10.2 Ordering

Candidates are ordered by priority number, lowest first. At equal priority a
sprite comes before any background, and a lower-numbered background before a
higher one. The backdrop comes last. The first candidate is the *top*, the
second the *beneath*.

### 10.3 Color effects

Effects apply only where the governing window enables them.

- If the top is a semi-transparent sprite and the beneath is a second
  target, the pixel is alpha-blended, whatever the `BLDCNT` effect.
- Otherwise, if the top is a first target: effect 1 alpha-blends it if the
  beneath is a second target; effect 2 brightens it; effect 3 darkens it.
- Otherwise the top color is shown unchanged.

Per 5-bit channel, with I the top and B the beneath:

- Alpha: min(31, (I × EVA + B × EVB) >> 4)
- Brighten: I + (((31 − I) × EVY) >> 4)
- Darken: I − ((I × EVY) >> 4)

## 11. Save RAM

Save RAM is 32 KB at 0x19000. At boot the host restores the bytes last
persisted for this cart (identified by `title` and `author`), or zeros if
none, and sets `SAVESTAT` accordingly. The cart sets `SAVECNT` bit 0 to ask
for persistence; the host writes all 32 KB after the frame completes. Hosts
SHOULD let users export and import save files as raw 32 KB images.

## 12. Determinism and save states

A cart's behavior is a function of its initial state, `SEED` and the
sequence of `KEYS` values. It has no access to wall-clock time or any other
entropy. Hosts MUST call `update` exactly once per frame as described in
§3.2, so a recorded seed and input sequence replays exactly.

Between frames a cart's whole state is its linear memory: the call stack is
empty, so the stack pointer is back at its initial value. A host MAY
implement save states, rewind and netplay by snapshotting linear memory
between frames. Carts MUST NOT keep state in mutable WebAssembly globals
other than the stack pointer.

## 13. Toolchain notes

The cart's data and stack must stay out of 0x00000–0x27FFF.

**Rust** (`wasm32-unknown-unknown`, `crate-type = ["cdylib"]`): rustc places
the stack at address 0 by default. Pass

```
-C link-arg=--no-stack-first -C link-arg=--global-base=163840 -C link-arg=-zstack-size=65536
```

which puts data at 0x28000 and the stack above it.

**C** (clang, `--target=wasm32`, `-nostdlib`, `-Wl,--no-entry`):
`-Wl,--global-base=163840 -Wl,-z,stack-size=65536 -Wl,--export=update`.
wasm-ld places the stack after the data unless given `--stack-first`.

Other languages need the equivalent of `--global-base=163840` and a stack
that is not placed first. Toolchains that cannot write custom sections can add
the metadata after linking. The repository's `sdk/` directory has SDKs and
build setups for Rust, C and C++, Zig, AssemblyScript and Go (TinyGo), and
`tools/wasmad.mjs check` validates a cart against this specification.

## 14. Reserved for future versions

- **Audio** (0.2): four tone/noise channels with GBA-compatible registers at
  0x460–0x4A8, and two PCM sample channels fed from buffers in 0x21000–0x27FFF.
- **Affine backgrounds**: video modes 1 and 2 and the BG2/BG3 affine
  parameters at 0x420–0x43E.
- Cart icons in the `wasm-ad` custom section.
