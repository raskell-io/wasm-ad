TARGET := target/wasm32-unknown-unknown/release
PORT ?= 8643
CARTS := twirl hello

DIST := dist
SITE_PORT ?= 8644
# A rainbow-river checkout; its cart is bundled into the site when present.
RAINBOW_RIVER ?= ../rainbow-river

.PHONY: all player carts serve site site-serve test clean

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

serve: all
	@echo "http://localhost:$(PORT)/?cart=carts/twirl.wasm"
	python3 -m http.server $(PORT) --directory player/web

# The website: site/ plus the player, the carts and the spec, in dist/.
site: all
	rm -rf $(DIST)
	mkdir -p $(DIST)/play $(DIST)/carts
	cp -R site/. $(DIST)/
	cp player/web/index.html player/web/player.js player/web/wasm-ad.js player/web/core.wasm $(DIST)/play/
	cp player/web/carts/*.wasm $(DIST)/carts/
	cp SPEC.md $(DIST)/
	if [ -d $(RAINBOW_RIVER) ]; then \
		$(MAKE) -C $(RAINBOW_RIVER) build && cp $(RAINBOW_RIVER)/web/carts/rainbow-river.wasm $(DIST)/carts/; \
	else echo "note: $(RAINBOW_RIVER) not found, Rainbow River cart not bundled"; fi

site-serve: site
	@echo "http://localhost:$(SITE_PORT)/"
	python3 -m http.server $(SITE_PORT) --directory $(DIST)

test:
	cargo test

clean:
	cargo clean
	rm -rf player/web/core.wasm player/web/carts $(DIST)
