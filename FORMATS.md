# WASM-AD File Formats, version 1

Companion to [SPEC.md](SPEC.md), for hosts: files that let a player share
what happened in a cart. A **replay** reproduces a run from boot; a **save
state** resumes one at a given frame. Both work in any host that implements
the spec, because a cart is deterministic (SPEC.md §12) and its whole state
is its linear memory.

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119. All
integers are little-endian.

## 1. Header

Both files start with the same 48 bytes, followed by a body compressed with
raw DEFLATE (RFC 1951, no zlib or gzip wrapper; `deflate-raw` in the
browser's `CompressionStream`, `zlib.deflateRawSync` in Node).

| Offset | Size | |
|---|---|---|
| 0 | 4 | Magic: `WADR` for a replay, `WADS` for a save state |
| 4 | u16 | Format version: 1 |
| 6 | u16 | `SPEC` of the host that wrote the file (SPEC.md §5) |
| 8 | 32 | SHA-256 of the cart's `.wasm` file. All zeros: not tied to a cart (replays only) |
| 40 | u32 | Frames: a replay's length, or the frames a save state has run |
| 44 | u32 | `SEED` |

A host MUST refuse a file with an unknown magic or a newer format version,
and MUST refuse one whose cart hash is neither all zeros nor the hash of the
loaded cart, unless the user explicitly overrides it.

Suggested file extensions are `.wadreplay` and `.wadstate`.

## 2. Replays

The decompressed body:

| Size | |
|---|---|
| u16 | Flags. Bit 0: save RAM follows |
| 32768 | Save RAM at boot, if bit 0 is set |
| 14 × n | Runs of input |

Each run is a u32 count followed by five u16 values, `KEYS`, `KEYS2`,
`KEYS3`, `KEYS4` and `PADS`, held for that many consecutive frames. The
counts add up to the header's frame count.

To play a replay, a host boots the cart as SPEC.md §3.1 describes, with
`SEED` from the header and, instead of the save RAM it has stored, the
replay's save RAM (with `SAVESTAT` bit 0 set) or zeros (with it clear). It
then runs frame n with the inputs of the run covering n, ignoring the user's
controllers. While playing a replay, a host SHOULD NOT persist save RAM.

A host recording a replay SHOULD include save RAM when it restored any at
boot. A replay that is not tied to a cart is an input script that any cart
can run; the SDK conformance tests use one.

## 3. Save states

The decompressed body:

| Size | |
|---|---|
| u32 | Length of the cart's linear memory in bytes, a multiple of 65536 |
| length | The cart's linear memory |

A save state is taken between frames: after the last frame counted in the
header has finished (SPEC.md §3.2 step 5) and before the next one begins.

To load one, a host instantiates the cart and grows its memory to the
stored length, then copies the stored memory into it. It does not perform
the boot steps of SPEC.md §3.1 and does not call `start`: the memory already
holds the hardware state, including sound (SPEC.md §14.8) and save RAM. The
next frame it runs has `FRAME` equal to the header's frame count. Loading a
save state does not persist save RAM; the cart's next request does.

Snapshots a host keeps for itself, for rewind for example, need not use this
format; files it writes for the user MUST.
