// Package wasmad is the TinyGo SDK for WASM-AD carts.
//
// A cart exports update (and optionally start and scanline) and drives the
// console by writing its memory-mapped hardware, which this package wraps in
// typed helpers. See SPEC.md for the hardware and README.md for the build.
//
//	//export update
//	func update() { wasmad.DISPCNT.Write(wasmad.Mode0) }
package wasmad

import (
	"runtime/volatile"
	"unsafe"
)

// Memory map (SPEC.md §4).
const (
	ScreenW = 240
	ScreenH = 160

	AddrIO      = 0x00400
	AddrPalBG   = 0x00800
	AddrPalObj  = 0x00A00
	AddrOAM     = 0x00C00
	AddrVRAM    = 0x01000
	VRAMSize    = 0x18000
	AddrObjVRAM = AddrVRAM + 0x10000 // sprite tile 0
	AddrSave    = 0x19000
	SaveSize    = 0x8000
	AddrCart    = 0x28000 // first address the cart may use

	ObjHidden   = 0x0200 // OAM attribute 0 that hides a sprite
	SpecVersion = 0x0001
)

// Reg16 is a 16-bit hardware register.
type Reg16 uintptr

func (r Reg16) Read() uint16   { return volatile.LoadUint16((*uint16)(unsafe.Pointer(uintptr(r)))) }
func (r Reg16) Write(v uint16) { volatile.StoreUint16((*uint16)(unsafe.Pointer(uintptr(r))), v) }

// Reg32 is a 32-bit hardware register.
type Reg32 uintptr

func (r Reg32) Read() uint32   { return volatile.LoadUint32((*uint32)(unsafe.Pointer(uintptr(r)))) }
func (r Reg32) Write(v uint32) { volatile.StoreUint32((*uint32)(unsafe.Pointer(uintptr(r))), v) }

// Every register, by its SPEC.md name.
const (
	DISPCNT  Reg16 = 0x400
	VCOUNT   Reg16 = 0x406
	WIN0H    Reg16 = 0x440
	WIN1H    Reg16 = 0x442
	WIN0V    Reg16 = 0x444
	WIN1V    Reg16 = 0x446
	WININ    Reg16 = 0x448
	WINOUT   Reg16 = 0x44A
	MOSAIC   Reg16 = 0x44C
	BLDCNT   Reg16 = 0x450
	BLDALPHA Reg16 = 0x452
	BLDY     Reg16 = 0x454
	KEYS     Reg16 = 0x700
	FRAME    Reg32 = 0x704
	SEED     Reg32 = 0x708
	SAVECNT  Reg16 = 0x70C
	SAVESTAT Reg16 = 0x70E
	SPEC     Reg16 = 0x710
)

// BGCNT returns background bg's control register (bg 0–3).
func BGCNT(bg int) Reg16 { return Reg16(0x408 + bg*2) }

// BGHOFS returns background bg's horizontal scroll register.
func BGHOFS(bg int) Reg16 { return Reg16(0x410 + bg*4) }

// BGVOFS returns background bg's vertical scroll register.
func BGVOFS(bg int) Reg16 { return Reg16(0x412 + bg*4) }

// DISPCNT bits (SPEC.md §6.1).
const (
	Mode0       uint16 = 0
	Mode3       uint16 = 3
	Mode4       uint16 = 4
	Mode5       uint16 = 5
	Page1       uint16 = 1 << 4
	Obj1D       uint16 = 1 << 6
	ForcedBlank uint16 = 1 << 7
	DispBG0     uint16 = 1 << 8
	DispBG1     uint16 = 1 << 9
	DispBG2     uint16 = 1 << 10
	DispBG3     uint16 = 1 << 11
	DispObj     uint16 = 1 << 12
	DispWin0    uint16 = 1 << 13
	DispWin1    uint16 = 1 << 14
	DispObjWin  uint16 = 1 << 15
)

// Layer bits for BLDCNT targets and window masks.
const (
	LayerBG0      uint16 = 1 << 0
	LayerBG1      uint16 = 1 << 1
	LayerBG2      uint16 = 1 << 2
	LayerBG3      uint16 = 1 << 3
	LayerObj      uint16 = 1 << 4
	LayerBackdrop uint16 = 1 << 5
	LayerEffects  uint16 = 1 << 5 // in window masks: color effects enabled
	LayerAll      uint16 = 0x3F
)

