//! The starter cart, the same in every SDK: a scrolling checkerboard blended
//! over a backdrop gradient drawn per scanline, and a spinning star you move
//! with the D-pad (B to go faster, A to toggle mosaic). It uses integer math
//! only, so every language's build renders bit-identical frames.
//!
//!     zig build        # zig-out/bin/starter.wasm

const ad = @import("wasm_ad");

comptime {
    ad.cart(.{
        .title = "Starter",
        .author = "raskell.io",
        .description = "The WASM-AD starter cart, in Zig",
    });
}

const MAP_BLOCK: u16 = 31;

// Cart state lives in linear memory, never in WebAssembly globals (SPEC.md §12);
// a container-level `var` is data in linear memory.
var state: struct {
    buttons: ad.Buttons = .{},
    x: i32 = 112,
    y: i32 = 72,
    mosaic: bool = false,
} = .{};

export fn start() void {
    ad.trace("starter: hello from Zig");

    // A checkerboard of two solid tiles on background 0.
    const bg = ad.bgPalette();
    bg[1] = ad.rgb(255, 255, 255);
    bg[2] = ad.rgb(40, 40, 72);
    const tiles = ad.bgTiles(0);
    tiles[1] = @splat(0x11);
    tiles[2] = @splat(0x22);
    for (ad.screenBlock(MAP_BLOCK), 0..) |*e, i| {
        const col = i % 32;
        const row = i / 32;
        e.* = .init(if ((col / 2 + row / 2) % 2 == 0) 1 else 2, 0);
    }

    // A 16×16 star in four 1D-mapped sprite tiles.
    const obj = ad.objPalette();
    obj[1] = ad.rgb(255, 216, 64);
    obj[2] = ad.rgb(232, 96, 32);
    const star = ad.objTiles();
    var y: i32 = 0;
    while (y < 16) : (y += 1) {
        var x: i32 = 0;
        while (x < 16) : (x += 1) {
            const dx: i32 = @intCast(@abs(x * 2 - 15));
            const dy: i32 = @intCast(@abs(y * 2 - 15));
            const c: u8 = if (dx + dy < 14 or (@min(dx, dy) < 3 and @max(dx, dy) < 15))
                (if (dx + dy < 7) 2 else 1)
            else
                0;
            const tile: usize = @intCast(@divTrunc(y, 8) * 2 + @divTrunc(x, 8));
            const at: usize = @intCast(@mod(y, 8) * 4 + @divTrunc(@mod(x, 8), 2));
            star[tile][at] |= c << @intCast(4 * @mod(x, 2));
        }
    }

    ad.reg.DISPCNT.write(ad.dispcnt.MODE0 | ad.dispcnt.BG0 | ad.dispcnt.OBJ | ad.dispcnt.OBJ_1D);
    ad.reg.BLDCNT.write(ad.blend.cnt(ad.blend.ALPHA, ad.layer.BG0, ad.layer.BACKDROP));
    ad.reg.BLDALPHA.write(ad.blend.alpha(6, 10));
    ad.reg.MOSAIC.write(0x3333);
}

export fn update() void {
    const s = &state;
    s.buttons.update();
    const speed: i32 = if (s.buttons.held(ad.key.B)) 3 else 1;
    if (s.buttons.held(ad.key.LEFT)) s.x -= speed;
    if (s.buttons.held(ad.key.RIGHT)) s.x += speed;
    if (s.buttons.held(ad.key.UP)) s.y -= speed;
    if (s.buttons.held(ad.key.DOWN)) s.y += speed;
    s.x = @max(0, @min(s.x, 240 - 32));
    s.y = @max(0, @min(s.y, 160 - 32));
    if (s.buttons.pressed(ad.key.A)) {
        s.mosaic = !s.mosaic;
    }

    const t: i32 = @bitCast(ad.frame());
    ad.reg.BGHOFS[0].write(@truncate(@as(u32, @bitCast(@divTrunc(t, 2)))));
    ad.reg.BGVOFS[0].write(@truncate(@as(u32, @bitCast(@divTrunc(t, 3)))));
    const cnt = (ad.BgCnt{}).screenBlock(MAP_BLOCK).priority(1);
    ad.reg.BGCNT[0].write(if (s.mosaic) cnt.mosaic().bits else cnt.bits);

    // Spin a quarter turn every 32 frames while the size pulses ±25%.
    const scale = 256 + ((ad.fixed.sin(t *% 3) * 64) >> 14);
    ad.oam.setAffine(0, .rotateScaleFixed(t *% 2, scale, scale));
    const star = ad.Sprite.init(s.x, s.y, .s16x16, 0).affine(0, true);
    ad.oam.set(0, if (s.mosaic) star.mosaic() else star);
}

/// Raster effect: a new backdrop color on every line.
export fn scanline(y: i32) void {
    ad.bgPalette()[0] = ad.rgb(lerp(40, 255, y), lerp(60, 140, y), lerp(160, 96, y));
}

fn lerp(a: i32, b: i32, y: i32) u8 {
    return @truncate(@as(u32, @bitCast(a + @divTrunc((b - a) * y, 159))));
}
