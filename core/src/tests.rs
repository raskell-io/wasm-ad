use super::*;

const RED: u16 = 31;
const GREEN: u16 = 31 << 5;
const BLUE: u16 = 31 << 10;
const WHITE: u16 = 0x7FFF;

struct Mem(Vec<u8>);

impl Mem {
    /// Memory as the host leaves it at boot: zeroed, all sprites hidden.
    fn new() -> Self {
        let mut m = Self(vec![0; VIDEO_END]);
        for i in 0..128 {
            m.w16(OAM + i * 8, 0x0200);
        }
        m
    }
    fn w16(&mut self, a: usize, v: u16) -> &mut Self {
        self.0[a..a + 2].copy_from_slice(&v.to_le_bytes());
        self
    }
    fn bg_color(&mut self, i: usize, c: u16) -> &mut Self {
        self.w16(PAL_BG + i * 2, c)
    }
    fn obj_color(&mut self, i: usize, c: u16) -> &mut Self {
        self.w16(PAL_OBJ + i * 2, c)
    }
    /// A 4bpp tile filled with color index `i`.
    fn solid4(&mut self, at: usize, i: u8) -> &mut Self {
        self.0[at..at + 32].fill(i | i << 4);
        self
    }
    fn entry(&mut self, sbb: usize, tx: usize, ty: usize, e: u16) -> &mut Self {
        self.w16(VRAM + sbb * 0x800 + (ty * 32 + tx) * 2, e)
    }
    fn obj(&mut self, i: usize, a0: u16, a1: u16, a2: u16) -> &mut Self {
        self.w16(OAM + i * 8, a0).w16(OAM + i * 8 + 2, a1).w16(OAM + i * 8 + 4, a2)
    }
    fn px(&self, x: usize, y: usize) -> u32 {
        let mut line = [0; SCREEN_W];
        render_line(&self.0, y, &mut line);
        line[x]
    }
}

/// BG0: tile 1 solid in bank 0 color 1 = RED, map in screen block 31.
fn red_bg0() -> Mem {
    let mut m = Mem::new();
    m.w16(DISPCNT, 0x0100).w16(bgcnt(0), 31 << 8).bg_color(1, RED).bg_color(0, BLUE);
    m.solid4(VRAM + 32, 1).entry(31, 0, 0, 1);
    m
}

#[test]
fn backdrop_and_forced_blank() {
    let mut m = Mem::new();
    m.bg_color(0, BLUE);
    assert_eq!(m.px(0, 0), rgba(BLUE));
    m.w16(DISPCNT, 0x80);
    assert_eq!(m.px(0, 0), 0xFFFF_FFFF);
    m.w16(DISPCNT, 1); // reserved mode
    assert_eq!(m.px(0, 0), 0xFFFF_FFFF);
}

#[test]
fn text_background_banks_and_transparency() {
    let mut m = red_bg0();
    assert_eq!(m.px(0, 0), rgba(RED));
    assert_eq!(m.px(8, 0), rgba(BLUE), "tile 0 is empty: backdrop shows");
    m.bg_color(3 * 16 + 1, GREEN).entry(31, 0, 0, 1 | 3 << 12);
    assert_eq!(m.px(0, 0), rgba(GREEN));
}

#[test]
fn screen_entry_flips() {
    let mut m = red_bg0();
    m.0[VRAM + 32..VRAM + 64].fill(0);
    m.0[VRAM + 32] = 0x01; // only pixel (0, 0)
    assert_eq!((m.px(0, 0), m.px(7, 0)), (rgba(RED), rgba(BLUE)));
    m.entry(31, 0, 0, 1 | 0x400);
    assert_eq!((m.px(0, 0), m.px(7, 0)), (rgba(BLUE), rgba(RED)));
    m.entry(31, 0, 0, 1 | 0xC00);
    assert_eq!(m.px(7, 7), rgba(RED));
}

#[test]
fn wide_maps_use_the_next_screen_block_and_scroll_wraps() {
    let mut m = red_bg0();
    m.entry(31, 0, 0, 0);
    m.w16(bgcnt(0), 30 << 8 | 1 << 14); // 64×32 from block 30
    m.entry(31, 0, 0, 1); // map column 32 lives in block 31
    m.w16(bghofs(0), 256);
    assert_eq!(m.px(0, 0), rgba(RED));
    m.w16(bghofs(0), 256 + 512); // only 9 bits count
    assert_eq!(m.px(0, 0), rgba(RED));
}

