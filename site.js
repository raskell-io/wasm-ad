import { createConsole, KEY } from './play/wasm-ad.js';

const $ = (sel) => document.querySelector(sel);
const handheld = $('#handheld');
const dock = $('#play');
const nowPlaying = $('#now-playing');

// The console peeks out of the hero until the visitor clicks into it; then it
// rises and takes keyboard input. Until then arrow keys keep scrolling the page.
let engaged = false;
function engage(on) {
  engaged = on;
  handheld.classList.toggle('engaged', on);
  dock.classList.toggle('raised', on);
}
function bringIntoView() {
  dock.scrollIntoView({ behavior: 'smooth', block: 'end' });
}
document.addEventListener('pointerdown', (e) => {
  const wasEngaged = engaged;
  engage(Boolean(e.target.closest('#handheld, .cart, #play-cta')));
  if (!wasEngaged && e.target.closest('#handheld')) setTimeout(bringIntoView, 350);
});
$('#play-cta').addEventListener('click', (e) => {
  e.preventDefault();
  engage(true);
  setTimeout(bringIntoView, 350);
});

function status(html) { nowPlaying.innerHTML = html; }
function escape(s) { return s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]); }

const ad = await createConsole({
  canvas: $('#screen'),
  core: new URL('./play/core.wasm', import.meta.url),
  active: () => engaged,
  onError: (e) => status(`<span class="err">${escape(e.message)}</span>`),
});

async function load(bytes, button) {
  try {
    const meta = await ad.load(bytes);
    document.querySelectorAll('.cart').forEach((c) => c.classList.toggle('active', c === button));
    status(`Now playing: <b>${escape(meta.title)}</b>${meta.author ? ` by ${escape(meta.author)}` : ''}`);
  } catch (e) {
    status(`<span class="err">${escape(e.message)}</span>`);
  }
}

async function loadUrl(url, button) {
  const res = await fetch(url);
  if (!res.ok) return status(`<span class="err">Could not load ${escape(url)}</span>`);
  await load(await res.arrayBuffer(), button);
}

// Carts
for (const cart of document.querySelectorAll('.cart[data-cart]')) {
  cart.addEventListener('click', async () => {
    await loadUrl(cart.dataset.cart, cart);
    engage(true);
    setTimeout(bringIntoView, 350);
  });
}
const own = $('#own-cart');
$('#cart-file').addEventListener('change', async (e) => {
  const file = e.target.files[0];
  e.target.value = '';
  if (!file) return;
  await load(await file.arrayBuffer(), own);
  engage(true);
  setTimeout(bringIntoView, 350);
});
for (const target of [own, handheld]) {
  target.addEventListener('dragover', (e) => { e.preventDefault(); own.classList.add('dragging'); });
  target.addEventListener('dragleave', () => own.classList.remove('dragging'));
  target.addEventListener('drop', async (e) => {
    e.preventDefault();
    own.classList.remove('dragging');
    const file = e.dataTransfer.files[0];
    if (file) { await load(await file.arrayBuffer(), own); engage(true); }
  });
}

// On-screen buttons, multi-touch: each pointer holds one button.
const held = new Map();
function syncVirtual() {
  let mask = 0;
  for (const btn of held.values()) mask |= KEY[btn.dataset.key];
  ad.setVirtualKeys(mask);
  for (const b of handheld.querySelectorAll('[data-key]')) b.classList.toggle('pressed', [...held.values()].includes(b));
}
handheld.addEventListener('pointerdown', (e) => {
  const btn = e.target.closest('[data-key]');
  if (!btn) return;
  e.preventDefault();
  btn.setPointerCapture?.(e.pointerId);
  held.set(e.pointerId, btn);
  syncVirtual();
});
for (const type of ['pointerup', 'pointercancel', 'lostpointercapture']) {
  handheld.addEventListener(type, (e) => { if (held.delete(e.pointerId)) syncVirtual(); });
}
// Keyboard presses light up the matching on-screen buttons too.
const KEYCODES = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right', KeyX: 'a', KeyZ: 'b', KeyA: 'l', KeyS: 'r', Enter: 'start', ShiftLeft: 'select', ShiftRight: 'select' };
for (const [type, on] of [['keydown', true], ['keyup', false]]) {
  addEventListener(type, (e) => {
    const k = KEYCODES[e.code];
    if (k && (engaged || !on)) handheld.querySelector(`[data-key="${k}"]`)?.classList.toggle('pressed', on);
  });
}

$('#engage').addEventListener('click', () => engage(true));

// Attract mode: boot Twirl; its stick spins safely in the start room until played.
await loadUrl('carts/twirl.wasm', $('.cart[data-cart="carts/twirl.wasm"]'));
