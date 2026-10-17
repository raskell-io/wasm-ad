// WASM-AD web player: implements the host side of SPEC.md and renders with
// the reference renderer (core.wasm, built from the wasm-ad-core crate).

const W = 240, H = 160, STEP = 1000 / 60;
const A = {
  IO: 0x400, PAL_END: 0xC00, VIDEO_END: 0x19000, OAM: 0xC00, SAVE: 0x19000, SAVE_SIZE: 0x8000,
  CART: 0x28000, VCOUNT: 0x406, KEYS: 0x700, FRAME: 0x704, SEED: 0x708,
  SAVECNT: 0x70C, SAVESTAT: 0x70E, SPEC: 0x710,
};
const SPEC_VERSION = 0x0001;
const KEY = { a: 1, b: 2, select: 4, start: 8, right: 16, left: 32, up: 64, down: 128, r: 256, l: 512 };
const KEYMAP = {
  ArrowRight: KEY.right, ArrowLeft: KEY.left, ArrowUp: KEY.up, ArrowDown: KEY.down,
  KeyX: KEY.a, KeyZ: KEY.b, KeyA: KEY.l, KeyS: KEY.r,
  Enter: KEY.start, ShiftLeft: KEY.select, ShiftRight: KEY.select,
};

const $ = (id) => document.getElementById(id);
const canvas = $('screen');
const ctx = canvas.getContext('2d');
const image = ctx.createImageData(W, H);

const core = (await WebAssembly.instantiateStreaming(fetch('core.wasm'))).instance.exports;

let cart = null;   // { exports, meta, module, frame, saveKey }
let keys = 0;

// ---- Input ----------------------------------------------------------------

addEventListener('keydown', (e) => {
  if (e.code in KEYMAP && !e.metaKey && !e.ctrlKey) { keys |= KEYMAP[e.code]; e.preventDefault(); }
});
addEventListener('keyup', (e) => { if (e.code in KEYMAP) keys &= ~KEYMAP[e.code]; });
addEventListener('blur', () => { keys = 0; });

