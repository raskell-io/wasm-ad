// The starter cart, the same in every SDK: a scrolling checkerboard blended
// over a backdrop gradient drawn per scanline, and a spinning star you move
// with the D-pad (B to go faster, A to toggle mosaic). It uses integer math
// only, so every language's build renders bit-identical frames.
//
//	./build.sh examples/starter
package main

import wasmad "github.com/raskell-io/wasm-ad/sdk/tinygo"

const mapBlock = 31

type State struct {
	buttons wasmad.Buttons
	x, y    int32
	mosaic  bool
}

// Package variables live in linear memory. Initialize them with constants
// only: package initializers do not run in a cart (see README.md).
var state = State{x: 112, y: 72}

//export start
func start() {
	wasmad.Trace("starter: hello from Go")

	// A checkerboard of two solid tiles on background 0.
	bg := wasmad.BGPalette()
	bg[1] = wasmad.RGB(255, 255, 255)
	bg[2] = wasmad.RGB(40, 40, 72)
	tiles := wasmad.BGTiles(0)
	for i := range tiles[1] {
		tiles[1][i] = 0x11
		tiles[2][i] = 0x22
	}
	screen := wasmad.ScreenBlock(mapBlock)
	for i := range screen {
		col, row := i%32, i/32
		tile := uint16(2)
		if (col/2+row/2)%2 == 0 {
			tile = 1
		}
		screen[i] = wasmad.NewScreenEntry(tile, 0)
	}

	// A 16×16 star in four 1D-mapped sprite tiles.
	obj := wasmad.ObjPalette()
	obj[1] = wasmad.RGB(255, 216, 64)
	obj[2] = wasmad.RGB(232, 96, 32)
	star := wasmad.ObjTiles()
	for y := int32(0); y < 16; y++ {
		for x := int32(0); x < 16; x++ {
			dx, dy := abs(x*2-15), abs(y*2-15)
			c := byte(0)
			if dx+dy < 14 || (min(dx, dy) < 3 && max(dx, dy) < 15) {
				c = 1
				if dx+dy < 7 {
					c = 2
				}
			}
			star[y/8*2+x/8][(y%8)*4+(x%8)/2] |= c << (4 * (x % 2))
		}
	}

	wasmad.DISPCNT.Write(wasmad.Mode0 | wasmad.DispBG0 | wasmad.DispObj | wasmad.Obj1D)
	wasmad.BLDCNT.Write(wasmad.BlendCnt(wasmad.BlendAlpha, wasmad.LayerBG0, wasmad.LayerBackdrop))
	wasmad.BLDALPHA.Write(wasmad.BlendAlphaCoeffs(6, 10))
	wasmad.MOSAIC.Write(0x3333)
}

//export update
func update() {
	s := &state
	s.buttons.Update()
	speed := int32(1)
	if s.buttons.Held(wasmad.KeyB) {
		speed = 3
	}
	if s.buttons.Held(wasmad.KeyLeft) {
		s.x -= speed
	}
	if s.buttons.Held(wasmad.KeyRight) {
		s.x += speed
	}
	if s.buttons.Held(wasmad.KeyUp) {
		s.y -= speed
	}
	if s.buttons.Held(wasmad.KeyDown) {
		s.y += speed
	}
	s.x = min(max(s.x, 0), 240-32)
	s.y = min(max(s.y, 0), 160-32)
	if s.buttons.Pressed(wasmad.KeyA) {
		s.mosaic = !s.mosaic
	}

	t := int32(wasmad.Frame())
	wasmad.BGHOFS(0).Write(uint16(t / 2))
	wasmad.BGVOFS(0).Write(uint16(t / 3))
	cnt := wasmad.BgCnt(0).ScreenBlock(mapBlock).Priority(1)
	if s.mosaic {
		cnt = cnt.Mosaic()
	}
	wasmad.BGCNT(0).Write(uint16(cnt))

	// Spin a quarter turn every 32 frames while the size pulses ±25%.
	scale := 256 + (wasmad.Sin(t*3)*64)>>14
	wasmad.OAMSetAffine(0, wasmad.RotateScaleFixed(t*2, scale, scale))
	star := wasmad.NewSprite(s.x, s.y, wasmad.S16x16, 0).Affine(0, true)
	if s.mosaic {
		star = star.Mosaic()
	}
	wasmad.OAMSet(0, star)
}

// Raster effect: a new backdrop color on every line.
//
//export scanline
func scanline(y int32) {
	lerp := func(a, b int32) uint8 { return uint8(a + (b-a)*y/159) }
	wasmad.BGPalette()[0] = wasmad.RGB(lerp(40, 255), lerp(60, 140), lerp(160, 96))
}

func abs(v int32) int32 {
	if v < 0 {
		return -v
	}
	return v
}

func main() {}
