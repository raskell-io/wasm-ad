#!/usr/bin/env node
// Command-line tools for WASM-AD carts, in any language. Needs Node 18+ and
// nothing else.
//
//   wasmad check <cart.wasm>                  validate a cart against SPEC.md
//   wasmad meta <cart.wasm> key=value...      set the `wasm-ad` metadata section
//   wasmad icon <cart.wasm> <icon.png>        set the icon (SPEC.md §2.5)
//   wasmad icon <cart.wasm> --png <out.png>   export the icon as a PNG
//   wasmad run <cart.wasm> [options]          run frames headlessly
//
// `meta` replaces the metadata section in place (or writes `-o <file>`); `spec`
// defaults to the version this tool implements. It is for toolchains that
// cannot emit custom sections themselves (AssemblyScript, TinyGo).
//
// `icon` takes a 32×32 PNG with at most 15 colors plus transparency.
//
// `run` boots the cart like a host does (SPEC.md §3) and renders and makes
// sound with the reference core, player/web/core.wasm (`make player`). Options:
//   --frames N        frames to run (default 1, or the replay's length)
//   --keys SCRIPT     held buttons per frame, e.g. "0-29:right,30-59:down+b,60:a";
//                     "a@2" is player 2's A button
//   --seed N          value for SEED (default 1)
//   --save FILE       boot with this 32 KB save RAM
//   --replay FILE     play a .wadreplay (FORMATS.md) instead of --keys
//   --record FILE     write the run as a .wadreplay (--unbound: not tied to the cart)
//   --state FILE      start from a .wadstate instead of booting
//   --save-state FILE write a .wadstate after the last frame
//   --force           use a replay or state made for a different cart
//   --hash F,F,...    print a hash of the frame rendered at each listed frame,
//                     and one of all the sound
//   --ppm FILE        write the last rendered frame as a PPM image
//   --wav FILE        write the sound of every frame as a 48 kHz stereo WAV
//   --core FILE       a different core.wasm

import { readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { deflateRawSync, deflateSync, inflateRawSync, inflateSync } from 'node:zlib';

const SPEC = '0.3';
const SPEC_VERSION = 0x0003;
const W = 240, H = 160;
const A = {
  IO: 0x400, PAL_END: 0xC00, OAM: 0xC00, VIDEO_END: 0x19000, SAVE: 0x19000, SAVE_SIZE: 0x8000, CART: 0x28000,
  VCOUNT: 0x406, KEYS: 0x700, FRAME: 0x704, SEED: 0x708, SAVECNT: 0x70C, SAVESTAT: 0x70E, SPEC: 0x710,
  KEYS2: 0x712, KEYS3: 0x714, KEYS4: 0x716, PADS: 0x718,
  BG2PA: 0x420, BG2PD: 0x426, BG3PA: 0x430, BG3PD: 0x436,
  SOUND_REGS: 0x460, SOUND_REGS_END: 0x4D0, SOUND_STATE: 0x780, SOUND_STATE_END: 0x800,
};
const SOUND_SAMPLES = 800;
const ICON_SIZE = 544;
const KEY = { a: 1, b: 2, select: 4, start: 8, right: 16, left: 32, up: 64, down: 128, r: 256, l: 512 };
const MAX_CART = 32 * 1024 * 1024;

// ---- WebAssembly binary reading -------------------------------------------

class Reader {
  constructor(bytes, pos = 0, end = bytes.length) {
    Object.assign(this, { bytes, pos, end });
  }
  byte() {
    if (this.pos >= this.end) throw new Error('unexpected end of module');
    return this.bytes[this.pos++];
  }
  u32() {
    let result = 0, shift = 0, b;
    do {
      b = this.byte();
      result |= (b & 0x7F) << shift;
      shift += 7;
    } while (b & 0x80);
    return result >>> 0;
  }
  i32() {
    let result = 0, shift = 0, b;
    do {
      b = this.byte();
      result |= (b & 0x7F) << shift;
      shift += 7;
    } while (b & 0x80);
    return shift < 32 && b & 0x40 ? result | (-1 << shift) : result | 0;
  }
  skip(n) { this.pos += n; }
  name() {
    const n = this.u32();
    const s = new TextDecoder().decode(this.bytes.subarray(this.pos, this.pos + n));
    this.pos += n;
    return s;
  }
  limits() {
    const flags = this.byte();
    const min = this.u32();
    const max = flags & 1 ? this.u32() : null;
    return { flags, min, max };
  }
  // A constant expression; returns its value if it is a single i32.const.
  constExpr() {
    let value = null, first = true;
    for (;;) {
      const op = this.byte();
      if (op === 0x0B) return value;
      if (op === 0x41) value = first ? this.i32() : (this.i32(), null);
      else if (op === 0x23) { this.u32(); value = null; } // global.get
      else if (op === 0x42) { while (this.byte() & 0x80); value = null; }
      else if (op === 0x43) { this.skip(4); value = null; }
      else if (op === 0x44) { this.skip(8); value = null; }
      else value = null;
      first = false;
    }
  }
}

const VALTYPE = { 0x7F: 'i32', 0x7E: 'i64', 0x7D: 'f32', 0x7C: 'f64', 0x7B: 'v128', 0x70: 'funcref', 0x6F: 'externref' };

function sections(bytes) {
  if (bytes.length < 8 || bytes[0] !== 0 || bytes[1] !== 0x61 || bytes[2] !== 0x73 || bytes[3] !== 0x6D) {
    throw new Error('not a WebAssembly module');
  }
  const out = [];
  const r = new Reader(bytes, 8);
  while (r.pos < bytes.length) {
    const start = r.pos;
    const id = r.byte();
    const size = r.u32();
    const body = r.pos;
    out.push({ id, start, body, end: body + size });
    r.pos = body + size;
  }
  return out;
}

/** Parses the parts of a module that SPEC.md constrains. */
export function inspect(bytes) {
  const m = { types: [], imports: [], funcs: [], memories: [], globals: [], exports: [], data: [], custom: [], start: null };
  let importedFuncs = 0, importedGlobals = 0;
  for (const s of sections(bytes)) {
    const r = new Reader(bytes, s.body, s.end);
    switch (s.id) {
      case 0: m.custom.push({ name: r.name(), data: bytes.subarray(r.pos, s.end), start: s.start, end: s.end }); break;
      case 1:
        for (let n = r.u32(); n--;) {
          if (r.byte() !== 0x60) throw new Error('unsupported type form');
          const params = Array.from({ length: r.u32() }, () => VALTYPE[r.byte()]);
          const results = Array.from({ length: r.u32() }, () => VALTYPE[r.byte()]);
          m.types.push({ params, results });
        }
        break;
      case 2:
        for (let n = r.u32(); n--;) {
          const imp = { module: r.name(), name: r.name(), kind: r.byte() };
          if (imp.kind === 0) { imp.type = r.u32(); importedFuncs++; m.funcs.push(imp.type); }
          else if (imp.kind === 1) { r.byte(); r.limits(); }
          else if (imp.kind === 2) { imp.limits = r.limits(); m.memories.push(imp.limits); }
          else if (imp.kind === 3) { r.byte(); imp.mutable = r.byte() === 1; importedGlobals++; m.globals.push({ imported: true, mutable: imp.mutable }); }
          m.imports.push(imp);
        }
        break;
      case 3: for (let n = r.u32(); n--;) m.funcs.push(r.u32()); break;
      case 5: for (let n = r.u32(); n--;) m.memories.push(r.limits()); break;
      case 6:
        for (let n = r.u32(); n--;) {
          const type = VALTYPE[r.byte()];
          const mutable = r.byte() === 1;
          m.globals.push({ type, mutable, init: r.constExpr() });
        }
        break;
      case 7:
        for (let n = r.u32(); n--;) m.exports.push({ name: r.name(), kind: r.byte(), index: r.u32() });
        break;
      case 8: m.start = r.u32(); break;
      case 11:
        for (let n = r.u32(); n--;) {
          const flags = r.u32();
          let offset = null, active = flags !== 1;
          if (flags === 2) r.u32();
          if (active) offset = r.constExpr();
          const size = r.u32();
          r.skip(size);
          m.data.push({ active, offset, size });
        }
        break;
    }
  }
  m.importedFuncs = importedFuncs;
  m.importedGlobals = importedGlobals;
  return m;
}

/** Validates a cart against SPEC.md. Returns { errors, warnings, meta, info }. */
export function check(bytes) {
  const errors = [], warnings = [], info = [];
  if (bytes.length > MAX_CART) errors.push(`cart is ${bytes.length} bytes; the limit is 32 MiB (§2.4)`);
  let m;
  try {
    m = inspect(bytes);
    new WebAssembly.Module(bytes);
  } catch (e) {
    return { errors: [`invalid module: ${e.message}`], warnings, meta: null, info };
  }

  // §2.2 imports
  for (const imp of m.imports) {
    if (imp.module === 'env' && imp.name === 'trace' && imp.kind === 0) {
      const t = m.types[imp.type];
      if (t.params.join() !== 'i32,i32' || t.results.length) errors.push('env.trace must have type (i32, i32) -> () (§2.2)');
    } else {
      errors.push(`imports ${imp.module}.${imp.name}; a cart may only import env.trace (§2.2)`);
    }
  }

  // §2.1 exports
  const exp = Object.fromEntries(m.exports.map((e) => [e.name, e]));
  const funcType = (name) => m.types[m.funcs[exp[name].index]];
  const sig = (t) => `(${t.params.join(', ')}) -> (${t.results.join(', ')})`;
  const want = { update: '() -> ()', start: '() -> ()', scanline: '(i32) -> ()' };
  if (!exp.memory || exp.memory.kind !== 2) errors.push('does not export memory (§2.1)');
  else {
    const mem = m.memories[exp.memory.index];
    if (mem.min < 3) errors.push(`memory starts at ${mem.min} pages; at least 3 are required (§2.1)`);
    if (mem.max !== null && mem.max < 256) warnings.push(`memory is capped at ${mem.max} pages; hosts allow growth to 16 MiB (§2.4)`);
    info.push(`memory: ${mem.min} pages${mem.max !== null ? `, max ${mem.max}` : ''}`);
  }
  for (const [name, want_] of Object.entries(want)) {
    if (!exp[name]) {
      if (name === 'update') errors.push('does not export update (§2.1)');
      continue;
    }
    if (exp[name].kind !== 0) { errors.push(`export ${name} is not a function (§2.1)`); continue; }
    const got = sig(funcType(name));
    if (got !== want_) errors.push(`${name} has type ${got}; expected ${want_} (§2.1)`);
  }
  info.push(`exports: ${['update', 'start', 'scanline'].filter((n) => exp[n]).join(', ')}`);
  if (m.start !== null) errors.push('uses the WebAssembly start section; export `start` instead (§2.1)');

  // §4 memory layout
  for (const d of m.data) {
    if (!d.active) continue;
    if (d.offset === null) warnings.push('a data segment has a non-constant offset; cannot check it is above 0x28000');
    else if ((d.offset >>> 0) < A.CART && d.size > 0) {
      errors.push(`data segment at 0x${(d.offset >>> 0).toString(16)} lies below 0x28000 (§4, §13)`);
    }
  }
  // Only the stack pointer may be a mutable global; we can't see names in a
  // stripped module, so a lone i32 starting in the cart region is taken as it.
  const mutable = m.globals.filter((g) => !g.imported && g.mutable);
  if (mutable.length > 1) {
    errors.push(`${mutable.length} mutable globals; cart state must live in linear memory, only the stack pointer may be a global (§12)`);
  } else if (mutable.length === 1) {
    const [g] = mutable;
    if (g.type !== 'i32') errors.push(`a mutable ${g.type} global holds cart state; it must live in linear memory (§12)`);
    else if (g.init !== null && (g.init >>> 0) < A.CART) {
      errors.push(`a mutable global starts at 0x${(g.init >>> 0).toString(16)}: as the stack pointer it would be inside the hardware region (§4, §13), and anything else is cart state that must live in linear memory (§12)`);
    } else if (g.init !== null) info.push(`stack pointer: 0x${(g.init >>> 0).toString(16)}`);
  }

  // §2.3 metadata
  let meta = null;
  const sections_ = m.custom.filter((c) => c.name === 'wasm-ad');
  if (!sections_.length) errors.push('missing the "wasm-ad" metadata section (§2.3)');
  else {
    if (sections_.length > 1) warnings.push('more than one "wasm-ad" section; hosts read the first');
    meta = parseMeta(sections_[0].data);
    if (!meta.spec) errors.push('metadata has no spec key (§2.3)');
    else if (!/^\d+\.\d+$/.test(meta.spec)) errors.push(`metadata spec "${meta.spec}" is not major.minor (§2.3)`);
    if (!meta.title) errors.push('metadata has no title (§2.3)');
  }
  // §2.5 icon
  const icons = m.custom.filter((c) => c.name === 'wasm-ad-icon');
  if (icons.length) {
    if (icons[0].data.length !== ICON_SIZE) errors.push(`the icon section is ${icons[0].data.length} bytes; it must be ${ICON_SIZE} (§2.5)`);
    else {
      const used = new Set();
      for (const b of icons[0].data.subarray(32)) used.add(b & 15).add(b >> 4);
      used.delete(0);
      info.push(`icon: 32×32, ${used.size} colors`);
    }
  }
  return { errors, warnings, meta, info };
}

export function parseMeta(data) {
  const meta = {};
  for (const line of new TextDecoder().decode(data).split('\n')) {
    const i = line.indexOf('=');
    if (i > 0) meta[line.slice(0, i).trim().toLowerCase()] = line.slice(i + 1).trim();
  }
  return meta;
}

function leb(n) {
  const out = [];
  do {
    let b = n & 0x7F;
    n >>>= 7;
    if (n) b |= 0x80;
    out.push(b);
  } while (n);
  return out;
}

function concat(parts) {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) { out.set(p, at); at += p.length; }
  return out;
}

