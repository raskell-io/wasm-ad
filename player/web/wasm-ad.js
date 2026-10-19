// WASM-AD host for the browser: implements the host side of SPEC.md and
// FORMATS.md, and renders and makes sound with the reference core (core.wasm,
// from the wasm-ad-core crate).
//
//   import { createConsole } from './wasm-ad.js';
//   const console = await createConsole({ canvas });
//   await console.load(await (await fetch('cart.wasm')).arrayBuffer());
//
// The console is built on `createMachine`, which has no DOM and also runs in
// Node: it boots carts and runs frames, nothing else.

export const W = 240, H = 160;
const STEP = 1000 / 60;
const A = {
  IO: 0x400, PAL_END: 0xC00, VIDEO_END: 0x19000, OAM: 0xC00, SAVE: 0x19000, SAVE_SIZE: 0x8000,
  CART: 0x28000, VCOUNT: 0x406, KEYS: 0x700, FRAME: 0x704, SEED: 0x708,
  SAVECNT: 0x70C, SAVESTAT: 0x70E, SPEC: 0x710, KEYS2: 0x712, KEYS3: 0x714, KEYS4: 0x716, PADS: 0x718,
  BG2PA: 0x420, BG2PD: 0x426, BG3PA: 0x430, BG3PD: 0x436,
  SOUND_REGS: 0x460, SOUND_REGS_END: 0x4D0, SOUND_STATE: 0x780, SOUND_STATE_END: 0x800,
};
export const SPEC_VERSION = 0x0003;
export const SAVE_SIZE = A.SAVE_SIZE;
const SOUND_RATE = 48000;
const SOUND_SAMPLES = 800;

// Plays the frames of sound the console posts, 800 stereo samples each. It
// waits for two frames before starting, and drops the oldest when more than
// six are queued, so latency stays between about 30 and 100 ms.
const WORKLET = `
class WasmAdSound extends AudioWorkletProcessor {
  constructor() {
    super();
    this.queue = [];
    this.at = 0;
    this.primed = false;
    this.port.onmessage = ({ data }) => {
      if (data === 'clear') { this.queue = []; this.at = 0; this.primed = false; return; }
      this.queue.push(data);
      while (this.queue.length > 6) { this.queue.shift(); this.at = 0; }
    };
  }
  process(inputs, [[left, right]]) {
    if (!this.primed && this.queue.length >= 2) this.primed = true;
    let i = 0;
    while (this.primed && i < left.length && this.queue.length) {
      const f = this.queue[0];
      for (; i < left.length && this.at < f.length; i++, this.at += 2) {
        left[i] = f[this.at];
        right[i] = f[this.at + 1];
      }
      if (this.at >= f.length) { this.queue.shift(); this.at = 0; }
    }
    if (i < left.length) {
      left.fill(0, i);
      right.fill(0, i);
      this.primed = false;
    }
    return true;
  }
}
registerProcessor('wasm-ad-sound', WasmAdSound);
`;

/** `KEYS` bits (SPEC.md §5). */
export const KEY = { a: 1, b: 2, select: 4, start: 8, right: 16, left: 32, up: 64, down: 128, r: 256, l: 512 };

export const KEYMAP = {
  ArrowRight: KEY.right, ArrowLeft: KEY.left, ArrowUp: KEY.up, ArrowDown: KEY.down,
  KeyX: KEY.a, KeyZ: KEY.b, KeyA: KEY.l, KeyS: KEY.r,
  Enter: KEY.start, ShiftLeft: KEY.select, ShiftRight: KEY.select,
};

/** The buttons a standard-mapping gamepad holds, Nintendo positions: the right face button is A. */
function padKeys(pad) {
  let k = 0;
  const on = (i) => pad.buttons[i]?.pressed;
  const [x = 0, y = 0] = pad.axes;
  if (on(1)) k |= KEY.a;
  if (on(0)) k |= KEY.b;
  if (on(4)) k |= KEY.l;
  if (on(5)) k |= KEY.r;
  if (on(8)) k |= KEY.select;
  if (on(9)) k |= KEY.start;
  if (on(12) || y < -0.5) k |= KEY.up;
  if (on(13) || y > 0.5) k |= KEY.down;
  if (on(14) || x < -0.5) k |= KEY.left;
  if (on(15) || x > 0.5) k |= KEY.right;
  return k;
}

