// WASM-AD host for the browser: implements the host side of SPEC.md, and
// renders and makes sound with the reference core (core.wasm, from the
// wasm-ad-core crate).
//
//   import { createConsole } from './wasm-ad.js';
//   const console = await createConsole({ canvas });
//   await console.load(await (await fetch('cart.wasm')).arrayBuffer());

export const W = 240, H = 160;
const STEP = 1000 / 60;
const A = {
  IO: 0x400, PAL_END: 0xC00, VIDEO_END: 0x19000, OAM: 0xC00, SAVE: 0x19000, SAVE_SIZE: 0x8000,
  CART: 0x28000, VCOUNT: 0x406, KEYS: 0x700, FRAME: 0x704, SEED: 0x708,
  SAVECNT: 0x70C, SAVESTAT: 0x70E, SPEC: 0x710,
  BG2PA: 0x420, BG2PD: 0x426, BG3PA: 0x430, BG3PD: 0x436,
  SOUND_REGS: 0x460, SOUND_REGS_END: 0x4D0, SOUND_STATE: 0x780, SOUND_STATE_END: 0x800,
};
export const SPEC_VERSION = 0x0002;
const SOUND_RATE = 48000;

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
export const SAVE_SIZE = A.SAVE_SIZE;

/** `KEYS` bits (SPEC.md §5). */
export const KEY = { a: 1, b: 2, select: 4, start: 8, right: 16, left: 32, up: 64, down: 128, r: 256, l: 512 };

export const KEYMAP = {
  ArrowRight: KEY.right, ArrowLeft: KEY.left, ArrowUp: KEY.up, ArrowDown: KEY.down,
  KeyX: KEY.a, KeyZ: KEY.b, KeyA: KEY.l, KeyS: KEY.r,
  Enter: KEY.start, ShiftLeft: KEY.select, ShiftRight: KEY.select,
};

