#!/usr/bin/env node
// Runs the WASM-AD host conformance suite: every test cart in wasm/, played
// from its replay in replays/, against the frame and sound hashes in
// expected.json. A host implements the spec when it reproduces all of them.
//
//   node conformance/run.mjs                      the reference host (tools/wasmad.mjs)
//   node conformance/run.mjs --host hosts/web.mjs  another host, through an adapter
//   node conformance/run.mjs --only sound          one test
//   node conformance/run.mjs --update              rebuild replays/ and expected.json
//                                                  from tests.json with the reference
//
// An adapter module exports `createHost(cartBytes, { seed, save })`, which may
// be async and returns { frame(input) -> RGBA8 pixels, sound }: see README.md.

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { boot, decodeReplay, encodeReplay, hash, parseKeys } from '../tools/wasmad.mjs';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const SAVE_SIZE = 0x8000;

/** The reference host: tools/wasmad.mjs with player/web/core.wasm. */
export async function createReferenceHost(cart, { seed, save }) {
  return boot(cart, { core: here('../player/web/core.wasm'), seed, save });
}

/** Which frames a test hashes: "scenes" (frames 0, 11 and 29 of each 30) or "every N". */
function marks(rule, frames) {
  const out = [];
  for (let f = 0; f < frames; f++) {
    if (rule === 'scenes' ? [0, 11, 29].includes(f % 30) : f % Number(rule.split(' ')[1]) === 0) out.push(f);
  }
  return out;
}

/** "0-29:1,30-59:15" → frame → PADS. */
function parsePads(script) {
  const spans = script.split(',').map((part) => {
    const [range, mask] = part.split(':');
    const [from, to = from] = range.split('-').map(Number);
    return { from, to, mask: Number(mask) };
  });
  return (f) => spans.find((s) => f >= s.from && f <= s.to)?.mask ?? 1;
}

async function run(createHost, test) {
  const replay = decodeReplay(readFileSync(here(test.replay)));
  const host = await createHost(new Uint8Array(readFileSync(here(test.cart))), { seed: replay.seed, save: replay.save });
  const frames = {}, sound = [];
  const want = new Set(Object.keys(test.frames ?? {}).map(Number));
  for (let f = 0; f < replay.frames; f++) {
    const pixels = host.frame(replay.inputs[f]);
    if (want.has(f) || test.marks?.includes(f)) frames[f] = hash(pixels);
    sound.push(host.sound.slice());
  }
  return { frames, sound: hash(...sound) };
}

async function update() {
  const { tests } = JSON.parse(readFileSync(here('tests.json'), 'utf8'));
  const out = [];
  for (const t of tests) {
    const keys = parseKeys(t.keys ?? '');
    const pads = t.pads ? parsePads(t.pads) : null;
    const inputs = Array.from({ length: t.frames }, (_, f) => {
      const input = keys(f);
      if (pads) input[4] = pads(f);
      return input;
    });
    const save = t.save === 'pattern' ? Uint8Array.from({ length: SAVE_SIZE }, (_, i) => (i * 13 + 7) & 255) : null;
    const replay = `replays/${t.name}.wadreplay`;
    writeFileSync(here(replay), encodeReplay({ cart: null, seed: t.seed ?? 1, save, inputs }));
    const entry = { name: t.name, sections: t.sections, cart: `wasm/${t.cart}.wasm`, replay };
    const result = await run(createReferenceHost, { ...entry, marks: marks(t.hash, t.frames) });
    out.push({ ...entry, frames: result.frames, sound: result.sound });
    console.log(`updated ${t.name}: ${Object.keys(result.frames).length} frames, sound ${result.sound}`);
  }
  const expected = {
    spec: '0.3',
    hashes: {
      frame: 'SHA-256 of the frame as RGBA8 bytes, 240 × 160, rows top to bottom (alpha 255); first 16 hex digits',
      sound: 'SHA-256 of the sound of every frame in order, as i16 little-endian stereo pairs, left first; first 16 hex digits',
    },
    tests: out,
  };
  writeFileSync(here('expected.json'), `${JSON.stringify(expected, null, 2)}\n`);
}

async function check(createHost, only) {
  const { tests } = JSON.parse(readFileSync(here('expected.json'), 'utf8'));
  let failed = 0;
  for (const t of tests) {
    if (only && t.name !== only) continue;
    let problems;
    try {
      const got = await run(createHost, t);
      problems = Object.entries(t.frames).filter(([f, h]) => got.frames[f] !== h).map(([f]) => `frame ${f}`);
      if (got.sound !== t.sound) problems.push('sound');
    } catch (e) {
      problems = [`error: ${e.message}`];
    }
    if (problems.length) {
      failed++;
      const shown = problems.slice(0, 6).join(', ') + (problems.length > 6 ? `, and ${problems.length - 6} more` : '');
      console.log(`FAIL ${t.name.padEnd(13)} ${shown}`);
    } else {
      console.log(`ok   ${t.name.padEnd(13)} ${Object.keys(t.frames).length} frames and sound  (§${t.sections.join(', §')})`);
    }
  }
  console.log(failed ? `${failed} failed` : 'all passed');
  return failed ? 1 : 0;
}

const args = process.argv.slice(2);
const opt = (name) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
if (args.includes('--update')) {
  await update();
} else {
  const hostPath = opt('--host');
  const createHost = hostPath ? (await import(pathToFileURL(resolve(hostPath)).href)).createHost : createReferenceHost;
  process.exit(await check(createHost, opt('--only')));
}