// BLDCNT effects (SPEC.md §6.5).
const (
	BlendNone     uint16 = 0
	BlendAlpha    uint16 = 1 << 6
	BlendBrighten uint16 = 2 << 6
	BlendDarken   uint16 = 3 << 6
)

// BlendCnt is a BLDCNT value: effect on first targets, blending onto second.
func BlendCnt(effect, first, second uint16) uint16 { return effect | first | second<<8 }

// BlendAlphaCoeffs is a BLDALPHA value; coefficients 0–16.
func BlendAlphaCoeffs(eva, evb uint16) uint16 { return eva | evb<<8 }

// KEYS bits (SPEC.md §5).
const (
	KeyA      uint16 = 1 << 0
	KeyB      uint16 = 1 << 1
	KeySelect uint16 = 1 << 2
	KeyStart  uint16 = 1 << 3
	KeyRight  uint16 = 1 << 4
	KeyLeft   uint16 = 1 << 5
	KeyUp     uint16 = 1 << 6
	KeyDown   uint16 = 1 << 7
	KeyR      uint16 = 1 << 8
	KeyL      uint16 = 1 << 9
)

// Background map sizes, in tiles.
const (
	Map32x32 uint16 = 0
	Map64x32 uint16 = 1
	Map32x64 uint16 = 2
	Map64x64 uint16 = 3
)

// BgCnt is a BGxCNT value (SPEC.md §6.2).
type BgCnt uint16

func (c BgCnt) Priority(p uint16) BgCnt    { return c&^3 | BgCnt(p&3) }
func (c BgCnt) CharBlock(b uint16) BgCnt   { return c&^0xC | BgCnt((b&3)<<2) }
func (c BgCnt) ScreenBlock(b uint16) BgCnt { return c&^0x1F00 | BgCnt((b&31)<<8) }
func (c BgCnt) Size(s uint16) BgCnt        { return c&0x3FFF | BgCnt(s<<14) }
func (c BgCnt) Bpp8() BgCnt                { return c | 0x80 }
func (c BgCnt) Mosaic() BgCnt              { return c | 0x40 }

// RGB packs 8-bit channels into RGB555.
func RGB(r, g, b uint8) uint16 {
	return uint16(r>>3) | uint16(g>>3)<<5 | uint16(b>>3)<<10
}

func view16(addr uintptr, n int) []uint16 { return unsafe.Slice((*uint16)(unsafe.Pointer(addr)), n) }

// BGPalette is the 256 background colors; 16 banks of 16 for 4bpp graphics.
func BGPalette() []uint16 { return view16(AddrPalBG, 256) }

// ObjPalette is the 256 sprite colors.
func ObjPalette() []uint16 { return view16(AddrPalObj, 256) }

// Tile4 is a 4bpp tile: 8 rows of 4 bytes, left pixel in the low nibble.
type Tile4 [32]byte

// VRAM is all of VRAM as bytes.
func VRAM() []byte { return unsafe.Slice((*byte)(unsafe.Pointer(uintptr(AddrVRAM))), VRAMSize) }

// BGTiles is the 4bpp background tiles from character block block (0–3) to
// the end of background VRAM.
func BGTiles(block int) []Tile4 {
	return unsafe.Slice((*Tile4)(unsafe.Pointer(uintptr(AddrVRAM+block*0x4000))), (4-block)*512)
}

// ObjTiles is the 1024 4bpp sprite tiles.
func ObjTiles() []Tile4 { return unsafe.Slice((*Tile4)(unsafe.Pointer(uintptr(AddrObjVRAM))), 1024) }

// ScreenEntry is a background map entry (SPEC.md §8.2).
type ScreenEntry uint16

func NewScreenEntry(tile, bank uint16) ScreenEntry { return ScreenEntry(tile&0x3FF | (bank&0xF)<<12) }
func (e ScreenEntry) HFlip() ScreenEntry           { return e | 1<<10 }
func (e ScreenEntry) VFlip() ScreenEntry           { return e | 1<<11 }

