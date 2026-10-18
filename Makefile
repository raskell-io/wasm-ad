TARGET := target/wasm32-unknown-unknown/release
PORT ?= 8643
CARTS := twirl hello

.PHONY: all player carts serve test sdk-test clean

all: player carts

# The reference renderer, compiled for the web player.
player:
	cargo build --release --target wasm32-unknown-unknown -p wasm-ad-core --features player
	wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int $(TARGET)/wasm_ad_core.wasm -o player/web/core.wasm 2>/dev/null \
		|| cp $(TARGET)/wasm_ad_core.wasm player/web/core.wasm

carts:
	mkdir -p player/web/carts
	cargo build --release --target wasm32-unknown-unknown $(addprefix -p ,$(CARTS))
	for c in $(CARTS); do cp $(TARGET)/$$c.wasm player/web/carts/$$c.wasm; done
	cargo build --release --target wasm32-unknown-unknown -p wasm-ad --example starter
	cp $(TARGET)/examples/starter.wasm player/web/carts/starter.wasm

serve: all
	@echo "http://localhost:$(PORT)/?cart=carts/twirl.wasm"
	python3 -m http.server $(PORT) --directory player/web

test:
	cargo test

# Every SDK's starter cart, checked against the spec and against each other.
sdk-test: player
	sdk/test.sh

clean:
	cargo clean
	rm -rf player/web/core.wasm player/web/carts
