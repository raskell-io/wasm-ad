# WASM-AD SDK for Go (TinyGo)

Package `wasmad` wraps the console's memory-mapped hardware in typed helpers,
with the same surface as the Rust SDK. Carts are built with
[TinyGo](https://tinygo.org); the standard Go compiler cannot produce a
WASM-AD cart.

## Build the starter

Install TinyGo 0.43 or newer (it uses the Go installation on your `PATH`; 0.43
supports Go up to 1.27), and Node 18+ for the metadata step:

```sh
./build.sh examples/starter        # build/starter.wasm, checked against SPEC.md
```

The starter is the same cart as every other SDK's, with the same frames and
sound: a rotating checkerboard on an affine background, a star you move, and
a tune on all four sound channels.

Set `TINYGO=/path/to/tinygo` if it is not on your `PATH`. Drop the `.wasm` into
the player, or run it headlessly from the repository root:

```sh
node tools/wasmad.mjs run sdk/tinygo/build/starter.wasm --frames 60 --ppm frame.ppm
```

## A new cart

```go
package main

import wasmad "github.com/raskell-io/wasm-ad/sdk/tinygo"

var x = int32(100) // state: package variables, constant initializers only

//export start
func start() {
	wasmad.BGPalette()[0] = wasmad.RGB(40, 60, 160)
}

//export update
func update() {
	if wasmad.Keys()&wasmad.KeyRight != 0 {
		x++
	}
	wasmad.BGHOFS(0).Write(uint16(x))
}

func main() {}
```

In your own module, require the SDK and copy its TinyGo target:

```sh
go mod init example.com/mycart
go mod edit -require=github.com/raskell-io/wasm-ad/sdk/tinygo@v0.2.0
go mod download github.com/raskell-io/wasm-ad/sdk/tinygo
cp "$(go list -m -f '{{.Dir}}' github.com/raskell-io/wasm-ad/sdk/tinygo)/wasm-ad.json" .
```

`go get` does not work here: it type-checks the package with the standard Go
toolchain, which has no `runtime/volatile`. Build with TinyGo, then add the
metadata section (TinyGo cannot write custom sections) from a wasm-ad checkout:

```sh
tinygo build -target wasm-ad.json -scheduler=none -panic=trap -opt=z -no-debug -o cart.wasm .
node path/to/wasm-ad/tools/wasmad.mjs meta cart.wasm title="My Cart" author=you
node path/to/wasm-ad/tools/wasmad.mjs check cart.wasm
```

Inside this repository, `./build.sh path/to/cart title="My Cart" author=you`
does all three steps.

## Why the build looks like this

`wasm-ad.json` is a TinyGo target that inherits `wasm-unknown` (no WASI, no
imports besides `env.trace`) and changes what SPEC.md needs:

| Setting | Why |
|---|---|
| `--no-stack-first`, `--global-base=163840` | Data and stack above 0x28000, out of the hardware region (§4, §13) |
| `-zstack-size=65536`, `--initial-memory=262144` | A 64 KB stack; 4 pages of memory (at least 3 are required, §2.1) |
| `"gc": "none"` | No heap. Allocating (`append` on a nil slice, `new` that escapes, string concatenation) fails to link with `undefined symbol: runtime.alloc` |

`build.sh` also passes `-scheduler=none -panic=trap -opt=z -no-debug`. A panic
traps, and the host reports the cart as crashed.

Two rules follow from how a host runs a cart (§3.1, §12):

- **Package initializers do not run.** TinyGo runs them from `_initialize`,
  which a WASM-AD host never calls. Initialize package variables with
  constants (they become data in the `.wasm`) and do the rest in `start`.
  `var seed = wasmad.Seed()` would stay 0.
- **State lives in package variables**, which are in linear memory, so hosts
  can snapshot it. Goroutines and channels need a scheduler and are not
  available.

## API

| | |
|---|---|
| Registers | `DISPCNT`, `BGCNT(bg)`, `BGHOFS(bg)`, `BGVOFS(bg)`, `BG2PA` … `BG3Y`, `WIN0H` … `BLDY`, `SOUND1CNT_L` … `SOUNDCNT_X`, `PCMA_SAD` … `PCMB_POS`, `KEYS`, `FRAME`, `SEED`, `SAVECNT`, `SAVESTAT`, `SPEC`, with `.Read()` / `.Write(v)` |
| Bits | `Mode0` … `DispObjWin` (DISPCNT), `Layer*`, `Blend*`, `BlendCnt`, `BlendAlphaCoeffs`, `Key*`, `Map*`, `Affine16x16` … `Affine128x128` |
| Backgrounds | `BgCnt(0).ScreenBlock(b).Priority(p)…`, `.AffineSize(s)`, `.Wrap()`, `NewScreenEntry(tile, bank)`, `ScreenBlock(n)`, `BGTiles(block)`, `BGTiles8(block)`, `AffineMap(block)`, `SetBGAffine(bg, m, tx, ty, sx, sy)`, `VRAM()` |
| Colors | `RGB(r, g, b)`, `BGPalette()`, `ObjPalette()` |
| Sprites | `NewSprite(x, y, S16x16, tile).Bank(b).Priority(p).Affine(slot, double)…`, `OAMSet`, `OAMHide`, `OAMHideAll`, `OAMSetAffine`, `ObjTiles()` |
| Math | `Sin`, `Cos` (angle in 1/256 turns, result in Q14), `RotateScaleFixed(angle, sx, sy)`: the same tables and rounding as every other SDK |
| Sound | `SoundEnable`, `SoundDisable`, `SoundStart`, `SoundLength`, `Duty12` … `Duty75`, `Wave*`, `SoundRate(hz)`, `SoundSweep`, `SoundEnvelope`, `SoundLengthTicks`, `SoundNoise`, `WaveRAM()`, `SampleRAM()`, `SoundStatus()` |
| Samples | `PCMPlay`, `PCMPlayLooped`, `PCMStop`, `PCMPosition`, `PCMPlaying`, channels `PCMA` and `PCMB` |
| Input | `Keys()`, `Buttons` with `Update`, `Held`, `Pressed` |
| System | `Frame()`, `Seed()`, `SaveRAM()`, `SaveRestored()`, `RequestSave()`, `Trace(msg)` |

Slices returned by the accessors point straight at hardware memory; writing
to them is writing to the console.

## Sound

Turn sound on once with `SoundEnable()`, then start a channel by writing its
registers with `SoundStart` set; the host clears the bit after acting on it.
The builders fill in the fields:

```go
wasmad.SOUND2CNT_L.Write(wasmad.Duty50 | wasmad.SoundEnvelope(12, 2, false))
wasmad.SOUND2CNT_H.Write(wasmad.SoundStart | wasmad.SoundRate(440))
```

Sample channels play signed 8-bit samples from `SampleRAM()`:
`PCMPlay(wasmad.PCMA, start, len, rate, volume)`. Registers are read once per
frame, after `update` (SPEC.md §14).

## Affine backgrounds

In `Mode1` BG2, and in `Mode2` BG2 and BG3, are affine: a byte map from
`AffineMap(block)` of 8bpp tiles from `BGTiles8(block)`, drawn through a matrix.
`SetBGAffine(2, wasmad.RotateScaleFixed(angle, 256, 256), tx, ty, 120, 80)`
rotates the map around texture pixel (tx, ty), shown at the screen centre
(SPEC.md §8.4).