/** Reads the `wasm-ad` metadata section (SPEC.md §2.3). */
export function readMeta(module) {
  const sections = WebAssembly.Module.customSections(module, 'wasm-ad');
  if (sections.length === 0) throw new Error('not a WASM-AD cart: missing "wasm-ad" metadata section');
  const meta = {};
  for (const line of new TextDecoder().decode(sections[0]).split('\n')) {
    const i = line.indexOf('=');
    if (i > 0) meta[line.slice(0, i).trim().toLowerCase()] = line.slice(i + 1).trim();
  }
  if (!meta.title) throw new Error('cart metadata has no title');
  return meta;
}

/** The cart's icon (SPEC.md §2.5) as 32×32 RGBA8 pixels, or null. */
export function readIcon(module) {
  const [section] = WebAssembly.Module.customSections(module, 'wasm-ad-icon');
  if (!section || section.byteLength !== 544) return null;
  const d = new Uint8Array(section), rgba = new Uint8ClampedArray(32 * 32 * 4);
  const x = (v) => (v << 3) | (v >> 2);
  for (let i = 0; i < 1024; i++) {
    const index = (d[32 + (i >> 1)] >> ((i & 1) * 4)) & 15;
    if (!index) continue;
    const c = d[index * 2] | (d[index * 2 + 1] << 8);
    rgba.set([x(c & 31), x((c >> 5) & 31), x((c >> 10) & 31), 255], i * 4);
  }
  return rgba;
}

// ---- Replays and save states (FORMATS.md) -------------------------------------

const FORMAT_VERSION = 1;

async function pipe(bytes, stream) {
  return new Uint8Array(await new Response(new Blob([bytes]).stream().pipeThrough(stream)).arrayBuffer());
}
const deflateRaw = (bytes) => pipe(bytes, new CompressionStream('deflate-raw'));
const inflateRaw = (bytes) => pipe(bytes, new DecompressionStream('deflate-raw'));

export async function sha256(bytes) {
  return new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
}

function concat(parts) {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) { out.set(p, at); at += p.length; }
  return out;
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

/** 'replay', 'state', 'cart' or null, from a file's first bytes. */
export function fileKind(bytes) {
  const magic = new TextDecoder().decode(bytes.subarray(0, 4));
  return { WADR: 'replay', WADS: 'state', '\0asm': 'cart' }[magic] ?? null;
}

async function readHeader(bytes, magic, what) {
  if (bytes.length < 48 || new TextDecoder().decode(bytes.subarray(0, 4)) !== magic) throw new Error(`not a ${what} file`);
  const dv = new DataView(bytes.buffer, bytes.byteOffset, 48);
  const version = dv.getUint16(4, true);
  if (version > FORMAT_VERSION) throw new Error(`${what} format ${version} is newer than this player's (${FORMAT_VERSION})`);
  const cart = bytes.slice(8, 40);
  return {
    version, spec: dv.getUint16(6, true), cart: cart.some((b) => b) ? cart : null,
    frames: dv.getUint32(40, true), seed: dv.getUint32(44, true), body: await inflateRaw(bytes.subarray(48)),
  };
}

/** Throws unless a replay or state made for `hash` (null: any cart) fits a cart hashing to `cartHash`. */
export function checkCart(hash, cartHash, force = false) {
  if (!hash || force) return;
  if (!cartHash.every((b, i) => b === hash[i])) throw new Error('this file was made for a different cart');
}