#[test]
fn eight_bpp_tiles_use_the_full_palette() {
    let mut m = Mem::new();
    m.w16(DISPCNT, 0x0100).w16(bgcnt(0), 31 << 8 | 0x80).bg_color(200, GREEN);
    m.0[VRAM + 64..VRAM + 128].fill(200);
    m.entry(31, 0, 0, 1);
    assert_eq!(m.px(3, 3), rgba(GREEN));
}

#[test]
fn background_priority_order() {
    let mut m = red_bg0();
    m.w16(DISPCNT, 0x0300).w16(bgcnt(1), 30 << 8).bg_color(2, GREEN).solid4(VRAM + 64, 2).entry(30, 0, 0, 2);
    assert_eq!(m.px(0, 0), rgba(RED), "equal priority: lower BG number wins");
    m.w16(bgcnt(0), 31 << 8 | 1);
    assert_eq!(m.px(0, 0), rgba(GREEN));
}

#[test]
fn sprite_tile_mapping_1d_and_2d() {
    let mut m = Mem::new();
    m.obj_color(1, RED).obj_color(2, GREEN);
    m.solid4(OBJ_VRAM + 32, 1).solid4(OBJ_VRAM + 32 * 32, 2);
    m.obj(0, 0, 1 << 14, 0); // 16×16 at (0, 0), tile 0
    m.w16(DISPCNT, 0x1040);
    assert_eq!(m.px(8, 0), rgba(RED), "1D: second tile follows the first");
    assert_eq!(m.px(0, 8), 0xFF00_0000, "1D: row 2 starts at tile 2 (empty)");
    m.w16(DISPCNT, 0x1000);
    assert_eq!(m.px(0, 8), rgba(GREEN), "2D: row 2 starts at tile 32");
}

#[test]
fn sprite_priority_and_overlap() {
    let mut m = red_bg0();
    m.w16(DISPCNT, 0x1140).obj_color(1, GREEN).obj_color(2, WHITE);
    m.solid4(OBJ_VRAM, 1).solid4(OBJ_VRAM + 32, 2);
    m.obj(5, 0, 0, 0); // tile 0, priority 0
    assert_eq!(m.px(0, 0), rgba(GREEN), "sprites win ties with backgrounds");
    m.obj(5, 0, 0, 1 << 10);
    assert_eq!(m.px(0, 0), rgba(RED));
    m.obj(5, 0, 0, 0).obj(9, 0, 0, 1);
    assert_eq!(m.px(0, 0), rgba(GREEN), "equal priority: lower OAM index wins");
    m.obj(5, 0, 0, 1 << 10);
    assert_eq!(m.px(0, 0), rgba(WHITE), "lower priority number beats lower index");
}

#[test]
fn sprite_coordinates_wrap() {
    let mut m = Mem::new();
    m.w16(DISPCNT, 0x1040).obj_color(1, RED).solid4(OBJ_VRAM, 1);
    m.obj(0, 250, 236, 0); // y = -6
    assert_eq!(m.px(236, 1), rgba(RED));
    assert_eq!(m.px(236, 2), 0xFF00_0000);
    m.obj(0, 0, 510, 0); // x = -2
    assert_eq!(m.px(5, 0), rgba(RED));
    assert_eq!(m.px(6, 0), 0xFF00_0000);
    m.obj(0, 0x200, 0, 0); // disabled
    assert_eq!(m.px(0, 0), 0xFF00_0000);
}

#[test]
fn semi_transparent_sprites_and_alpha_blend() {
    let mut m = red_bg0();
    m.w16(DISPCNT, 0x1140).obj_color(1, BLUE).solid4(OBJ_VRAM, 1);
    m.obj(0, 1 << 10, 0, 0);
    m.w16(BLDCNT, 1 << 8).w16(BLDALPHA, 8 | 8 << 8); // second target BG0, effect none
    assert_eq!(m.px(0, 0), rgba(15 | 15 << 10));
    m.obj(0, 0, 0, 0); // normal sprite: no blend without first target
    assert_eq!(m.px(0, 0), rgba(BLUE));
    m.w16(BLDCNT, OBJ_BIT as u16 | 1 << 6 | 1 << 8);
    assert_eq!(m.px(0, 0), rgba(15 | 15 << 10));
}

#[test]
fn brightness_effects() {
    let mut m = red_bg0();
    m.w16(BLDCNT, 1 | 3 << 6).w16(BLDY, 8);
    assert_eq!(m.px(0, 0), rgba(16), "31 - (31×8 >> 4)");
    m.w16(BLDCNT, 1 | 2 << 6);
    assert_eq!(m.px(0, 0), rgba(31 | 15 << 5 | 15 << 10), "0 + (31×8 >> 4) on G and B");
    m.w16(BLDY, 31); // clamps to 16
    assert_eq!(m.px(0, 0), rgba(WHITE));
}

