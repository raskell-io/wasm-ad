// The starter cart, the same in every SDK: a rotating checkerboard (an
// affine background) blended over a backdrop gradient drawn per scanline,
// a spinning star you move with the D-pad, and a little tune on all four
// sound channels. B goes faster and plays a sample; A toggles mosaic with a
// rising blip. It uses integer math only, so every language's build renders
// bit-identical frames and sound.
//
//	./build.sh examples/starter
package main

import wasmad "github.com/raskell-io/wasm-ad/sdk/tinygo"

const (
	mapBlock = 8
	zapLen   = 2048 // the zap sample's length in sample RAM
)

// An arpeggio for the square channel and a bass line for the wave channel, in Hz.
var (
	notes = [8]uint32{262, 330, 392, 523, 392, 330, 262, 196}
	bass  = [4]uint32{65, 55, 44, 49}
)

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

	// A checkerboard of two solid 8bpp tiles on affine background 2, 32×32 tiles.
	bg := wasmad.BGPalette()
	bg[1] = wasmad.RGB(255, 255, 255)
	bg[2] = wasmad.RGB(40, 40, 72)
	tiles := wasmad.BGTiles8(0)
	for i := range tiles[1] {
		tiles[1][i] = 1
		tiles[2][i] = 2
	}
	m := wasmad.AffineMap(mapBlock)
	for i := 0; i < 32*32; i++ {
		col, row := i%32, i/32
		m[i] = 2
		if (col/2+row/2)%2 == 0 {
			m[i] = 1
		}
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

	wasmad.DISPCNT.Write(wasmad.Mode1 | wasmad.DispBG2 | wasmad.DispObj | wasmad.Obj1D)
	wasmad.BLDCNT.Write(wasmad.BlendCnt(wasmad.BlendAlpha, wasmad.LayerBG2, wasmad.LayerBackdrop))
	wasmad.BLDALPHA.Write(wasmad.BlendAlphaCoeffs(6, 10))
	wasmad.MOSAIC.Write(0x3333)

	// A triangle wave for the bass, and a falling zap in sample RAM.
	level := func(j int) byte {
		if j < 16 {
			return byte(j)
		}
		return byte(31 - j)
	}
	wave := wasmad.WaveRAM()
	for i := 0; i < 16; i++ {
		wave[i] = level(i*2)<<4 | level(i*2+1)
	}
	zap := wasmad.SampleRAM()
	for i := int32(0); i < zapLen; i++ {
		s := (wasmad.Sin((i*i)>>9) * 127) >> 14
		zap[i] = int8((s * (zapLen - i)) >> 11)
	}
	wasmad.SoundEnable()
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
		// A rising blip: the sweep raises the pitch until it runs out.
		wasmad.SOUND1CNT_L.Write(wasmad.SoundSweep(2, 2, false))
		wasmad.SOUND1CNT_H.Write(wasmad.Duty50 | wasmad.SoundEnvelope(12, 2, false))
		wasmad.SOUND1CNT_X.Write(wasmad.SoundStart | wasmad.SoundRate(440))
	}
	if s.buttons.Pressed(wasmad.KeyB) {
		wasmad.PCMPlay(wasmad.PCMA, 0, zapLen, 16000, 48)
	}

	t := int32(wasmad.Frame())
	// The tune: a note every 16 frames, a hi-hat between them, bass every 64.
	if t%16 == 0 {
		wasmad.SOUND2CNT_L.Write(wasmad.Duty25 | wasmad.SoundEnvelope(9, 1, false))
		wasmad.SOUND2CNT_H.Write(wasmad.SoundStart | wasmad.SoundRate(notes[t/16%8]))
	}
	if t%16 == 8 {
		wasmad.SOUND4CNT_L.Write(wasmad.SoundEnvelope(6, 1, false))
		wasmad.SOUND4CNT_H.Write(wasmad.SoundStart | wasmad.SoundNoise(1, 1, false))
	}
	if t%64 == 0 {
		wasmad.SOUND3CNT_L.Write(wasmad.WaveOn)
		wasmad.SOUND3CNT_H.Write(wasmad.Wave50 | (256 - 48))
		wasmad.SOUND3CNT_X.Write(wasmad.SoundStart | wasmad.SoundLength | wasmad.SoundRate(bass[t/64%4]*2))
	}

	// Turn once every 1024 frames around the screen centre, scrolling diagonally.
	cnt := wasmad.BgCnt(0).ScreenBlock(mapBlock).AffineSize(wasmad.Affine32x32).Wrap().Priority(1)
	if s.mosaic {
		cnt = cnt.Mosaic()
	}
	wasmad.BGCNT(2).Write(uint16(cnt))
	m := wasmad.RotateScaleFixed(t/4, 256, 256)
	wasmad.SetBGAffine(2, m, (t/2)%256, (t/3)%256, 120, 80)

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
