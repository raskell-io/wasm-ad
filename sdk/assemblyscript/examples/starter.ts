// The starter cart, the same in every SDK: a rotating checkerboard (an
// affine background) blended over a backdrop gradient drawn per scanline,
// a spinning star you move with the D-pad, and a little tune on all four
// sound channels. B goes faster and plays a sample; A toggles mosaic with a
// rising blip. A second controller brings in a blue star for player 2. It
// uses integer math only, so every language's build renders bit-identical
// frames and sound.
//
//   npm run build        -> build/starter.wasm

import {
  affine, affineMap, AffineSize, bgcnt, bgTile8, blend, buttons, connected, dispcnt, fixed, frame, key, layer, oam, objTile,
  pcm, reg, rgb, setBgAffine, setBgColor, setObjColor, sound, sprite, SpriteSize, trace, write16,
} from "../assembly/index";

const MAP_BLOCK: u16 = 8;
/** The zap sample's length in sample RAM. */
const ZAP_LEN: i32 = 2048;
/** An arpeggio for the square channel and a bass line for the wave channel, in Hz. */
const NOTES = memory.data<u32>([262, 330, 392, 523, 392, 330, 262, 196]);
const BASS = memory.data<u32>([65, 55, 44, 49]);

// Cart state lives in linear memory, never in WebAssembly globals (SPEC.md §12).
@unmanaged
class State {
  x: i32;
  y: i32;
  mosaic: bool;
  x2: i32;
  y2: i32;
}
const state = changetype<State>(memory.data(offsetof<State>()));

export function start(): void {
  trace("starter: hello from AssemblyScript");
  state.x = 112;
  state.y = 72;
  state.mosaic = false;
  state.x2 = 40;
  state.y2 = 72;

  // A checkerboard of two solid 8bpp tiles on affine background 2, 32×32 tiles.
  setBgColor(1, rgb(255, 255, 255));
  setBgColor(2, rgb(40, 40, 72));
  memory.fill(bgTile8(0, 1), 1, 64);
  memory.fill(bgTile8(0, 2), 2, 64);
  const map = affineMap(MAP_BLOCK);
  for (let i = 0; i < 32 * 32; i++) {
    const col = i % 32, row = i / 32;
    store<u8>(map + <usize>i, (col / 2 + row / 2) % 2 == 0 ? 1 : 2);
  }

  // A 16×16 star in four 1D-mapped sprite tiles.
  setObjColor(1, rgb(255, 216, 64));
  setObjColor(2, rgb(232, 96, 32));
  // Player 2's star: the same tiles in palette bank 1.
  setObjColor(17, rgb(96, 200, 255));
  setObjColor(18, rgb(40, 80, 220));
  for (let y = 0; y < 16; y++) {
    for (let x = 0; x < 16; x++) {
      const dx = abs(x * 2 - 15), dy = abs(y * 2 - 15);
      let c = 0;
      if (dx + dy < 14 || (min(dx, dy) < 3 && max(dx, dy) < 15)) c = dx + dy < 7 ? 2 : 1;
      const at = objTile((y / 8) * 2 + x / 8) + ((y % 8) * 4 + (x % 8) / 2);
      store<u8>(at, load<u8>(at) | (c << (4 * (x % 2))));
    }
  }

  write16(reg.DISPCNT, dispcnt.MODE1 | dispcnt.BG2 | dispcnt.OBJ | dispcnt.OBJ_1D);
  write16(reg.BLDCNT, blend.cnt(blend.ALPHA, layer.BG2, layer.BACKDROP));
  write16(reg.BLDALPHA, blend.alpha(6, 10));
  write16(reg.MOSAIC, 0x3333);

  // A triangle wave for the bass, and a falling zap in sample RAM.
  for (let i = 0; i < 16; i++) {
    store<u8>(sound.waveRam() + <usize>i, (level(i * 2) << 4) | level(i * 2 + 1));
  }
  for (let i = 0; i < ZAP_LEN; i++) {
    const s = (fixed.sin((i * i) >> 9) * 127) >> 14;
    store<i8>(sound.sampleRam() + <usize>i, <i8>((s * (ZAP_LEN - i)) >> 11));
  }
  sound.enable();
}