// ScreenBlock is screen block block (0–31): a 32×32 map, row-major.
func ScreenBlock(block int) []ScreenEntry {
	return unsafe.Slice((*ScreenEntry)(unsafe.Pointer(uintptr(AddrVRAM+block*0x800))), 1024)
}

// SpriteSize is a (shape, size) pair (SPEC.md §9.2): shape in bits 2–3, size in bits 0–1.
type SpriteSize uint16

const (
	S8x8 SpriteSize = iota
	S16x16
	S32x32
	S64x64
	W16x8
	W32x8
	W32x16
	W64x32
	T8x16
	T8x32
	T16x32
	T32x64
)

// Dims returns the sprite's width and height in pixels.
func (s SpriteSize) Dims() (int32, int32) {
	shape, size := s/4, s%4
	switch shape {
	case 0:
		return 8 << size, 8 << size
	case 1:
		return [4]int32{16, 32, 32, 64}[size], [4]int32{8, 8, 16, 32}[size]
	default:
		return [4]int32{8, 8, 16, 32}[size], [4]int32{16, 32, 32, 64}[size]
	}
}

// Sprite is one OAM entry's three attributes (SPEC.md §9.1), built fluently.
type Sprite struct{ Attr0, Attr1, Attr2 uint16 }

var SpriteHidden = Sprite{Attr0: ObjHidden}

// NewSprite is a sprite with its top-left at screen (x, y), using tile and bank 0.
func NewSprite(x, y int32, size SpriteSize, tile uint16) Sprite {
	shape, sz := uint16(size/4), uint16(size%4)
	return Sprite{
		Attr0: uint16(y&0xFF) | shape<<14,
		Attr1: uint16(x&0x1FF) | sz<<14,
		Attr2: tile & 0x3FF,
	}
}

func (s Sprite) Bank(bank uint16) Sprite { s.Attr2 = s.Attr2&0x0FFF | (bank&0xF)<<12; return s }
func (s Sprite) Priority(p uint16) Sprite {
	s.Attr2 = s.Attr2&^0x0C00 | (p&3)<<10
	return s
}
func (s Sprite) HFlip(on bool) Sprite { s.Attr1 = setBit(s.Attr1, 0x1000, on); return s }
func (s Sprite) VFlip(on bool) Sprite { s.Attr1 = setBit(s.Attr1, 0x2000, on); return s }
func (s Sprite) SemiTransparent() Sprite {
	s.Attr0 = s.Attr0&^0x0C00 | 0x0400
	return s
}
func (s Sprite) Window() Sprite { s.Attr0 = s.Attr0&^0x0C00 | 0x0800; return s }
func (s Sprite) Mosaic() Sprite { s.Attr0 |= 0x1000; return s }
func (s Sprite) Bpp8() Sprite   { s.Attr0 |= 0x2000; return s }

// Affine uses affine matrix slot (0–31). With doubleSize the bounding box
// doubles so rotated or enlarged pixels are not clipped.
func (s Sprite) Affine(slot uint16, doubleSize bool) Sprite {
	s.Attr0 = setBit(s.Attr0|0x0100, 0x0200, doubleSize)
	s.Attr1 = s.Attr1&^0x3E00 | (slot&31)<<9
	return s
}

func setBit(v, bit uint16, on bool) uint16 {
	if on {
		return v | bit
	}
	return v &^ bit
}

// OAMSet writes sprite i (0–127).
func OAMSet(i int, s Sprite) {
	at := AddrOAM + i*8
	Reg16(at).Write(s.Attr0)
	Reg16(at + 2).Write(s.Attr1)
	Reg16(at + 4).Write(s.Attr2)
}

// OAMHide hides sprite i.
func OAMHide(i int) { OAMSet(i, SpriteHidden) }

// OAMHideAll hides all 128 sprites.
func OAMHideAll() {
	for i := 0; i < 128; i++ {
		OAMHide(i)
	}
}

