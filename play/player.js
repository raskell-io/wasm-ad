// The standalone WASM-AD player page. The console itself is wasm-ad.js.

import { createConsole, W, H } from './wasm-ad.js';

const $ = (id) => document.getElementById(id);
const canvas = $('screen');

function showMessage(text, error = false) {
  $('message').textContent = text;
  $('message').className = error ? 'error' : '';
}

const ad = await createConsole({ canvas, onError: (e) => { showMessage(e.message, true); console.error(e); } });

async function loadFrom(promise) {
  try {
    const meta = await ad.load(await promise);
    $('info').textContent = [meta.title, meta.author && `by ${meta.author}`, meta.version && `v${meta.version}`]
      .filter(Boolean).join(' · ');
    document.title = `${meta.title} · WASM-AD`;
    for (const id of ['reset', 'export']) $(id).disabled = false;
    showMessage('');
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
$('reset').addEventListener('click', () => { ad.reset(); showMessage(''); });
$('export').addEventListener('click', () => {
  const blob = new Blob([ad.exportSave()], { type: 'application/octet-stream' });
  const a = Object.assign(document.createElement('a'), { href: URL.createObjectURL(blob), download: `${ad.meta.title}.sav` });
  a.click();
  URL.revokeObjectURL(a.href);
});
$('import').addEventListener('change', async (e) => {
  const file = e.target.files[0];
  e.target.value = '';
  if (!file || !ad.meta) return;
  try { ad.importSave(new Uint8Array(await file.arrayBuffer())); } catch (err) { showMessage(err.message, true); }
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