/** Returns `bytes` with its custom sections called `name` replaced by one holding `data`. */
export function setSection(bytes, name, data) {
  const n = new TextEncoder().encode(name);
  const payload = concat([Uint8Array.from([...leb(n.length), ...n]), data]);
  const parts = [bytes.subarray(0, 8)];
  for (const s of sections(bytes)) {
    if (s.id === 0 && new Reader(bytes, s.body, s.end).name() === name) continue;
    parts.push(bytes.subarray(s.start, s.end));
  }
  parts.push(Uint8Array.from([0, ...leb(payload.length)]), payload);
  return concat(parts);
}

/** Returns `bytes` with its `wasm-ad` sections replaced by one holding `meta`. */
export function setMeta(bytes, meta) {
  const text = Object.entries(meta).map(([k, v]) => `${k}=${v}\n`).join('');
  return setSection(bytes, 'wasm-ad', new TextEncoder().encode(text));
}

// ---- Icons (SPEC.md §2.5) --------------------------------------------------------

/** Decodes a non-interlaced 8-bit-or-less PNG into { width, height, rgba }. */
export function decodePng(bytes) {
  const sig = [137, 80, 78, 71, 13, 10, 26, 10];
  if (sig.some((b, i) => bytes[i] !== b)) throw new Error('not a PNG file');
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let width, height, depth, type, palette = [], alpha = [], idat = [];
  for (let at = 8; at < bytes.length;) {
    const len = dv.getUint32(at);
    const kind = String.fromCharCode(...bytes.subarray(at + 4, at + 8));
    const data = bytes.subarray(at + 8, at + 8 + len);
    if (kind === 'IHDR') {
      const h = new DataView(data.buffer, data.byteOffset, len);
      [width, height, depth, type] = [h.getUint32(0), h.getUint32(4), data[8], data[9]];
      if (data[12]) throw new Error('interlaced PNGs are not supported');
      if (depth > 8) throw new Error('16-bit PNGs are not supported');
    } else if (kind === 'PLTE') for (let i = 0; i < len; i += 3) palette.push([data[i], data[i + 1], data[i + 2]]);
    else if (kind === 'tRNS') alpha = [...data];
    else if (kind === 'IDAT') idat.push(data);
    at += 12 + len;
  }
  const channels = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 }[type];
  if (!channels) throw new Error(`unsupported PNG color type ${type}`);
  const raw = inflateSync(concat(idat));
  const stride = Math.ceil((width * channels * depth) / 8), bpp = Math.max(1, (channels * depth) / 8);
  const rows = [];
  let prev = new Uint8Array(stride);
  for (let y = 0; y < height; y++) {
    const f = raw[y * (stride + 1)], line = raw.slice(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let i = 0; i < stride; i++) {
      const a = i >= bpp ? line[i - bpp] : 0, b = prev[i], c = i >= bpp ? prev[i - bpp] : 0;
      const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
      line[i] += [0, a, b, (a + b) >> 1, pa <= pb && pa <= pc ? a : pb <= pc ? b : c][f];
    }
    rows.push(prev = line);
  }
  const rgba = new Uint8Array(width * height * 4);
  const sample = (row, i) => (depth === 8 ? row[i] : (row[(i * depth) >> 3] >> (8 - depth - ((i * depth) & 7))) & ((1 << depth) - 1));
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const o = (y * width + x) * 4, r = rows[y];
      let px;
      if (type === 3) { const i = sample(r, x); px = [...(palette[i] ?? [0, 0, 0]), alpha[i] ?? 255]; }
      else if (type === 0 || type === 4) {
        const g = sample(r, x * channels) * (255 / ((1 << depth) - 1));
        px = [g, g, g, type === 4 ? r[x * 2 + 1] : 255];
      } else px = [r[x * channels], r[x * channels + 1], r[x * channels + 2], type === 6 ? r[x * 4 + 3] : 255];
      rgba.set(px, o);
    }
  }
  return { width, height, rgba };
}