function gamepadKeys() {
  let k = 0;
  for (const pad of navigator.getGamepads?.() ?? []) {
    if (!pad) continue;
    const on = (i) => pad.buttons[i]?.pressed;
    const [x = 0, y = 0] = pad.axes;
    // Standard mapping, Nintendo positions: right face button is A.
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

// ---- Loading --------------------------------------------------------------

function parseMeta(module) {
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

async function loadCart(bytes) {
  if (bytes.byteLength > 32 * 1024 * 1024) throw new Error('cart exceeds 32 MiB');
  const module = await WebAssembly.compile(bytes);
  for (const imp of WebAssembly.Module.imports(module)) {
    if (!(imp.module === 'env' && imp.name === 'trace' && imp.kind === 'function')) {
      throw new Error(`cart imports ${imp.module}.${imp.name}, which WASM-AD does not provide`);
    }
  }
  const meta = parseMeta(module);
  const [major, minor] = (meta.spec ?? '').split('.').map(Number);
  if (major !== 0 || minor > (SPEC_VERSION & 0xFF)) console.warn(`cart targets spec ${meta.spec}; this player implements 0.1`);
  cart = { module, meta, saveKey: `wasm-ad:${meta.author ?? ''}:${meta.title}` };
  boot();
  $('info').textContent = [meta.title, meta.author && `by ${meta.author}`, meta.version && `v${meta.version}`]
    .filter(Boolean).join(' · ');
  document.title = `${meta.title} · WASM-AD`;
  for (const id of ['reset', 'export']) $(id).disabled = false;
  showMessage('');
}

function boot() {
  const decoder = new TextDecoder();
  let instance;
  const imports = {
    env: {
      trace: (ptr, len) => console.log(`[${cart.meta.title}]`, decoder.decode(new Uint8Array(instance.exports.memory.buffer, ptr, len))),
    },
  };
  instance = new WebAssembly.Instance(cart.module, imports);
  const ex = instance.exports;
  if (!(ex.memory instanceof WebAssembly.Memory)) throw new Error('cart does not export memory');
  if (typeof ex.update !== 'function') throw new Error('cart does not export update()');
  if (ex.memory.buffer.byteLength < 3 * 65536) throw new Error('cart memory is smaller than 3 pages');

  // SPEC.md §3.1
  const mem = new Uint8Array(ex.memory.buffer);
  mem.fill(0, 0, A.CART);
  const dv = new DataView(ex.memory.buffer);
  for (let i = 0; i < 128; i++) dv.setUint16(A.OAM + i * 8, 0x0200, true);
  dv.setUint32(A.SEED, crypto.getRandomValues(new Uint32Array(1))[0], true);
  dv.setUint16(A.SPEC, SPEC_VERSION, true);
  const saved = loadSave();
  if (saved) { mem.set(saved, A.SAVE); dv.setUint16(A.SAVESTAT, 1, true); }

  cart.exports = ex;
  cart.frame = 0;
  ex.start?.();
}

// ---- Saves ----------------------------------------------------------------

function loadSave() {
  try {
    const b64 = localStorage.getItem(cart.saveKey);
    if (!b64) return null;
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    return bytes.length === A.SAVE_SIZE ? bytes : null;
  } catch { return null; }
}

function saveBytes() {
  return new Uint8Array(cart.exports.memory.buffer, A.SAVE, A.SAVE_SIZE).slice();
}

function persist(bytes) {
  let bin = '';
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  try { localStorage.setItem(cart.saveKey, btoa(bin)); } catch (e) { console.warn('could not persist save', e); }
}

// ---- Frame (SPEC.md §3.2) --------------------------------------------------

function runFrame() {
  const ex = cart.exports;
  let dv = new DataView(ex.memory.buffer);
  dv.setUint16(A.KEYS, (keys | gamepadKeys()) & 0x3FF, true);
  dv.setUint32(A.FRAME, cart.frame >>> 0, true);
  dv.setUint16(A.VCOUNT, 160, true);
  ex.update();

  // update() may have grown memory, which replaces the buffer.
  const coreMem = new Uint8Array(core.memory.buffer, core.ad_mem(), core.ad_mem_len());
  coreMem.set(new Uint8Array(ex.memory.buffer, 0, A.VIDEO_END));
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

function present() {
  image.data.set(new Uint8Array(core.memory.buffer, core.ad_framebuffer(), W * H * 4));
  ctx.putImageData(image, 0, 0);
}

let last = performance.now(), acc = 0, failed = false;
function tick(now) {
  acc = Math.min(acc + now - last, STEP * 5);
  last = now;
  if (cart && !failed) {
    try {
      let ran = false;
      while (acc >= STEP) { runFrame(); acc -= STEP; ran = true; }
      if (ran) present();
    } catch (e) {
      failed = true;
      showMessage(`Cart crashed: ${e.message}`, true);
      console.error(e);
    }
  } else {
    acc = 0;
  }
  requestAnimationFrame(tick);
}
requestAnimationFrame(tick);

// ---- UI -------------------------------------------------------------------

function showMessage(text, error = false) {
  $('message').textContent = text;
  $('message').className = error ? 'error' : '';
}

async function loadFrom(promise) {
  try {
    failed = false;
    await loadCart(await promise);
  } catch (e) {
    showMessage(e.message, true);
    console.error(e);
  }
}

$('file').addEventListener('change', (e) => {
  const file = e.target.files[0];
  if (file) loadFrom(file.arrayBuffer());
  e.target.value = '';
});
$('reset').addEventListener('click', () => { failed = false; boot(); showMessage(''); });
$('export').addEventListener('click', () => {
  const blob = new Blob([saveBytes()], { type: 'application/octet-stream' });
  const a = Object.assign(document.createElement('a'), {
    href: URL.createObjectURL(blob), download: `${cart.meta.title}.sav`,
  });
  a.click();
  URL.revokeObjectURL(a.href);
});
$('import').addEventListener('change', async (e) => {
  const file = e.target.files[0];
  e.target.value = '';
  if (!file || !cart) return;
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (bytes.length !== A.SAVE_SIZE) return showMessage('A save file must be exactly 32 KB.', true);
  persist(bytes);
  failed = false;
  boot();
});

addEventListener('dragover', (e) => { e.preventDefault(); document.body.classList.add('dragging'); });
addEventListener('dragleave', (e) => { if (e.target === $('drop')) document.body.classList.remove('dragging'); });
addEventListener('drop', (e) => {
  e.preventDefault();
  document.body.classList.remove('dragging');
  const file = e.dataTransfer.files[0];
  if (file) loadFrom(file.arrayBuffer());
});

function fit() {
  const scale = Math.max(1, Math.floor(Math.min((innerWidth - 32) / W, (innerHeight - 120) / H)));
  canvas.style.width = `${W * scale}px`;
  canvas.style.height = `${H * scale}px`;
}
addEventListener('resize', fit);
fit();

const url = new URLSearchParams(location.search).get('cart');
if (url) {
  showMessage(`Loading ${url}…`);
  loadFrom(fetch(url).then((r) => {
    if (!r.ok) throw new Error(`could not fetch ${url}: ${r.status}`);
    return r.arrayBuffer();
  }));
}