function gamepadKeys() {
  let k = 0;
  for (const pad of navigator.getGamepads?.() ?? []) {
    if (!pad) continue;
    const on = (i) => pad.buttons[i]?.pressed;
    const [x = 0, y = 0] = pad.axes;
    // Standard mapping, Nintendo positions: the right face button is A.
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
  }
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

/**
 * Creates a console drawing into `canvas`.
 *
 * Options:
 * - `core`: URL of core.wasm (defaults to next to this module)
 * - `keyboard`: listen to the keyboard (default true)
 * - `active()`: whether keyboard input should go to the console right now;
 *   arrow keys only stop scrolling the page while it returns true
 * - `sound`: play sound (default true). Browsers only allow it after the
 *   user interacts with the page, so it starts on the first key or click.
 * - `volume`: 0 to 1 (default 1)
 * - `onError(error)`, `onTrace(message, meta)`
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
}) {
  const ctx = canvas.getContext('2d');
  const image = ctx.createImageData(W, H);
  const core = (await WebAssembly.instantiateStreaming(fetch(coreUrl))).instance.exports;

  let cart = null; // { module, meta, saveKey, exports, frame }
  // `latched` remembers presses until a frame has seen them, so a tap shorter
  // than one frame still reaches the cart.
  let keys = 0, virtualKeys = 0, latched = 0, failed = false, paused = false;

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
    const pcm = new Int16Array(core.memory.buffer, core.ad_sound_buffer(), 1600);
    const f = new Float32Array(1600);
    for (let i = 0; i < 1600; i++) f[i] = pcm[i] / 32768;
    audio.node.port.postMessage(f, [f.buffer]);
  }
  const clearSound = () => audio?.node?.port.postMessage('clear');

  if (keyboard) {
    addEventListener('keydown', (e) => {
      if (!(e.code in KEYMAP) || e.metaKey || e.ctrlKey || !cart || !active()) return;
      const t = e.target;
      if (t instanceof HTMLElement && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
      keys |= KEYMAP[e.code];
      latched |= KEYMAP[e.code];
      e.preventDefault();
    });
    addEventListener('keyup', (e) => { if (e.code in KEYMAP) keys &= ~KEYMAP[e.code]; });
    addEventListener('blur', () => { keys = 0; });
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

  // SPEC.md §3.1
  function boot() {
    const decoder = new TextDecoder();
    let instance;
    const imports = {
      env: {
        trace: (ptr, len) => onTrace(decoder.decode(new Uint8Array(instance.exports.memory.buffer, ptr, len)), cart.meta),
      },
    };
    instance = new WebAssembly.Instance(cart.module, imports);
    const ex = instance.exports;
    if (!(ex.memory instanceof WebAssembly.Memory)) throw new Error('cart does not export memory');
    if (typeof ex.update !== 'function') throw new Error('cart does not export update()');
    if (ex.memory.buffer.byteLength < 3 * 65536) throw new Error('cart memory is smaller than 3 pages');

    const mem = new Uint8Array(ex.memory.buffer);
    mem.fill(0, 0, A.CART);
    const dv = new DataView(ex.memory.buffer);
    for (let i = 0; i < 128; i++) dv.setUint16(A.OAM + i * 8, 0x0200, true);
    for (const a of [A.BG2PA, A.BG2PD, A.BG3PA, A.BG3PD]) dv.setUint16(a, 0x100, true);
    dv.setUint32(A.SEED, crypto.getRandomValues(new Uint32Array(1))[0], true);
    dv.setUint16(A.SPEC, SPEC_VERSION, true);
    const saved = loadSave();
    if (saved) { mem.set(saved, A.SAVE); dv.setUint16(A.SAVESTAT, 1, true); }

    cart.exports = ex;
    cart.frame = 0;
    failed = false;
    clearSound();
    ex.start?.();
  }

  // SPEC.md §3.2
  function runFrame() {
    const ex = cart.exports;
    let dv = new DataView(ex.memory.buffer);
    dv.setUint16(A.KEYS, (keys | virtualKeys | latched | gamepadKeys()) & 0x3FF, true);
    latched = 0;
    dv.setUint32(A.FRAME, cart.frame >>> 0, true);
    dv.setUint16(A.VCOUNT, 160, true);
    ex.update();

    // update() may have grown memory, which replaces the buffer.
    const coreMem = new Uint8Array(core.memory.buffer, core.ad_mem(), core.ad_mem_len());
    coreMem.set(new Uint8Array(ex.memory.buffer, 0, A.CART));
    // Sound for this frame (§14.2); the core updates registers and state.
    core.ad_sound_frame();
    const cartMem = new Uint8Array(ex.memory.buffer);
    for (const [from, to] of [[A.SOUND_REGS, A.SOUND_REGS_END], [A.SOUND_STATE, A.SOUND_STATE_END]]) {
      cartMem.set(coreMem.subarray(from, to), from);
    }
    postSound();
    for (let y = 0; y < H; y++) {
      if (ex.scanline) {
        dv = new DataView(ex.memory.buffer);
        dv.setUint16(A.VCOUNT, y, true);
        ex.scanline(y);
        // Only registers and palette change mid-frame; VRAM and OAM stay latched.
        coreMem.set(new Uint8Array(ex.memory.buffer, A.IO, A.PAL_END - A.IO), A.IO);
      }
      core.ad_render_line(y);
    }

    dv = new DataView(ex.memory.buffer);
    if (dv.getUint16(A.SAVECNT, true) & 1) {
      persist(saveBytes());
      dv.setUint16(A.SAVECNT, dv.getUint16(A.SAVECNT, true) & ~1, true);
    }
    cart.frame++;
  }

  function saveBytes() {
    return new Uint8Array(cart.exports.memory.buffer, A.SAVE, A.SAVE_SIZE).slice();
  }

  function present() {
    image.data.set(new Uint8Array(core.memory.buffer, core.ad_framebuffer(), W * H * 4));
    ctx.putImageData(image, 0, 0);
  }

  // Fixed timestep: carts run at 60 Hz whatever the display refresh rate.
  let last = performance.now(), acc = 0;
  function tick(now) {
    acc = Math.min(acc + now - last, STEP * 5);
    last = now;
    if (cart?.exports && !failed && !paused) {
      try {
        let ran = false;
        while (acc >= STEP) { runFrame(); acc -= STEP; ran = true; }
        if (ran) present();
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
      if (major !== 0 || minor > (SPEC_VERSION & 0xFF)) console.warn(`cart targets spec ${meta.spec}; this host implements 0.2`);
      cart = { module, meta, saveKey: `wasm-ad:${meta.author ?? ''}:${meta.title}` };
      boot();
      return meta;
    },
    reset() { if (cart) boot(); },
    get meta() { return cart?.meta ?? null; },
    get running() { return Boolean(cart?.exports) && !failed && !paused; },
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
    exportSave() { return cart?.exports ? saveBytes() : null; },
    importSave(bytes) {
      if (bytes.length !== A.SAVE_SIZE) throw new Error('a save file must be exactly 32 KB');
      persist(bytes);
      boot();
    },
  };
}