/** A .wadreplay; `inputs` holds [KEYS, KEYS2, KEYS3, KEYS4, PADS] per frame; `cartHash` null makes it unbound. */
export async function encodeReplay({ cartHash = null, seed, save = null, inputs }) {
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
  return concat([header('WADR', cartHash, inputs.length, seed), await deflateRaw(body)]);
}

export async function decodeReplay(bytes) {
  const { body, ...h } = await readHeader(bytes, 'WADR', 'replay');
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

/** A .wadstate of `memory` after `frames` frames. */
export async function encodeState({ cartHash, frames, seed, memory }) {
  const body = new Uint8Array(4 + memory.length);
  new DataView(body.buffer).setUint32(0, memory.length, true);
  body.set(memory, 4);
  return concat([header('WADS', cartHash, frames, seed), await deflateRaw(body)]);
}

export async function decodeState(bytes) {
  const { body, ...h } = await readHeader(bytes, 'WADS', 'save state');
  const length = new DataView(body.buffer, body.byteOffset, 4).getUint32(0, true);
  if (length % 65536 || body.length !== 4 + length) throw new Error('the save state is damaged');
  if (!h.cart) throw new Error('the save state names no cart');
  return { ...h, memory: body.subarray(4) };
}

// ---- The machine ----------------------------------------------------------------

/**
 * A console without a screen: boots carts and runs frames (SPEC.md §3) with
 * the reference core's exports. It has no DOM, so it also runs in Node.
 *
 * Options: `onTrace(message)`; `onSave(bytes)`, called when the cart asks for
 * save RAM to be persisted (§11).
 */
export function createMachine(core, { onTrace = () => {}, onSave = () => {} } = {}) {
  let module = null, ex = null, frames = 0;
  let sound = new Int16Array(SOUND_SAMPLES * 2);
  const decoder = new TextDecoder();
  const mem = () => new Uint8Array(ex.memory.buffer);
  const dv = () => new DataView(ex.memory.buffer);

  function instantiate() {
    let instance;
    const imports = { env: { trace: (ptr, len) => onTrace(decoder.decode(new Uint8Array(instance.exports.memory.buffer, ptr, len))) } };
    instance = new WebAssembly.Instance(module, imports);
    const e = instance.exports;
    if (!(e.memory instanceof WebAssembly.Memory)) throw new Error('cart does not export memory');
    if (typeof e.update !== 'function') throw new Error('cart does not export update()');
    if (e.memory.buffer.byteLength < 3 * 65536) throw new Error('cart memory is smaller than 3 pages');
    ex = e;
  }

  /** Copies a save state's memory in, growing memory or starting a fresh instance as needed. */
  function restore(state) {
    if (ex.memory.buffer.byteLength > state.memory.length) instantiate();
    const grow = (state.memory.length - ex.memory.buffer.byteLength) / 65536;
    if (grow < 0) throw new Error('the save state has less memory than the cart starts with');
    if (grow) ex.memory.grow(grow);
    mem().set(state.memory);
    frames = state.frames;
  }

  return {
    get exports() { return ex; },
    get module() { return module; },
    /** Frames run since boot. */
    get frames() { return frames; },
    /** The last frame's sound: 800 stereo pairs of i16, left first. */
    get sound() { return sound; },

    /**
     * Boots a cart (bytes or a compiled WebAssembly.Module). `save` is the
     * save RAM to restore; `state` a decoded save state ({ frames, memory })
     * to start from instead of booting (FORMATS.md §3).
     */
    async boot(cart, { seed = 1, save = null, state = null } = {}) {
      module = cart instanceof WebAssembly.Module ? cart : await WebAssembly.compile(cart);
      instantiate();
      frames = 0;
      if (state) return restore(state);
      const m = mem(), d = dv();
      m.fill(0, 0, A.CART);
      for (let i = 0; i < 128; i++) d.setUint16(A.OAM + i * 8, 0x0200, true);
      for (const a of [A.BG2PA, A.BG2PD, A.BG3PA, A.BG3PD]) d.setUint16(a, 0x100, true);
      d.setUint32(A.SEED, seed >>> 0, true);
      d.setUint16(A.SPEC, SPEC_VERSION, true);
      if (save) {
        m.set(save, A.SAVE);
        d.setUint16(A.SAVESTAT, 1, true);
      }
      ex.start?.();
    },

    /** Returns to a snapshot from `memory()`, without booting. */
    restore,

    /**
     * Runs one frame (SPEC.md §3.2) and returns the rendered RGBA8 pixels,
     * valid until the next frame. `input` is KEYS, or [KEYS, KEYS2, KEYS3, KEYS4, PADS].
     */
    frame(input = 0) {
      const [keys, k2 = 0, k3 = 0, k4 = 0, pads = 1] = typeof input === 'number' ? [input] : input;
      let d = dv();
      d.setUint16(A.KEYS, keys & 0x3FF, true);
      d.setUint16(A.KEYS2, k2 & 0x3FF, true);
      d.setUint16(A.KEYS3, k3 & 0x3FF, true);
      d.setUint16(A.KEYS4, k4 & 0x3FF, true);
      d.setUint16(A.PADS, (pads & 0xF) | 1, true);
      d.setUint32(A.FRAME, frames >>> 0, true);
      d.setUint16(A.VCOUNT, 160, true);
      ex.update();

      // update() may have grown memory, which replaces the buffer.
      const coreMem = new Uint8Array(core.memory.buffer, core.ad_mem(), core.ad_mem_len());
      coreMem.set(new Uint8Array(ex.memory.buffer, 0, A.CART));
      // Sound for this frame (§14.2); the core updates registers and state.
      core.ad_sound_frame();
      const m = mem();
      for (const [from, to] of [[A.SOUND_REGS, A.SOUND_REGS_END], [A.SOUND_STATE, A.SOUND_STATE_END]]) {
        m.set(coreMem.subarray(from, to), from);
      }
      sound = new Int16Array(core.memory.buffer, core.ad_sound_buffer(), SOUND_SAMPLES * 2).slice();
      for (let y = 0; y < H; y++) {
        if (ex.scanline) {
          dv().setUint16(A.VCOUNT, y, true);
          ex.scanline(y);
          // Only registers and palette change mid-frame; VRAM and OAM stay latched.
          coreMem.set(new Uint8Array(ex.memory.buffer, A.IO, A.PAL_END - A.IO), A.IO);
        }
        core.ad_render_line(y);
      }

      d = dv();
      if (d.getUint16(A.SAVECNT, true) & 1) {
        onSave(mem().slice(A.SAVE, A.SAVE + A.SAVE_SIZE));
        d.setUint16(A.SAVECNT, d.getUint16(A.SAVECNT, true) & ~1, true);
      }
      frames++;
      return new Uint8Array(core.memory.buffer, core.ad_framebuffer(), W * H * 4);
    },

    /** A copy of the cart's whole linear memory, between frames. */
    memory() { return mem().slice(); },
    /** A copy of save RAM. */
    saveRam() { return mem().slice(A.SAVE, A.SAVE + A.SAVE_SIZE); },
    /** The current `SEED`. */
    get seed() { return dv().getUint32(A.SEED, true); },
  };
}

// ---- The console -----------------------------------------------------------------

// Rewind keeps a snapshot every few frames, for about ten seconds, within a memory budget.
const REWIND_EVERY = 4;
const REWIND_SECONDS = 10;
const REWIND_BUDGET = 64 * 1024 * 1024;

/**
 * Creates a console drawing into `canvas`.
 *
 * Options:
 * - `core`: URL of core.wasm (defaults to next to this module)
 * - `keyboard`: listen to the keyboard (default true); holding Backspace rewinds
 * - `active()`: whether keyboard input should go to the console right now;
 *   arrow keys only stop scrolling the page while it returns true
 * - `sound`: play sound (default true). Browsers only allow it after the
 *   user interacts with the page, so it starts on the first key or click.
 * - `volume`: 0 to 1 (default 1)
 * - `onError(error)`, `onTrace(message, meta)`
 * - `onReplayEnd()`: a replay finished and live play resumed
 *
 * Gamepads are players 1–4 in the order they connected; the keyboard and
 * on-screen buttons are player 1 too (SPEC.md §5).
 */
export async function createConsole({
  canvas,
  core: coreUrl = new URL('core.wasm', import.meta.url),
  keyboard = true,
  active = () => true,
  sound = true,
  volume = 1,
  onError = (e) => console.error(e),
  onTrace = (msg, meta) => console.log(`[${meta.title}]`, msg),
  onReplayEnd = () => {},
}) {
  const ctx = canvas.getContext('2d');
  const image = ctx.createImageData(W, H);
  const core = (await WebAssembly.instantiateStreaming(fetch(coreUrl))).instance.exports;

  let cart = null; // { module, meta, saveKey, hash, icon }
  let recording = null; // { seed, save, inputs }
  let replay = null; // { inputs }
  // `latched` remembers presses until a frame has seen them, so a tap shorter
  // than one frame still reaches the cart.
  let keys = 0, virtualKeys = 0, latched = 0, failed = false, paused = false;
  let rewinding = false, keyRewind = false, ring = [], ringSlots = 0;

  const machine = createMachine(core, {
    onTrace: (msg) => onTrace(msg, cart.meta),
    onSave: (bytes) => { if (!replay) persist(bytes); },
  });

  // Web Audio, created on the first user gesture (autoplay rules).
  let audio = null; // { ctx, node, gain }
  async function startAudio() {
    if (audio || !sound || typeof AudioWorkletNode === 'undefined') return;
    const ctx = new AudioContext({ sampleRate: SOUND_RATE, latencyHint: 'interactive' });
    audio = { ctx };
    try {
      const url = URL.createObjectURL(new Blob([WORKLET], { type: 'text/javascript' }));
      await ctx.audioWorklet.addModule(url);
      URL.revokeObjectURL(url);
      const node = new AudioWorkletNode(ctx, 'wasm-ad-sound', { numberOfInputs: 0, outputChannelCount: [2] });
      const gain = new GainNode(ctx, { gain: volume });
      node.connect(gain).connect(ctx.destination);
      Object.assign(audio, { node, gain });
    } catch (e) {
      console.warn('no sound:', e);
    }
  }
  const unlock = () => {
    if (!audio) startAudio();
    else if (audio.ctx.state === 'suspended') audio.ctx.resume();
  };
  for (const type of ['pointerdown', 'keydown', 'touchstart']) addEventListener(type, unlock, { capture: true });

  function postSound() {
    if (!audio?.node || audio.ctx.state !== 'running') return;
    const pcm = machine.sound, f = new Float32Array(pcm.length);
    for (let i = 0; i < pcm.length; i++) f[i] = pcm[i] / 32768;
    audio.node.port.postMessage(f, [f.buffer]);
  }
  const clearSound = () => audio?.node?.port.postMessage('clear');

  if (keyboard) {
    const typing = (t) => t instanceof HTMLElement && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));
    addEventListener('keydown', (e) => {
      if (e.metaKey || e.ctrlKey || !cart || !active() || typing(e.target)) return;
      if (e.code === 'Backspace') {
        keyRewind = true;
        e.preventDefault();
        return;
      }
      if (!(e.code in KEYMAP)) return;
      keys |= KEYMAP[e.code];
      latched |= KEYMAP[e.code];
      e.preventDefault();
    });
    addEventListener('keyup', (e) => {
      if (e.code === 'Backspace') keyRewind = false;
      if (e.code in KEYMAP) keys &= ~KEYMAP[e.code];
    });
    addEventListener('blur', () => { keys = 0; keyRewind = false; });
  }

  /** [KEYS, KEYS2, KEYS3, KEYS4, PADS] from the keyboard, on-screen buttons and gamepads. */
  function liveInput() {
    const out = [keys | virtualKeys | latched, 0, 0, 0, 1];
    latched = 0;
    const pads = [...(navigator.getGamepads?.() ?? [])].filter((p) => p?.connected);
    pads.slice(0, 4).forEach((pad, i) => {
      out[i] |= padKeys(pad);
      out[4] |= 1 << i;
    });
    for (let i = 0; i < 4; i++) out[i] &= 0x3FF;
    return out;
  }

  function loadSave() {
    try {
      const b64 = localStorage.getItem(cart.saveKey);
      if (!b64) return null;
      const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
      return bytes.length === A.SAVE_SIZE ? bytes : null;
    } catch { return null; }
  }

  function persist(bytes) {
    let bin = '';
    for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    try { localStorage.setItem(cart.saveKey, btoa(bin)); } catch (e) { console.warn('could not persist save', e); }
  }

  function resetRing() {
    ring = [];
    ringSlots = 0;
  }

  /** Started fresh: a random seed and stored save RAM, unless given (SPEC.md §3.1). */
  async function boot({ seed = crypto.getRandomValues(new Uint32Array(1))[0], save = loadSave(), state = null } = {}) {
    failed = false;
    clearSound();
    resetRing();
    await machine.boot(cart.module, { seed, save, state });
    return { seed, save };
  }

  // Rewind snapshots: memory plus the frame they show.
  function snapshot(pixels) {
    if (machine.frames % REWIND_EVERY) return;
    const memory = machine.memory();
    if (!ringSlots) ringSlots = Math.max(1, Math.min((REWIND_SECONDS * 60) / REWIND_EVERY, Math.floor(REWIND_BUDGET / (memory.length + pixels.length))));
    if (ring.length >= ringSlots) ring.shift();
    ring.push({ frames: machine.frames, memory, pixels: pixels.slice() });
  }

  function rewindStep() {
    if (ring.length > 1) ring.pop();
    const snap = ring.at(-1);
    if (!snap) return;
    machine.restore(snap);
    if (recording) recording.inputs.length = Math.min(recording.inputs.length, snap.frames);
    present(snap.pixels);
  }

  function runFrame() {
    let input;
    if (replay) {
      input = replay.inputs[machine.frames];
      if (!input) {
        replay = null;
        onReplayEnd();
        input = liveInput();
      }
    } else {
      input = liveInput();
    }
    if (recording) recording.inputs[machine.frames] = input;
    const pixels = machine.frame(input);
    postSound();
    snapshot(pixels);
    return pixels;
  }

  function present(pixels) {
    image.data.set(pixels);
    ctx.putImageData(image, 0, 0);
  }

  // Fixed timestep: carts run at 60 Hz whatever the display refresh rate.
  // While rewinding, each tick steps back one snapshot instead.
  let last = performance.now(), acc = 0;
  function tick(now) {
    acc = Math.min(acc + now - last, STEP * 5);
    last = now;
    if (machine.exports && !failed && !paused) {
      try {
        if (rewinding || keyRewind) {
          if (acc >= STEP * 2) { rewindStep(); acc = 0; clearSound(); }
        } else {
          let pixels = null;
          while (acc >= STEP) { pixels = runFrame(); acc -= STEP; }
          if (pixels) present(pixels);
        }
      } catch (e) {
        failed = true;
        clearSound();
        onError(new Error(`cart crashed: ${e.message}`));
      }
    } else {
      acc = 0;
    }
    requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);

  const cartHash = () => cart?.hash ?? Promise.reject(new Error('no cart loaded'));

  return {
    /** Validates, instantiates and boots a cart. Resolves to its metadata. */
    async load(bytes) {
      if (bytes.byteLength > 32 * 1024 * 1024) throw new Error('cart exceeds 32 MiB');
      const module = await WebAssembly.compile(bytes);
      for (const imp of WebAssembly.Module.imports(module)) {
        if (!(imp.module === 'env' && imp.name === 'trace' && imp.kind === 'function')) {
          throw new Error(`cart imports ${imp.module}.${imp.name}, which WASM-AD does not provide`);
        }
      }
      const meta = readMeta(module);
      const [major, minor] = (meta.spec ?? '').split('.').map(Number);
      if (major !== 0 || minor > (SPEC_VERSION & 0xFF)) console.warn(`cart targets spec ${meta.spec}; this host implements 0.${SPEC_VERSION & 0xFF}`);
      recording = null;
      replay = null;
      cart = {
        module, meta, saveKey: `wasm-ad:${meta.author ?? ''}:${meta.title}`,
        hash: sha256(new Uint8Array(bytes)), icon: readIcon(module),
      };
      await boot();
      return meta;
    },
    reset() {
      if (!cart) return;
      recording = null;
      replay = null;
      return boot();
    },
    get meta() { return cart?.meta ?? null; },
    /** The cart's icon as 32×32 RGBA8 pixels (SPEC.md §2.5), or null. */
    get icon() { return cart?.icon ?? null; },
    /** The icon as a PNG data URL, or null. */
    iconURL() {
      if (!cart?.icon) return null;
      const c = document.createElement('canvas');
      c.width = c.height = 32;
      c.getContext('2d').putImageData(new ImageData(cart.icon, 32, 32), 0, 0);
      return c.toDataURL();
    },
    get running() { return Boolean(machine.exports) && !failed && !paused; },
    pause() { paused = true; keys = 0; clearSound(); },
    resume() { paused = false; },
    /** Buttons held by on-screen controls, OR-ed with keyboard and gamepads. */
    setVirtualKeys(mask) { latched |= mask & ~virtualKeys; virtualKeys = mask; },
    /** Sound volume, 0 to 1. */
    get volume() { return volume; },
    set volume(v) {
      volume = Math.min(1, Math.max(0, v));
      if (audio?.gain) audio.gain.gain.value = volume;
    },
    exportSave() { return machine.exports ? machine.saveRam() : null; },
    importSave(bytes) {
      if (bytes.length !== A.SAVE_SIZE) throw new Error('a save file must be exactly 32 KB');
      persist(bytes);
      recording = null;
      replay = null;
      return boot();
    },

    /** While on, the console steps back through the last seconds of play. */
    setRewinding(on) { rewinding = Boolean(on); },
    get rewinding() { return rewinding || keyRewind; },

    /** The running cart as a .wadstate (FORMATS.md §3). */
    async saveState() {
      if (!machine.exports) throw new Error('no cart loaded');
      const memory = machine.memory(), frames = machine.frames, seed = machine.seed;
      return encodeState({ cartHash: await cartHash(), frames, seed, memory });
    },
    /** Resumes from a .wadstate made for this cart. A recording in progress stops. */
    async loadState(bytes, { force = false } = {}) {
      const state = await decodeState(bytes);
      checkCart(state.cart, await cartHash(), force);
      recording = null;
      replay = null;
      await boot({ state });
    },

    /** Reboots the cart and records from boot. */
    async startRecording() {
      if (!cart) throw new Error('no cart loaded');
      replay = null;
      const { seed, save } = await boot();
      recording = { seed, save, inputs: [] };
    },
    get recording() { return Boolean(recording); },
    /** Stops recording; resolves to the .wadreplay (FORMATS.md §2). */
    async stopRecording() {
      if (!recording) throw new Error('not recording');
      const { seed, save, inputs } = recording;
      recording = null;
      return encodeReplay({ cartHash: await cartHash(), seed, save, inputs: inputs.slice(0, machine.frames) });
    },
    /** Reboots the cart and plays a .wadreplay; live play resumes at its end. */
    async playReplay(bytes, { force = false } = {}) {
      const r = await decodeReplay(bytes);
      checkCart(r.cart, await cartHash(), force);
      recording = null;
      await boot({ seed: r.seed, save: r.save });
      replay = { inputs: r.inputs };
    },
    get replaying() { return Boolean(replay); },
  };
}
