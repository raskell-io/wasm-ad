#!/usr/bin/env node
// Command-line tools for WASM-AD carts, in any language. Needs Node 18+ and
// nothing else.
//
//   wasmad check <cart.wasm>                  validate a cart against SPEC.md
//   wasmad meta <cart.wasm> key=value...      set the `wasm-ad` metadata section
//   wasmad run <cart.wasm> [options]          run frames headlessly
//
// `meta` replaces the metadata section in place (or writes `-o <file>`); `spec`
// defaults to the version this tool implements. It is for toolchains that
// cannot emit custom sections themselves (AssemblyScript, TinyGo).
//
// `run` boots the cart like a host does (SPEC.md §3) and renders with the
// reference renderer, player/web/core.wasm (`make player`). Options:
//   --frames N        frames to run (default 1)
//   --keys SCRIPT     held buttons per frame, e.g. "0-29:right,30-59:down+b,60:a"
//   --seed N          value for SEED (default 1)
//   --hash F,F,...    print a hash of the frame rendered at each listed frame
//   --ppm FILE        write the last rendered frame as a PPM image
//   --core FILE       a different core.wasm

import { readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const SPEC = '0.1';
const SPEC_VERSION = 0x0001;
const W = 240, H = 160;
const A = {
  IO: 0x400, PAL_END: 0xC00, OAM: 0xC00, VIDEO_END: 0x19000, SAVE: 0x19000, SAVE_SIZE: 0x8000, CART: 0x28000,
  VCOUNT: 0x406, KEYS: 0x700, FRAME: 0x704, SEED: 0x708, SAVECNT: 0x70C, SAVESTAT: 0x70E, SPEC: 0x710,
};
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

/** Returns `bytes` with its `wasm-ad` sections replaced by one holding `meta`. */
export function setMeta(bytes, meta) {
  const text = Object.entries(meta).map(([k, v]) => `${k}=${v}\n`).join('');
  const name = new TextEncoder().encode('wasm-ad');
  const payload = [...leb(name.length), ...name, ...new TextEncoder().encode(text)];
  const section = Uint8Array.from([0, ...leb(payload.length), ...payload]);
  const parts = [bytes.subarray(0, 8)];
  for (const s of sections(bytes)) {
    if (s.id === 0 && new Reader(bytes, s.body, s.end).name() === 'wasm-ad') continue;
    parts.push(bytes.subarray(s.start, s.end));
  }
  parts.push(section);
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) { out.set(p, at); at += p.length; }
  return out;
}

// ---- Headless host ----------------------------------------------------------

/** Parses "0-29:right,30-59:down+b,60:a" into a frame -> KEYS function. */
export function parseKeys(script) {
  const spans = (script ?? '').split(',').filter(Boolean).map((part) => {
    const [range, buttons] = part.split(':');
    const [from, to = from] = range.split('-').map(Number);
    let mask = 0;
    for (const b of (buttons ?? '').split('+').filter(Boolean)) {
      if (!(b in KEY)) throw new Error(`unknown button "${b}"; use ${Object.keys(KEY).join(', ')}`);
      mask |= KEY[b];
    }
    return { from, to, mask };
  });
  return (frame) => spans.reduce((k, s) => (frame >= s.from && frame <= s.to ? k | s.mask : k), 0);
}

/** Boots a cart and returns a host that runs frames (SPEC.md §3). */
export async function boot(bytes, { core: corePath, seed = 1, onTrace = () => {} } = {}) {
  const core = (await WebAssembly.instantiate(readFileSync(corePath))).instance.exports;
  let instance;
  const decoder = new TextDecoder();
  const imports = { env: { trace: (ptr, len) => onTrace(decoder.decode(new Uint8Array(instance.exports.memory.buffer, ptr, len))) } };
  instance = (await WebAssembly.instantiate(bytes, imports)).instance;
  const ex = instance.exports;
  const mem = () => new Uint8Array(ex.memory.buffer);
  const dv = () => new DataView(ex.memory.buffer);
  mem().fill(0, 0, A.CART);
  for (let i = 0; i < 128; i++) dv().setUint16(A.OAM + i * 8, 0x0200, true);
  dv().setUint32(A.SEED, seed >>> 0, true);
  dv().setUint16(A.SPEC, SPEC_VERSION, true);
  ex.start?.();

  let frame = 0;
  return {
    exports: ex,
    /** Runs one frame with `keys` held and returns the rendered RGBA8 pixels. */
    frame(keys = 0) {
      dv().setUint16(A.KEYS, keys & 0x3FF, true);
      dv().setUint32(A.FRAME, frame >>> 0, true);
      dv().setUint16(A.VCOUNT, 160, true);
      ex.update();
      const coreMem = new Uint8Array(core.memory.buffer, core.ad_mem(), core.ad_mem_len());
      coreMem.set(mem().subarray(0, A.VIDEO_END));
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

export function hash(pixels) {
  return createHash('sha256').update(pixels).digest('hex').slice(0, 16);
}

function ppm(pixels) {
  const head = new TextEncoder().encode(`P6 ${W} ${H} 255\n`);
  const out = new Uint8Array(head.length + W * H * 3);
  out.set(head);
  for (let i = 0; i < W * H; i++) out.set(pixels.subarray(i * 4, i * 4 + 3), head.length + i * 3);
  return out;
}

// ---- CLI ----------------------------------------------------------------------

function options(args) {
  const opts = { _: [] };
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '-o') opts.o = args[++i];
    else if (args[i].startsWith('--')) opts[args[i].slice(2)] = args[++i];
    else opts._.push(args[i]);
  }
  return opts;
}

async function main([cmd, ...args]) {
  const opts = options(args);
  const [file, ...rest] = opts._;
  if (!cmd || !file || !['check', 'meta', 'run'].includes(cmd)) {
    console.error('usage: wasmad check <cart.wasm>\n       wasmad meta <cart.wasm> [-o out.wasm] key=value...\n       wasmad run <cart.wasm> [--frames N] [--keys SCRIPT] [--seed N] [--hash F,F] [--ppm FILE]');
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
  const host = await boot(bytes, { core, seed: Number(opts.seed ?? 1), onTrace: (m) => console.log(`trace: ${m}`) });
  const keys = parseKeys(opts.keys);
  const frames = Number(opts.frames ?? 1);
  const marks = new Set((opts.hash ?? '').split(',').filter(Boolean).map(Number));
  let pixels;
  for (let f = 0; f < frames; f++) {
    pixels = host.frame(keys(f));
    if (marks.has(f)) console.log(`frame ${f}: ${hash(pixels)}`);
  }
  if (opts.ppm) writeFileSync(opts.ppm, ppm(pixels));
  return 0;
}

// Run as a command, also through a symlink such as node_modules/.bin/wasmad.
if (process.argv[1] && realpathSync(fileURLToPath(import.meta.url)) === realpathSync(process.argv[1])) {
  main(process.argv.slice(2)).then((code) => process.exit(code), (e) => {
    console.error(`error: ${e.message}`);
    process.exit(1);
  });
}