/** Sample `j` (0–31) of the triangle wave. */
function level(j: i32): i32 {
  return j < 16 ? j : 31 - j;
}

export function update(): void {
  buttons.update();
  const speed = buttons.held(key.B) ? 3 : 1;
  if (buttons.held(key.LEFT)) state.x -= speed;
  if (buttons.held(key.RIGHT)) state.x += speed;
  if (buttons.held(key.UP)) state.y -= speed;
  if (buttons.held(key.DOWN)) state.y += speed;
  state.x = min(max(state.x, 0), 240 - 32);
  state.y = min(max(state.y, 0), 160 - 32);
  if (buttons.pressed(key.A)) {
    state.mosaic = !state.mosaic;
    // A rising blip: the sweep raises the pitch until it runs out.
    write16(reg.SOUND1CNT_L, sound.sweep(2, 2, false));
    write16(reg.SOUND1CNT_H, sound.DUTY_50 | sound.envelope(12, 2, false));
    write16(reg.SOUND1CNT_X, sound.START | sound.rate(440));
  }
  if (buttons.pressed(key.B)) pcm.play(0, 0, <u16>ZAP_LEN, 16000, 48);

  const t = <i32>frame();
  // The tune: a note every 16 frames, a hi-hat between them, bass every 64.
  if (t % 16 == 0) {
    write16(reg.SOUND2CNT_L, sound.DUTY_25 | sound.envelope(9, 1, false));
    write16(reg.SOUND2CNT_H, sound.START | sound.rate(load<u32>(NOTES + (<usize>(t / 16 % 8) << 2))));
  }
  if (t % 16 == 8) {
    write16(reg.SOUND4CNT_L, sound.envelope(6, 1, false));
    write16(reg.SOUND4CNT_H, sound.START | sound.noise(1, 1, false));
  }
  if (t % 64 == 0) {
    write16(reg.SOUND3CNT_L, sound.WAVE_ON);
    write16(reg.SOUND3CNT_H, sound.WAVE_50 | (256 - 48));
    write16(reg.SOUND3CNT_X, sound.START | sound.LENGTH | sound.rate(load<u32>(BASS + (<usize>(t / 64 % 4) << 2)) * 2));
  }

  // Turn once every 1024 frames around the screen centre, scrolling diagonally.
  const cnt = bgcnt.priority(bgcnt.wrap(bgcnt.affineSize(bgcnt.screenBlock(0, MAP_BLOCK), AffineSize.S32x32)), 1);
  write16(reg.BGCNT(2), state.mosaic ? bgcnt.mosaic(cnt) : cnt);
  setBgAffine(2, affine.rotateScaleFixed(t / 4, 256, 256), (t / 2) % 256, (t / 3) % 256, 120, 80);

  // Spin a quarter turn every 32 frames while the size pulses ±25%.
  const scale = 256 + ((fixed.sin(t * 3) * 64) >> 14);
  oam.setAffine(0, affine.rotateScaleFixed(t * 2, scale, scale));
  const star = sprite.affine(sprite.make(state.x, state.y, SpriteSize.S16x16, 0), 0, true);
  oam.set(0, state.mosaic ? sprite.mosaic(star) : star);

  // Player 2 joins when a second controller connects (SPEC.md §5).
  if (connected(2)) {
    buttons.update(2);
    if (buttons.held(key.LEFT, 2)) state.x2 -= 1;
    if (buttons.held(key.RIGHT, 2)) state.x2 += 1;
    if (buttons.held(key.UP, 2)) state.y2 -= 1;
    if (buttons.held(key.DOWN, 2)) state.y2 += 1;
    state.x2 = min(max(state.x2, 0), 240 - 32);
    state.y2 = min(max(state.y2, 0), 160 - 32);
    oam.set(1, sprite.affine(sprite.bank(sprite.make(state.x2, state.y2, SpriteSize.S16x16, 0), 1), 0, true));
  } else {
    oam.hide(1);
  }
}

/** Raster effect: a new backdrop color on every line. */
export function scanline(y: i32): void {
  setBgColor(0, rgb(40 + ((255 - 40) * y) / 159, 60 + ((140 - 60) * y) / 159, 160 + ((96 - 160) * y) / 159));
}
