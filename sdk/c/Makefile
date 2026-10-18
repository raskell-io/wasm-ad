# Builds the starter cart from the same source as C (build/starter.wasm) and
# as C++ (build/starter-cpp.wasm).
#
# Needs clang with the WebAssembly backend and wasm-ld (LLVM's lld). Apple's
# clang has no WebAssembly backend; on macOS use Homebrew's llvm and lld, which
# this file finds on its own.

BREW_LLVM := $(lastword $(sort $(wildcard /opt/homebrew/opt/llvm@*/bin /usr/local/opt/llvm@*/bin)))
BREW_LLD := $(lastword $(sort $(wildcard /opt/homebrew/opt/lld@*/bin /usr/local/opt/lld@*/bin)))
# clang finds wasm-ld on PATH.
export PATH := $(BREW_LLD):$(PATH)
CLANG := $(if $(BREW_LLVM),$(BREW_LLVM)/,)clang

# SPEC.md §13: data at 0x28000 (163840) with a 64 KiB stack above it. wasm-ld
# places the stack after the data unless told --stack-first.
CFLAGS := --target=wasm32 -O2 -nostdlib -ffreestanding -Wall -Wextra
LDFLAGS := -Wl,--no-entry -Wl,--global-base=163840 -Wl,-z,stack-size=65536 -Wl,--strip-debug

.PHONY: all clean
all: build/starter.wasm build/starter-cpp.wasm

build/starter.wasm: examples/starter.c wasm_ad.h
	@mkdir -p build
	$(CLANG) -std=c11 $(CFLAGS) $(LDFLAGS) $< -o $@

build/starter-cpp.wasm: examples/starter.c wasm_ad.h
	@mkdir -p build
	$(CLANG)++ -x c++ -std=c++17 $(CFLAGS) $(LDFLAGS) $< -o $@

clean:
	rm -rf build
