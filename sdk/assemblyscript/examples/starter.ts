// The starter cart, the same in every SDK: a scrolling checkerboard blended
// over a backdrop gradient drawn per scanline, and a spinning star you move
// with the D-pad (B to go faster, A to toggle mosaic). It uses integer math
// only, so every language's build renders bit-identical frames.
//
//   npm run build        -> build/starter.wasm

import {
  affine, bgcnt, bgTile, blend, buttons, dispcnt, fillTile, fixed, frame, key, layer, oam, objTile, reg,
  rgb, screenEntry, setBgColor, setObjColor, setScreenEntry, sprite, SpriteSize, trace, write16,
} from "../assembly/index";

const MAP_BLOCK: u16 = 31;

// Cart state lives in linear memory, never in WebAssembly globals (SPEC.md §12).
@unmanaged
class State {
  x: i32;
  y: i32;
  mosaic: bool;
}
const state = changetype<State>(memory.data(offsetof<State>()));

export function start(): void {
  trace("starter: hello from AssemblyScript");
  state.x = 112;
  state.y = 72;
  state.mosaic = false;

  // A checkerboard of two solid tiles on background 0.
  setBgColor(1, rgb(255, 255, 255));
  setBgColor(2, rgb(40, 40, 72));
  fillTile(bgTile(0, 1), 0x11);
  fillTile(bgTile(0, 2), 0x22);
  for (let i = 0; i < 1024; i++) {
    const col = i % 32, row = i / 32;
    setScreenEntry(MAP_BLOCK, i, screenEntry.make((col / 2 + row / 2) % 2 == 0 ? 1 : 2, 0));
  }

  // A 16×16 star in four 1D-mapped sprite tiles.
  setObjColor(1, rgb(255, 216, 64));
  setObjColor(2, rgb(232, 96, 32));
  for (let y = 0; y < 16; y++) {
    for (let x = 0; x < 16; x++) {
      const dx = abs(x * 2 - 15), dy = abs(y * 2 - 15);
      let c = 0;
      if (dx + dy < 14 || (min(dx, dy) < 3 && max(dx, dy) < 15)) c = dx + dy < 7 ? 2 : 1;
      const at = objTile((y / 8) * 2 + x / 8) + ((y % 8) * 4 + (x % 8) / 2);
      store<u8>(at, load<u8>(at) | (c << (4 * (x % 2))));
    }
  }

  write16(reg.DISPCNT, dispcnt.MODE0 | dispcnt.BG0 | dispcnt.OBJ | dispcnt.OBJ_1D);
  write16(reg.BLDCNT, blend.cnt(blend.ALPHA, layer.BG0, layer.BACKDROP));
  write16(reg.BLDALPHA, blend.alpha(6, 10));
  write16(reg.MOSAIC, 0x3333);
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
  if (buttons.pressed(key.A)) state.mosaic = !state.mosaic;

  const t = <i32>frame();
  write16(reg.BGHOFS(0), <u16>(t / 2));
  write16(reg.BGVOFS(0), <u16>(t / 3));
  const cnt = bgcnt.priority(bgcnt.screenBlock(0, MAP_BLOCK), 1);
  write16(reg.BGCNT(0), state.mosaic ? bgcnt.mosaic(cnt) : cnt);

  // Spin a quarter turn every 32 frames while the size pulses ±25%.
  const scale = 256 + ((fixed.sin(t * 3) * 64) >> 14);
  oam.setAffine(0, affine.rotateScaleFixed(t * 2, scale, scale));
  const star = sprite.affine(sprite.make(state.x, state.y, SpriteSize.S16x16, 0), 0, true);
  oam.set(0, state.mosaic ? sprite.mosaic(star) : star);
}

/** Raster effect: a new backdrop color on every line. */
export function scanline(y: i32): void {
  setBgColor(0, rgb(40 + ((255 - 40) * y) / 159, 60 + ((140 - 60) * y) / 159, 160 + ((96 - 160) * y) / 159));
}
