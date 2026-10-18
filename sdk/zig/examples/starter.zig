//! The starter cart, the same in every SDK: a rotating checkerboard (an
//! affine background) blended over a backdrop gradient drawn per scanline,
//! a spinning star you move with the D-pad, and a little tune on all four
//! sound channels. B goes faster and plays a sample; A toggles mosaic with a
//! rising blip. It uses integer math only, so every language's build renders
//! bit-identical frames and sound.
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

const MAP_BLOCK: u16 = 8;
/// The zap sample's length in sample RAM.
const ZAP_LEN: u16 = 2048;
/// An arpeggio for the square channel and a bass line for the wave channel, in Hz.
const NOTES = [8]u32{ 262, 330, 392, 523, 392, 330, 262, 196 };
const BASS = [4]u32{ 65, 55, 44, 49 };

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

    // A checkerboard of two solid 8bpp tiles on affine background 2, 32×32 tiles.
    const bg = ad.bgPalette();
    bg[1] = ad.rgb(255, 255, 255);
    bg[2] = ad.rgb(40, 40, 72);
    const tiles = ad.bgTiles8(0);
    tiles[1] = @splat(1);
    tiles[2] = @splat(2);
    for (ad.affineMap(MAP_BLOCK)[0 .. 32 * 32], 0..) |*e, i| {
        const col = i % 32;
        const row = i / 32;
        e.* = if ((col / 2 + row / 2) % 2 == 0) 1 else 2;
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

    ad.reg.DISPCNT.write(ad.dispcnt.MODE1 | ad.dispcnt.BG2 | ad.dispcnt.OBJ | ad.dispcnt.OBJ_1D);
    ad.reg.BLDCNT.write(ad.blend.cnt(ad.blend.ALPHA, ad.layer.BG2, ad.layer.BACKDROP));
    ad.reg.BLDALPHA.write(ad.blend.alpha(6, 10));
    ad.reg.MOSAIC.write(0x3333);

    // A triangle wave for the bass, and a falling zap in sample RAM.
    const wave = ad.sound.waveRam();
    for (wave[0..16], 0..) |*b, i| {
        b.* = (level(i * 2) << 4) | level(i * 2 + 1);
    }
    const zap = ad.sound.sampleRam();
    var i: i32 = 0;
    while (i < ZAP_LEN) : (i += 1) {
        const v = (ad.fixed.sin((i * i) >> 9) * 127) >> 14;
        zap[@intCast(i)] = @truncate((v * (ZAP_LEN - i)) >> 11);
    }
    ad.sound.enable();
}

fn level(j: usize) u8 {
    return @intCast(if (j < 16) j else 31 - j);
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
        // A rising blip: the sweep raises the pitch until it runs out.
        ad.reg.SOUND1CNT_L.write(ad.sound.sweep(2, 2, false));
        ad.reg.SOUND1CNT_H.write(ad.sound.DUTY_50 | ad.sound.envelope(12, 2, false));
        ad.reg.SOUND1CNT_X.write(ad.sound.START | ad.sound.rate(440));
    }
    if (s.buttons.pressed(ad.key.B)) {
        ad.pcm.play(0, 0, ZAP_LEN, 16000, 48);
    }

    const t: i32 = @bitCast(ad.frame());
    // The tune: a note every 16 frames, a hi-hat between them, bass every 64.
    if (@rem(t, 16) == 0) {
        ad.reg.SOUND2CNT_L.write(ad.sound.DUTY_25 | ad.sound.envelope(9, 1, false));
        ad.reg.SOUND2CNT_H.write(ad.sound.START | ad.sound.rate(NOTES[@intCast(@rem(@divTrunc(t, 16), 8))]));
    }
    if (@rem(t, 16) == 8) {
        ad.reg.SOUND4CNT_L.write(ad.sound.envelope(6, 1, false));
        ad.reg.SOUND4CNT_H.write(ad.sound.START | ad.sound.noise(1, 1, false));
    }
    if (@rem(t, 64) == 0) {
        ad.reg.SOUND3CNT_L.write(ad.sound.WAVE_ON);
        ad.reg.SOUND3CNT_H.write(ad.sound.WAVE_50 | (256 - 48));
        ad.reg.SOUND3CNT_X.write(ad.sound.START | ad.sound.LENGTH | ad.sound.rate(BASS[@intCast(@rem(@divTrunc(t, 64), 4))] * 2));
    }

    // Turn once every 1024 frames around the screen centre, scrolling diagonally.
    const cnt = (ad.BgCnt{}).screenBlock(MAP_BLOCK).affineSize(.s32x32).wrap().priority(1);
    ad.reg.BGCNT[2].write(if (s.mosaic) cnt.mosaic().bits else cnt.bits);
    const m = ad.AffineMatrix.rotateScaleFixed(@divTrunc(t, 4), 256, 256);
    ad.setBgAffine(2, m, @rem(@divTrunc(t, 2), 256), @rem(@divTrunc(t, 3), 256), 120, 80);

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