/** Encodes RGBA8 pixels as a PNG. */
export function encodePng(width, height, rgba) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    for (let k = 0; k < 8; k++) n = n & 1 ? 0xEDB88320 ^ (n >>> 1) : n >>> 1;
    return n >>> 0;
  });
  const crc = (b) => { let c = 0xFFFFFFFF; for (const x of b) c = crcTable[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xFFFFFFFF) >>> 0; };
  const chunk = (kind, data) => {
    const out = new Uint8Array(12 + data.length), dv = new DataView(out.buffer);
    dv.setUint32(0, data.length);
    out.set(new TextEncoder().encode(kind), 4);
    out.set(data, 8);
    dv.setUint32(8 + data.length, crc(out.subarray(4, 8 + data.length)));
    return out;
  };
  const ihdr = new Uint8Array(13), h = new DataView(ihdr.buffer);
  h.setUint32(0, width); h.setUint32(4, height); ihdr.set([8, 6, 0, 0, 0], 8);
  const raw = new Uint8Array(height * (width * 4 + 1));
  for (let y = 0; y < height; y++) raw.set(rgba.subarray(y * width * 4, (y + 1) * width * 4), y * (width * 4 + 1) + 1);
  return concat([Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', new Uint8Array())]);
}

/** Converts a 32×32 PNG with at most 15 colors plus transparency to an icon section's 544 bytes. */
export function iconFromPng(png) {
  const { width, height, rgba } = decodePng(png);
  if (width !== 32 || height !== 32) throw new Error(`the icon is ${width}×${height}; it must be 32×32`);
  const colors = [0], out = new Uint8Array(ICON_SIZE), dv = new DataView(out.buffer);
  for (let i = 0; i < 1024; i++) {
    let index = 0;
    if (rgba[i * 4 + 3] >= 128) {
      const c = (rgba[i * 4] >> 3) | ((rgba[i * 4 + 1] >> 3) << 5) | ((rgba[i * 4 + 2] >> 3) << 10);
      index = colors.indexOf(c, 1);
      if (index < 0) index = colors.push(c) - 1;
    }
    out[32 + (i >> 1)] |= index << ((i & 1) * 4);
  }
  if (colors.length > 16) throw new Error(`the icon has ${colors.length - 1} colors; at most 15 plus transparency fit (§2.5)`);
  colors.forEach((c, i) => dv.setUint16(i * 2, c, true));
  return out;
}

/** The cart's icon as 32×32 RGBA8 pixels, or null. */
export function readIcon(bytes) {
  const icon = inspect(bytes).custom.find((c) => c.name === 'wasm-ad-icon');
  if (!icon || icon.data.length !== ICON_SIZE) return null;
  const d = icon.data, rgba = new Uint8Array(1024 * 4);
  for (let i = 0; i < 1024; i++) {
    const index = (d[32 + (i >> 1)] >> ((i & 1) * 4)) & 15;
    if (!index) continue;
    const c = d[index * 2] | (d[index * 2 + 1] << 8);
    const x = (v) => (v << 3) | (v >> 2);
    rgba.set([x(c & 31), x((c >> 5) & 31), x((c >> 10) & 31), 255], i * 4);
  }
  return rgba;
}

// ---- Replays and save states (FORMATS.md) -------------------------------------

const FORMAT_VERSION = 1;

export function sha256(bytes) {
  return new Uint8Array(createHash('sha256').update(bytes).digest());
}

function header(magic, cartHash, frames, seed) {
  const out = new Uint8Array(48), dv = new DataView(out.buffer);
  out.set(new TextEncoder().encode(magic));
  dv.setUint16(4, FORMAT_VERSION, true);
  dv.setUint16(6, SPEC_VERSION, true);
  if (cartHash) out.set(cartHash, 8);
  dv.setUint32(40, frames >>> 0, true);
  dv.setUint32(44, seed >>> 0, true);
  return out;
}

function readHeader(bytes, magic, what) {
  if (bytes.length < 48 || new TextDecoder().decode(bytes.subarray(0, 4)) !== magic) throw new Error(`not a ${what} file`);
  const dv = new DataView(bytes.buffer, bytes.byteOffset, 48);
  const version = dv.getUint16(4, true);
  if (version > FORMAT_VERSION) throw new Error(`${what} format ${version} is newer than this tool's (${FORMAT_VERSION})`);
  const cart = bytes.slice(8, 40);
  return {
    version, spec: dv.getUint16(6, true), cart: cart.some((b) => b) ? cart : null,
    frames: dv.getUint32(40, true), seed: dv.getUint32(44, true), body: inflateRawSync(bytes.subarray(48)),
  };
}

/** Throws unless a replay or state made for `hash` (null: any cart) fits `cart`. */
export function checkCart(hash, cart, force = false) {
  if (!hash || force) return;
  if (!sha256(cart).every((b, i) => b === hash[i])) throw new Error('this file was made for a different cart (--force to use it anyway)');
}

/**
 * A .wadreplay: `inputs` holds [KEYS, KEYS2, KEYS3, KEYS4, PADS] per frame;
 * `cart` null makes it an input script for any cart.
 */
export function encodeReplay({ cart = null, seed, save = null, inputs }) {
  const runs = [];
  for (const input of inputs) {
    const last = runs.at(-1);
    if (last && last.input.every((v, i) => v === input[i])) last.n++;
    else runs.push({ n: 1, input });
  }
  const body = new Uint8Array(2 + (save ? A.SAVE_SIZE : 0) + runs.length * 14), dv = new DataView(body.buffer);
  let at = 2;
  if (save) { dv.setUint16(0, 1, true); body.set(save, 2); at += A.SAVE_SIZE; }
  for (const { n, input } of runs) {
    dv.setUint32(at, n, true);
    for (let i = 0; i < 5; i++) dv.setUint16(at + 4 + i * 2, input[i] ?? 0, true);
    at += 14;
  }
  return concat([header('WADR', cart && sha256(cart), inputs.length, seed), deflateRawSync(body)]);
}

export function decodeReplay(bytes) {
  const { body, ...h } = readHeader(bytes, 'WADR', 'replay');
  const dv = new DataView(body.buffer, body.byteOffset, body.byteLength);
  let at = 2, save = null;
  if (dv.getUint16(0, true) & 1) { save = body.slice(2, 2 + A.SAVE_SIZE); at += A.SAVE_SIZE; }
  const inputs = [];
  for (; at + 14 <= body.length; at += 14) {
    const input = [0, 1, 2, 3, 4].map((i) => dv.getUint16(at + 4 + i * 2, true));
    for (let n = dv.getUint32(at, true); n--;) inputs.push(input);
  }
  if (inputs.length !== h.frames) throw new Error('the replay is damaged: its runs do not add up to its frame count');
  return { ...h, save, inputs };
}

/** A .wadstate of `memory` after `frame` frames. */
export function encodeState({ cart, frame, seed, memory }) {
  const body = new Uint8Array(4 + memory.length);
  new DataView(body.buffer).setUint32(0, memory.length, true);
  body.set(memory, 4);
  return concat([header('WADS', sha256(cart), frame, seed), deflateRawSync(body)]);
}

export function decodeState(bytes) {
  const { body, ...h } = readHeader(bytes, 'WADS', 'save state');
  const length = new DataView(body.buffer, body.byteOffset, 4).getUint32(0, true);
  if (length % 65536 || body.length !== 4 + length) throw new Error('the save state is damaged');
  if (!h.cart) throw new Error('the save state names no cart');
  return { ...h, memory: body.subarray(4) };
}

// ---- Headless host ----------------------------------------------------------

/**
 * Parses "0-29:right,30-59:down+b+a@2" into a function from a frame number to
 * [KEYS, KEYS2, KEYS3, KEYS4, PADS]. "b@2" is player 2's B; every player the
 * script mentions has a controller for the whole run.
 */
export function parseKeys(script) {
  let pads = 1;
  const spans = (script ?? '').split(',').filter(Boolean).map((part) => {
    const [range, buttons] = part.split(':');
    const [from, to = from] = range.split('-').map(Number);
    const masks = [0, 0, 0, 0];
    for (const b of (buttons ?? '').split('+').filter(Boolean)) {
      const [name, player = '1'] = b.split('@');
      const p = Number(player) - 1;
      if (!(name in KEY)) throw new Error(`unknown button "${name}"; use ${Object.keys(KEY).join(', ')}`);
      if (!(p >= 0 && p < 4)) throw new Error(`unknown player "${player}"; use 1 to 4`);
      masks[p] |= KEY[name];
      pads |= 1 << p;
    }
    return { from, to, masks };
  });
  return (frame) => {
    const out = [0, 0, 0, 0, pads];
    for (const s of spans) if (frame >= s.from && frame <= s.to) s.masks.forEach((m, i) => { out[i] |= m; });
    return out;
  };
}

/**
 * Boots a cart and returns a host that runs frames (SPEC.md §3). `save` is
 * the save RAM to restore at boot; `state` a decoded save state to start
 * from instead of booting (FORMATS.md §3).
 */
export async function boot(bytes, { core: corePath, seed = 1, save = null, state = null, onTrace = () => {} } = {}) {
  const core = (await WebAssembly.instantiate(readFileSync(corePath))).instance.exports;
  let instance;
  const decoder = new TextDecoder();
  const imports = { env: { trace: (ptr, len) => onTrace(decoder.decode(new Uint8Array(instance.exports.memory.buffer, ptr, len))) } };
  instance = (await WebAssembly.instantiate(bytes, imports)).instance;
  const ex = instance.exports;
  const mem = () => new Uint8Array(ex.memory.buffer);
  const dv = () => new DataView(ex.memory.buffer);
  let frame = 0, sound = new Int16Array(SOUND_SAMPLES * 2);
  if (state) {
    const grow = (state.memory.length - ex.memory.buffer.byteLength) / 65536;
    if (grow < 0) throw new Error('the save state has less memory than the cart starts with');
    if (grow) ex.memory.grow(grow);
    mem().set(state.memory);
    frame = state.frames;
  } else {
    mem().fill(0, 0, A.CART);
    for (let i = 0; i < 128; i++) dv().setUint16(A.OAM + i * 8, 0x0200, true);
    for (const a of [A.BG2PA, A.BG2PD, A.BG3PA, A.BG3PD]) dv().setUint16(a, 0x100, true);
    dv().setUint32(A.SEED, seed >>> 0, true);
    dv().setUint16(A.SPEC, SPEC_VERSION, true);
    if (save) {
      mem().set(save, A.SAVE);
      dv().setUint16(A.SAVESTAT, 1, true);
    }
    ex.start?.();
  }

  return {
    exports: ex,
    /** The last frame's sound: 800 stereo pairs of i16, left first. */
    get sound() { return sound; },
    /** Frames run since boot. */
    get frames() { return frame; },
    /** A copy of the cart's whole linear memory, between frames. */
    memory() { return mem().slice(); },
    /**
     * Runs one frame and returns the rendered RGBA8 pixels. `input` is KEYS,
     * or [KEYS, KEYS2, KEYS3, KEYS4, PADS].
     */
    frame(input = 0) {
      const [keys, k2 = 0, k3 = 0, k4 = 0, pads = 1] = typeof input === 'number' ? [input] : input;
      dv().setUint16(A.KEYS, keys & 0x3FF, true);
      dv().setUint16(A.KEYS2, k2 & 0x3FF, true);
      dv().setUint16(A.KEYS3, k3 & 0x3FF, true);
      dv().setUint16(A.KEYS4, k4 & 0x3FF, true);
      dv().setUint16(A.PADS, (pads & 0xF) | 1, true);
      dv().setUint32(A.FRAME, frame >>> 0, true);
      dv().setUint16(A.VCOUNT, 160, true);
      ex.update();
      const coreMem = new Uint8Array(core.memory.buffer, core.ad_mem(), core.ad_mem_len());
      coreMem.set(mem().subarray(0, A.CART));
      core.ad_sound_frame();
      for (const [from, to] of [[A.SOUND_REGS, A.SOUND_REGS_END], [A.SOUND_STATE, A.SOUND_STATE_END]]) {
        mem().set(coreMem.subarray(from, to), from);
      }
      sound = new Int16Array(core.memory.buffer, core.ad_sound_buffer(), SOUND_SAMPLES * 2).slice();
      for (let y = 0; y < H; y++) {
        if (ex.scanline) {
          dv().setUint16(A.VCOUNT, y, true);
          ex.scanline(y);
          coreMem.set(mem().subarray(A.IO, A.PAL_END), A.IO);
        }
        core.ad_render_line(y);
      }
      if (dv().getUint16(A.SAVECNT, true) & 1) dv().setUint16(A.SAVECNT, dv().getUint16(A.SAVECNT, true) & ~1, true);
      frame++;
      return new Uint8Array(core.memory.buffer, core.ad_framebuffer(), W * H * 4).slice();
    },
  };
}

export function hash(...chunks) {
  const h = createHash('sha256');
  for (const c of chunks) h.update(new Uint8Array(c.buffer, c.byteOffset, c.byteLength));
  return h.digest('hex').slice(0, 16);
}

/** A 48 kHz 16-bit stereo WAV file of `frames` (Int16Arrays from `sound`). */
export function wav(frames) {
  const bytes = frames.reduce((n, f) => n + f.byteLength, 0);
  const out = new DataView(new ArrayBuffer(44 + bytes));
  const tag = (at, s) => [...s].forEach((c, i) => out.setUint8(at + i, c.charCodeAt(0)));
  tag(0, 'RIFF'); out.setUint32(4, 36 + bytes, true); tag(8, 'WAVEfmt ');
  out.setUint32(16, 16, true); out.setUint16(20, 1, true); out.setUint16(22, 2, true);
  out.setUint32(24, 48000, true); out.setUint32(28, 48000 * 4, true); out.setUint16(32, 4, true);
  out.setUint16(34, 16, true); tag(36, 'data'); out.setUint32(40, bytes, true);
  let at = 44;
  for (const f of frames) for (const s of f) { out.setInt16(at, s, true); at += 2; }
  return new Uint8Array(out.buffer);
}

function ppm(pixels) {
  const head = new TextEncoder().encode(`P6 ${W} ${H} 255\n`);
  const out = new Uint8Array(head.length + W * H * 3);
  out.set(head);
  for (let i = 0; i < W * H; i++) out.set(pixels.subarray(i * 4, i * 4 + 3), head.length + i * 3);
  return out;
}

// ---- CLI ----------------------------------------------------------------------

const FLAGS = new Set(['force', 'unbound']);

function options(args) {
  const opts = { _: [] };
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '-o') opts.o = args[++i];
    else if (args[i].startsWith('--')) {
      const name = args[i].slice(2);
      opts[name] = FLAGS.has(name) ? true : args[++i];
    } else opts._.push(args[i]);
  }
  return opts;
}

