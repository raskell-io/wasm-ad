#!/usr/bin/env node
// Checks every SDK's starter cart against SPEC.md and against the Rust one:
// all of them must render bit-identical frames and sound for the same input.
//
//   node sdk/conformance.mjs rust=a.wasm zig=b.wasm ...      (`make sdk-test`)

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { boot, check, decodeReplay, hash } from '../tools/wasmad.mjs';

// The input: a replay not tied to any cart (FORMATS.md), recorded with
//   wasmad run <starter> --frames 121 --unbound --record sdk/starter.wadreplay \
//     --keys '0-29:right,30-59:down+b+right@2,60:a,61-90:left+up@2,91-120:up+b+down@2+left@2'
const REPLAY = decodeReplay(readFileSync(new URL('starter.wadreplay', import.meta.url)));
const MARKS = [0, 29, 45, 60, 61, 100, 120];
const core = fileURLToPath(new URL('../player/web/core.wasm', import.meta.url));

async function hashes(bytes) {
  const traces = [];
  const host = await boot(bytes, { core, seed: REPLAY.seed, save: REPLAY.save, onTrace: (m) => traces.push(m) });
  const out = [], sound = [];
  for (let f = 0; f <= MARKS.at(-1); f++) {
    const pixels = host.frame(REPLAY.inputs[f]);
    sound.push(host.sound);
    if (MARKS.includes(f)) out.push(hash(pixels));
  }
  out.push(hash(...sound));
  return { out, traces };
}

let reference = null, failed = 0;
for (const arg of process.argv.slice(2)) {
  const [lang, file] = arg.split('=');
  const bytes = new Uint8Array(readFileSync(file));
  const { errors, meta } = check(bytes);
  if (errors.length) {
    console.log(`FAIL ${lang}: ${errors.join('; ')}`);
    failed++;
    continue;
  }
  const { out, traces } = await hashes(bytes);
  reference ??= { lang, out };
  const diff = [...MARKS, 'sound'].filter((_, i) => out[i] !== reference.out[i]);
  if (diff.length) {
    console.log(`FAIL ${lang}: ${diff.map((d) => (d === 'sound' ? 'sound' : `frame ${d}`)).join(', ')} differ from ${reference.lang}`);
    failed++;
  } else {
    console.log(`ok   ${lang.padEnd(15)} ${String(bytes.length).padStart(6)} bytes  "${meta.description}"  trace: ${traces.join(' | ')}`);
  }
}
process.exit(failed ? 1 : 0);
