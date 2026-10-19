// Conformance adapter for the browser host, player/web/wasm-ad.js, whose
// `createMachine` has no DOM and runs in Node with the same core.wasm.
//
//   node conformance/run.mjs --host conformance/hosts/web.mjs

import { readFileSync } from 'node:fs';
import { createMachine } from '../../player/web/wasm-ad.js';

const core = (await WebAssembly.instantiate(readFileSync(new URL('../../player/web/core.wasm', import.meta.url)))).instance.exports;

export async function createHost(cart, { seed, save }) {
  const machine = createMachine(core);
  await machine.boot(cart, { seed, save });
  return {
    frame: (input) => machine.frame(input),
    get sound() { return machine.sound; },
  };
}