async function main([cmd, ...args]) {
  const opts = options(args);
  const [file, ...rest] = opts._;
  if (!cmd || !file || !['check', 'meta', 'icon', 'run'].includes(cmd)) {
    console.error([
      'usage: wasmad check <cart.wasm>',
      '       wasmad meta <cart.wasm> [-o out.wasm] key=value...',
      '       wasmad icon <cart.wasm> <icon.png> [-o out.wasm]',
      '       wasmad icon <cart.wasm> --png <out.png>',
      '       wasmad run <cart.wasm> [--frames N] [--keys SCRIPT | --replay FILE] [--seed N] [--save FILE]',
      '                  [--state FILE] [--hash F,F] [--ppm FILE] [--wav FILE] [--record FILE [--unbound]]',
      '                  [--save-state FILE] [--force]',
    ].join('\n'));
    return 2;
  }
  const bytes = new Uint8Array(readFileSync(file));

  if (cmd === 'meta') {
    const meta = { spec: SPEC };
    for (const kv of rest) {
      const i = kv.indexOf('=');
      if (i <= 0) throw new Error(`expected key=value, got "${kv}"`);
      meta[kv.slice(0, i).toLowerCase()] = kv.slice(i + 1);
    }
    if (!meta.title) throw new Error('title=... is required');
    writeFileSync(opts.o ?? file, setMeta(bytes, meta));
    return 0;
  }

  if (cmd === 'icon') {
    if (opts.png) {
      const rgba = readIcon(bytes);
      if (!rgba) throw new Error('the cart has no icon');
      writeFileSync(opts.png, encodePng(32, 32, rgba));
    } else {
      if (!rest[0]) throw new Error('give a 32×32 PNG');
      writeFileSync(opts.o ?? file, setSection(bytes, 'wasm-ad-icon', iconFromPng(new Uint8Array(readFileSync(rest[0])))));
    }
    return 0;
  }

  if (cmd === 'check') {
    const { errors, warnings, meta, info } = check(bytes);
    if (meta) console.log(`${meta.title}${meta.author ? ` by ${meta.author}` : ''} (spec ${meta.spec}), ${bytes.length} bytes`);
    for (const line of info) console.log(`  ${line}`);
    for (const w of warnings) console.log(`warning: ${w}`);
    for (const e of errors) console.log(`error: ${e}`);
    if (!errors.length) console.log('ok');
    return errors.length ? 1 : 0;
  }

  const { errors } = check(bytes);
  if (errors.length) {
    for (const e of errors) console.error(`error: ${e}`);
    return 1;
  }
  const core = opts.core ?? fileURLToPath(new URL('../player/web/core.wasm', import.meta.url));
  const replay = opts.replay ? decodeReplay(new Uint8Array(readFileSync(opts.replay))) : null;
  const state = opts.state ? decodeState(new Uint8Array(readFileSync(opts.state))) : null;
  if (replay) checkCart(replay.cart, bytes, opts.force);
  if (state) checkCart(state.cart, bytes, opts.force);
  if (state && opts.record) throw new Error('a replay starts at boot, so it cannot be recorded from a save state');
  let save = replay ? replay.save : opts.save ? new Uint8Array(readFileSync(opts.save)) : null;
  if (save && save.length !== A.SAVE_SIZE) throw new Error('a save file must be exactly 32 KB');
  const seed = replay?.seed ?? Number(opts.seed ?? 1);
  const host = await boot(bytes, { core, seed, save, state, onTrace: (m) => console.log(`trace: ${m}`) });
  const keys = parseKeys(opts.keys);
  const input = (f) => (replay ? replay.inputs[f] ?? [0, 0, 0, 0, 1] : keys(f));
  const first = host.frames, frames = Number(opts.frames ?? (replay ? replay.frames - first : 1));
  const marks = new Set((opts.hash ?? '').split(',').filter(Boolean).map(Number));
  let pixels;
  const sound = [], recorded = [];
  for (let f = first; f < first + frames; f++) {
    recorded.push(input(f));
    pixels = host.frame(input(f));
    sound.push(host.sound);
    if (marks.has(f)) console.log(`frame ${f}: ${hash(pixels)}`);
  }
  if (opts.record) writeFileSync(opts.record, encodeReplay({ cart: opts.unbound ? null : bytes, seed, save, inputs: recorded }));
  if (opts['save-state']) writeFileSync(opts['save-state'], encodeState({ cart: bytes, frame: host.frames, seed: state?.seed ?? seed, memory: host.memory() }));
  if (opts.hash) console.log(`sound: ${hash(...sound)}`);
  if (opts.ppm) writeFileSync(opts.ppm, ppm(pixels));
  if (opts.wav) writeFileSync(opts.wav, wav(sound));
  return 0;
}

// Run as a command, also through a symlink such as node_modules/.bin/wasmad.
if (process.argv[1] && realpathSync(fileURLToPath(import.meta.url)) === realpathSync(process.argv[1])) {
  main(process.argv.slice(2)).then((code) => process.exit(code), (e) => {
    console.error(`error: ${e.message}`);
    process.exit(1);
  });
}