// OAMSetAffine writes affine matrix slot (0–31) into the gaps of OAM.
func OAMSetAffine(slot int, m AffineMatrix) {
	at := AddrOAM + slot*32
	for i, v := range [4]int16{m.PA, m.PB, m.PC, m.PD} {
		Reg16(at + 6 + i*8).Write(uint16(v))
	}
}

// AffineMatrix is an affine matrix in 8.8 fixed point, mapping screen to texture space.
type AffineMatrix struct{ PA, PB, PC, PD int16 }

var AffineIdentity = AffineMatrix{256, 0, 0, 256}

// RotateScaleFixed rotates counter-clockwise by angle (1/256 turns) and
// scales by (sx, sy) in 8.8 fixed point (256 = 1×, must be nonzero), in
// integer math so every SDK computes the same matrix.
func RotateScaleFixed(angle, sx, sy int32) AffineMatrix {
	sin, cos := Sin(angle), Cos(angle)
	return AffineMatrix{clamp16(cos * 4 / sx), clamp16(-sin * 4 / sx), clamp16(sin * 4 / sy), clamp16(cos * 4 / sy)}
}

func clamp16(v int32) int16 {
	if v < -32768 {
		return -32768
	}
	if v > 32767 {
		return 32767
	}
	return int16(v)
}

// SinQ14 is sin over a quarter turn in 1/256-turn steps, scaled by 16384 (Q14).
// Shared by all WASM-AD SDKs, so carts compute bit-identical results (SPEC.md §12).
var SinQ14 = [65]int32{
	0, 402, 804, 1205, 1606, 2006, 2404, 2801, 3196, 3590, 3981, 4370, 4756, 5139, 5520, 5897, 6270, 6639, 7005,
	7366, 7723, 8076, 8423, 8765, 9102, 9434, 9760, 10080, 10394, 10702, 11003, 11297, 11585, 11866, 12140, 12406,
	12665, 12916, 13160, 13395, 13623, 13842, 14053, 14256, 14449, 14635, 14811, 14978, 15137, 15286, 15426, 15557,
	15679, 15791, 15893, 15986, 16069, 16143, 16207, 16261, 16305, 16340, 16364, 16379, 16384,
}

// Sin is sin of angle 1/256 turns (only the low 8 bits count), in Q14.
func Sin(angle int32) int32 {
	a := angle & 255
	switch {
	case a < 64:
		return SinQ14[a]
	case a < 128:
		return SinQ14[128-a]
	case a < 192:
		return -SinQ14[a-128]
	default:
		return -SinQ14[256-a]
	}
}

// Cos is cos of angle 1/256 turns, in Q14.
func Cos(angle int32) int32 { return Sin(angle + 64) }

// Keys returns the buttons held this frame.
func Keys() uint16 { return KEYS.Read() }

// Buttons does edge detection over KEYS; keep one in your game state and
// call Update at the start of every frame.
type Buttons struct{ held, prev uint16 }

func (b *Buttons) Update() {
	b.prev = b.held
	b.held = Keys()
}
func (b *Buttons) Held(k uint16) bool { return b.held&k != 0 }

// Pressed is true only on the frame the button went down.
func (b *Buttons) Pressed(k uint16) bool { return b.held&^b.prev&k != 0 }

// Frame is the number of frames since boot.
func Frame() uint32 { return FRAME.Read() }

// Seed is the boot-time random seed, the cart's only entropy (SPEC.md §12).
func Seed() uint32 { return SEED.Read() }

// SaveRAM is the 32 KB battery-backed save RAM.
func SaveRAM() []byte { return unsafe.Slice((*byte)(unsafe.Pointer(uintptr(AddrSave))), SaveSize) }

// SaveRestored reports whether the host restored save RAM from storage at boot.
func SaveRestored() bool { return SAVESTAT.Read()&1 != 0 }

// RequestSave asks the host to persist save RAM after this frame.
func RequestSave() { SAVECNT.Write(SAVECNT.Read() | 1) }

//go:wasmimport env trace
func hostTrace(ptr unsafe.Pointer, len uint32)

// Trace writes a message to the host's debug log.
func Trace(msg string) {
	hostTrace(unsafe.Pointer(unsafe.StringData(msg)), uint32(len(msg)))
}
