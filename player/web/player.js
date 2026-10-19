// The standalone WASM-AD player page. The console itself is wasm-ad.js.

import { createConsole, fileKind, W, H } from './wasm-ad.js';

const $ = (id) => document.getElementById(id);
const canvas = $('screen');

function showMessage(text, error = false) {
  $('message').textContent = text;
  $('message').className = error ? 'error' : '';
}

const ad = await createConsole({
  canvas,
  onError: (e) => { showMessage(e.message, true); console.error(e); },
  onReplayEnd: () => showMessage('Replay finished: you have control.'),
});

function download(bytes, name) {
  const a = Object.assign(document.createElement('a'), { href: URL.createObjectURL(new Blob([bytes])), download: name });
  a.click();
  URL.revokeObjectURL(a.href);
}
const fileName = (ext) => `${ad.meta.title.replace(/[^\w.-]+/g, '-')}${ext}`;

function setRecording(on) {
  $('record').setAttribute('aria-pressed', String(on));
  $('record').textContent = on ? '■ Stop' : '● Record';
}

async function loadCart(bytes) {
  const meta = await ad.load(bytes);
  $('info').textContent = [meta.title, meta.author && `by ${meta.author}`, meta.version && `v${meta.version}`]
    .filter(Boolean).join(' · ');
  document.title = `${meta.title} · WASM-AD`;
  const icon = ad.iconURL();
  $('icon').hidden = !icon;
  if (icon) $('icon').src = icon;
  for (const id of ['reset', 'export', 'rewind', 'save-state', 'record']) $(id).disabled = false;
  setRecording(false);
  showMessage('');
}

/** A cart, a .wadstate or a .wadreplay, told apart by their magic numbers (FORMATS.md). */
async function open(promise) {
  try {
    const bytes = new Uint8Array(await promise);
    const kind = fileKind(bytes);
    if (kind === 'cart') return await loadCart(bytes);
    if (!ad.meta) throw new Error(`load the cart before its ${kind === 'state' ? 'save state' : 'replay'}`);
    setRecording(false);
    if (kind === 'state') { await ad.loadState(bytes); showMessage(''); }
    else if (kind === 'replay') { await ad.playReplay(bytes); showMessage('Playing the replay…'); }
    else throw new Error('not a WASM-AD cart, save state or replay');
  } catch (e) {
    showMessage(e.message, true);
    console.error(e);
  }
}

const picked = (id) => $(id).addEventListener('change', (e) => {
  const file = e.target.files[0];
  e.target.value = '';
  if (file) open(file.arrayBuffer());
});
picked('file');
picked('load-state');
picked('play-replay');

$('reset').addEventListener('click', () => { ad.reset(); setRecording(false); showMessage(''); });
$('sound').addEventListener('click', (e) => {
  const on = ad.volume === 0;
  ad.volume = on ? 1 : 0;
  e.currentTarget.textContent = on ? 'Sound on' : 'Sound off';
  e.currentTarget.setAttribute('aria-pressed', String(on));
});

// Rewind while the button is held.
const rewind = $('rewind');
rewind.addEventListener('pointerdown', (e) => { rewind.setPointerCapture(e.pointerId); ad.setRewinding(true); rewind.setAttribute('aria-pressed', 'true'); });
for (const type of ['pointerup', 'pointercancel', 'lostpointercapture']) {
  rewind.addEventListener(type, () => { ad.setRewinding(false); rewind.setAttribute('aria-pressed', 'false'); });
}

$('save-state').addEventListener('click', async () => {
  try { download(await ad.saveState(), fileName('.wadstate')); } catch (e) { showMessage(e.message, true); }
});
$('record').addEventListener('click', async () => {
  try {
    if (ad.recording) {
      setRecording(false);
      download(await ad.stopRecording(), fileName('.wadreplay'));
      showMessage('');
    } else {
      await ad.startRecording();
      setRecording(true);
      showMessage('Recording from boot…');
      setTimeout(() => { if ($('message').textContent === 'Recording from boot…') showMessage(''); }, 1500);
    }
  } catch (e) { showMessage(e.message, true); }
});

$('export').addEventListener('click', () => download(ad.exportSave(), fileName('.sav')));
$('import').addEventListener('change', async (e) => {
  const file = e.target.files[0];
  e.target.value = '';
  if (!file || !ad.meta) return;
  try { await ad.importSave(new Uint8Array(await file.arrayBuffer())); } catch (err) { showMessage(err.message, true); }
});

addEventListener('dragover', (e) => { e.preventDefault(); document.body.classList.add('dragging'); });
addEventListener('dragleave', (e) => { if (e.target === $('drop')) document.body.classList.remove('dragging'); });
addEventListener('drop', (e) => {
  e.preventDefault();
  document.body.classList.remove('dragging');
  const file = e.dataTransfer.files[0];
  if (file) open(file.arrayBuffer());
});

function fit() {
  const scale = Math.max(1, Math.floor(Math.min((innerWidth - 32) / W, (innerHeight - 160) / H)));
  canvas.style.width = `${W * scale}px`;
  canvas.style.height = `${H * scale}px`;
}
addEventListener('resize', fit);
fit();

const url = new URLSearchParams(location.search).get('cart');
if (url) {
  showMessage(`Loading ${url}…`);
  open(fetch(url).then((r) => {
    if (!r.ok) throw new Error(`could not fetch ${url}: ${r.status}`);
    return r.arrayBuffer();
  }));
}