#[test]
fn windows_select_layers_and_effects() {
    let mut m = red_bg0();
    m.w16(DISPCNT, 0x2100).w16(WIN0H, 10 << 8 | 20).w16(WIN0V, 160);
    m.w16(WININ, 0).w16(WINOUT, 1);
    m.entry(31, 1, 0, 1).entry(31, 2, 0, 1);
    assert_eq!(m.px(9, 0), rgba(RED));
    assert_eq!(m.px(10, 0), rgba(BLUE), "inside WIN0 BG0 is off");
    assert_eq!(m.px(20, 0), rgba(RED), "right edge is exclusive");

    // Effects only where the window enables them.
    m.w16(BLDCNT, 1 | 3 << 6).w16(BLDY, 16).w16(WININ, 1 | EFFECTS_BIT as u16);
    assert_eq!(m.px(12, 0), rgba(0));
    assert_eq!(m.px(5, 0), rgba(RED));
}

#[test]
fn sprite_window() {
    let mut m = red_bg0();
    m.solid4(OBJ_VRAM, 1).obj_color(1, GREEN);
    m.obj(0, 2 << 10, 0, 0); // window-mode sprite over (0..8, 0..8)
    m.w16(DISPCNT, 0x9140).w16(WINOUT, 1 << 8);
    assert_eq!(m.px(0, 0), rgba(RED), "BG0 enabled inside the sprite window");
    m.entry(31, 1, 0, 1);
    assert_eq!(m.px(8, 0), rgba(BLUE), "outside it everything is off");
}

#[test]
fn background_mosaic() {
    let mut m = red_bg0();
    m.0[VRAM + 32..VRAM + 64].fill(0);
    m.0[VRAM + 32] = 0x01; // only pixel (0, 0)
    m.w16(bgcnt(0), 31 << 8 | 0x40).w16(MOSAIC, 3 | 3 << 4);
    assert_eq!(m.px(3, 3), rgba(RED));
    assert_eq!(m.px(4, 0), rgba(BLUE));
}

#[test]
fn bitmap_modes() {
    let mut m = Mem::new();
    m.bg_color(0, BLUE);
    m.w16(DISPCNT, 3 | 0x400).w16(VRAM + (2 * 240 + 1) * 2, GREEN);
    assert_eq!(m.px(1, 2), rgba(GREEN));

    m.bg_color(7, RED);
    m.0[VRAM + 0xA000 + 5] = 7;
    m.w16(DISPCNT, 4 | 0x400);
    assert_eq!(m.px(5, 0), rgba(BLUE), "page 0, index 0 is transparent");
    m.w16(DISPCNT, 4 | 0x410);
    assert_eq!(m.px(5, 0), rgba(RED));

    m.w16(DISPCNT, 5 | 0x400);
    assert_eq!(m.px(200, 0), rgba(BLUE), "outside the 160×128 bitmap");
}

#[test]
fn bitmap_modes_hide_low_sprite_tiles() {
    let mut m = Mem::new();
    m.w16(DISPCNT, 3 | 0x1040).obj_color(1, RED);
    m.solid4(OBJ_VRAM, 1).solid4(OBJ_VRAM + 512 * 32, 1);
    m.obj(0, 0, 0, 0);
    assert_eq!(m.px(0, 0), rgba(0));
    m.obj(0, 0, 0, 512);
    assert_eq!(m.px(0, 0), rgba(RED));
}

#[test]
fn affine_sprites() {
    let mut m = Mem::new();
    m.w16(DISPCNT, 0x1040).obj_color(1, RED);
    for t in 0..4 {
        m.solid4(OBJ_VRAM + t * 32, 1);
    }
    let matrix = |m: &mut Mem, pa: i16, pd: i16| {
        m.w16(OAM + 6, pa as u16).w16(OAM + 14, 0).w16(OAM + 22, 0).w16(OAM + 30, pd as u16);
    };
    // 16×16 affine sprite, matrix 0.
    m.obj(0, 0x100, 1 << 14, 0);
    matrix(&mut m, 256, 256);
    assert_eq!((m.px(0, 0), m.px(15, 15)), (rgba(RED), rgba(RED)));

    // Texture-space step of 2 shrinks it to the middle 8×8.
    matrix(&mut m, 512, 512);
    assert_eq!(m.px(3, 3), rgba(0));
    assert_eq!(m.px(4, 4), rgba(RED));

    // Double-size with a step of ½ doubles it to fill the 32×32 box.
    m.obj(0, 0x300, 1 << 14, 0);
    matrix(&mut m, 128, 128);
    assert_eq!((m.px(0, 0), m.px(31, 31)), (rgba(RED), rgba(RED)));
}
