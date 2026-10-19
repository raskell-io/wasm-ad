# SDKs

| Directory | Language | Starter cart |
|---|---|---|
| [`rust/`](rust) | Rust | `cargo build --release --target wasm32-unknown-unknown -p wasm-ad --example starter` |
| [`c/`](c) | C and C++ | `make -C sdk/c` |
| [`zig/`](zig) | Zig | `cd sdk/zig && zig build` |
| [`assemblyscript/`](assemblyscript) | AssemblyScript | `cd sdk/assemblyscript && npm install && npm run build` |
| [`tinygo/`](tinygo) | Go (TinyGo) | `sdk/tinygo/build.sh examples/starter` |

All of them wrap the same hardware under the names SPEC.md uses, and each
has the same starter cart: a rotating checkerboard (an affine background)
over a per-scanline gradient, a spinning star moved with the D-pad, and a
little tune on every sound channel, and a second star for player 2. The
starters use integer math only,
through a fixed-point sine table every SDK shares (angles in 1/256 turns,
results in Q14), so they render bit-identical frames and sound:

```sh
make sdk-test     # builds every starter whose toolchain is installed and compares them
```

[`conformance.mjs`](conformance.mjs) runs each cart through
`tools/wasmad.mjs check`, plays the same input into all of them on a headless
host (the replay [`starter.wadreplay`](starter.wadreplay), which is not tied to
a cart), and compares frame and sound hashes with the Rust starter's.

A new SDK needs: register access at the fixed addresses of SPEC.md §4–6, the
`wasm-ad` metadata section (or a build step running `tools/wasmad.mjs meta`),
data and stack above 0x28000, no imports besides `env.trace`, cart state in
linear memory rather than mutable globals, and a starter port that passes
`make sdk-test`.
