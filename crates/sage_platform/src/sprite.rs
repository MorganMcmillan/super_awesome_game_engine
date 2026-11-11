//! The renderer's CPU half: camera layout, sprite batching and a reference
//! rasterizer. No window or GPU is needed, so all of it is unit-tested.
//!
//! The camera is applied here, on the CPU: batches hold clip-space vertices,
//! so the sprite shaders need no uniforms (see `tools/shaderc`).

use sage_assets::{BitmapFont, Image};
use sage_core::host::{DrawKind, Frame, UiDraw, UiFrame};

/// A texture slot in the renderer. 0 is a 1x1 white texture (solid
/// rectangles) and 1 is the font atlas; user textures follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub u32);

pub const WHITE: TextureId = TextureId(0);
pub const FONT: TextureId = TextureId(1);

/// One vertex as the sprite shader reads it: clip-space position, texture
/// coordinates, straight-alpha color. 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// Consecutive quads that share a texture, drawn with one call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub texture: TextureId,
    pub first_index: u32,
    pub index_count: u32,
}

/// Sprites in draw order, as vertices and indices for one upload.
#[derive(Clone, Debug)]
pub struct SpriteBatch {
    viewport: [f32; 2],
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub runs: Vec<Run>,
}

impl SpriteBatch {
    /// `viewport` is the render target size in pixels.
    pub fn new(viewport: [f32; 2]) -> SpriteBatch {
        SpriteBatch {
            viewport,
            vertices: vec![],
            indices: vec![],
            runs: vec![],
        }
    }

    pub fn viewport(&self) -> [f32; 2] {
        self.viewport
    }

    pub fn sprite_count(&self) -> usize {
        self.indices.len() / 6
    }

    /// Adds a quad given in screen pixels (origin top-left, y down).
    pub fn quad(
        &mut self,
        texture: TextureId,
        top_left: [f32; 2],
        size: [f32; 2],
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        color: [f32; 4],
    ) {
        let clip = |x: f32, y: f32| {
            [
                x / self.viewport[0] * 2.0 - 1.0,
                1.0 - y / self.viewport[1] * 2.0,
            ]
        };
        let (x0, y0) = (top_left[0], top_left[1]);
        let (x1, y1) = (x0 + size[0], y0 + size[1]);
        let base = self.vertices.len() as u32;
        for (x, y, u, v) in [
            (x0, y0, uv_min[0], uv_min[1]),
            (x1, y0, uv_max[0], uv_min[1]),
            (x1, y1, uv_max[0], uv_max[1]),
            (x0, y1, uv_min[0], uv_max[1]),
        ] {
            self.vertices.push(Vertex {
                pos: clip(x, y),
                uv: [u, v],
                color,
            });
        }
        let first = self.indices.len() as u32;
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        match self.runs.last_mut() {
            Some(run) if run.texture == texture => run.index_count += 6,
            _ => self.runs.push(Run {
                texture,
                first_index: first,
                index_count: 6,
            }),
        }
    }

    pub fn solid(&mut self, top_left: [f32; 2], size: [f32; 2], color: [f32; 4]) {
        self.quad(WHITE, top_left, size, [0.0; 2], [1.0; 2], color);
    }

    /// Draws `text` with the font atlas, top-left at `origin` (pixels).
    pub fn text(
        &mut self,
        font: &BitmapFont,
        text: &str,
        origin: [f32; 2],
        scale: f32,
        color: [f32; 4],
    ) {
        for g in font.layout(text, origin, scale) {
            self.quad(FONT, g.pos, g.size, g.uv_min, g.uv_max, color);
        }
    }
}

/// How the world is fitted into the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleMode {
    /// Largest scale that fits, any fraction; edges are not snapped.
    Fit,
    /// Integer scale (at least 1), origin and edges snapped to whole pixels,
    /// so every world unit is the same number of screen pixels.
    PixelPerfect,
}

/// An orthographic 2D camera over a world with its origin bottom-left and y
/// up, drawn into the part of the window below `reserved_top` pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub scale: f32,
    /// Screen position of the world's top-left corner, in pixels.
    pub origin: [f32; 2],
    pub world_height: f32,
    pub mode: ScaleMode,
}

impl View {
    /// `margin` is extra world units kept visible on every side of the world
    /// (Pong draws its goal zones there).
    pub fn new(
        world_size: [f32; 2],
        viewport: [f32; 2],
        reserved_top: f32,
        margin: f32,
        mode: ScaleMode,
    ) -> View {
        let avail = [viewport[0], (viewport[1] - reserved_top).max(1.0)];
        let shown = [world_size[0] + 2.0 * margin, world_size[1] + 2.0 * margin];
        let fit = (avail[0] / shown[0]).min(avail[1] / shown[1]);
        let scale = match mode {
            ScaleMode::Fit => fit,
            ScaleMode::PixelPerfect if fit >= 1.0 => fit.floor(),
            ScaleMode::PixelPerfect => fit,
        };
        let mut origin = [
            (avail[0] - shown[0] * scale) / 2.0 + margin * scale,
            reserved_top + (avail[1] - shown[1] * scale) / 2.0 + margin * scale,
        ];
        if mode == ScaleMode::PixelPerfect {
            origin = origin.map(f32::floor);
        }
        View {
            scale,
            origin,
            world_height: world_size[1],
            mode,
        }
    }

    /// Screen rectangle (top-left, size) of a world rectangle given by its
    /// center and half extents. Pixel-perfect mode rounds the edges, so
    /// neighbors never leave gaps or overlap.
    pub fn rect(&self, center: [f32; 2], half: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let left = self.origin[0] + (center[0] - half[0]) * self.scale;
        let right = self.origin[0] + (center[0] + half[0]) * self.scale;
        let top = self.origin[1] + (self.world_height - (center[1] + half[1])) * self.scale;
        let bottom = self.origin[1] + (self.world_height - (center[1] - half[1])) * self.scale;
        let snap = |v: f32| {
            if self.mode == ScaleMode::PixelPerfect {
                v.round()
            } else {
                v
            }
        };
        let (l, r, t, b) = (snap(left), snap(right), snap(top), snap(bottom));
        ([l, t], [r - l, b - t])
    }
}

/// Fill color of a `DrawKind`, straight alpha.
pub fn kind_color(kind: DrawKind) -> [f32; 4] {
    let rgb = crate::kind_rgb(kind);
    [
        f32::from(rgb[0]) / 255.0,
        f32::from(rgb[1]) / 255.0,
        f32::from(rgb[2]) / 255.0,
        1.0,
    ]
}

/// Which `DrawKind`s are drawn with a texture instead of a solid color.
#[derive(Clone, Debug, Default)]
pub struct Skin {
    pub sprites: Vec<(DrawKind, TextureId)>,
}

impl Skin {
    fn texture(&self, kind: DrawKind) -> Option<TextureId> {
        self.sprites.iter().find(|s| s.0 == kind).map(|s| s.1)
    }
}

/// Integer text scale for a viewport: crisp, and grows with the window.
pub fn text_scale(viewport_h: f32) -> f32 {
    (viewport_h / 240.0).floor().max(1.0)
}

/// World units of margin around the court that stay visible.
pub const MARGIN: f32 = 4.0;

/// Builds the batch for one frame: the court, the drawables in `StableId`
/// order, then the HUD line centered at the top.
pub fn compose(
    frame: &Frame,
    viewport: [f32; 2],
    mode: ScaleMode,
    font: &BitmapFont,
    skin: &Skin,
) -> SpriteBatch {
    let ts = text_scale(viewport[1]);
    let hud_height = font.glyph[1] as f32 * ts + 8.0 * ts;
    let view = View::new(frame.world_size, viewport, hud_height, MARGIN, mode);
    let mut batch = SpriteBatch::new(viewport);
    let mut rects: Vec<_> = frame.rects.iter().collect();
    rects.sort_by_key(|r| (r.kind != DrawKind::Court, r.id));
    for r in rects {
        let (pos, size) = view.rect(r.center, r.half);
        match skin.texture(r.kind) {
            Some(tex) => batch.quad(tex, pos, size, [0.0; 2], [1.0; 2], [1.0; 4]),
            None => batch.solid(pos, size, kind_color(r.kind)),
        }
    }
    let line = frame.hud.join("    ");
    let width = font.measure(&line, ts)[0];
    let x = ((viewport[0] - width) / 2.0).floor().max(2.0 * ts);
    batch.text(font, &line, [x, 4.0 * ts], ts, [0.8, 0.8, 0.8, 1.0]);
    batch
}

/// How a UI of `ui` units is placed in a viewport of pixels: the largest
/// whole-number scale that fits (crisp, like the pixel-perfect camera), and
/// the pixel position of the UI's top-left corner when it is centered. The
/// window loop uses the same numbers to turn the mouse position back into
/// UI units.
pub fn ui_view(viewport: [f32; 2], ui: [f32; 2]) -> (f32, [f32; 2]) {
    let scale = (viewport[0] / ui[0])
        .min(viewport[1] / ui[1])
        .floor()
        .max(1.0);
    let origin = [
        ((viewport[0] - ui[0] * scale) / 2.0).floor(),
        ((viewport[1] - ui[1] * scale) / 2.0).floor(),
    ];
    (scale, origin)
}

/// Builds the batch for a UI frame at the scale and position of
/// [`ui_view`]. Draw commands keep their order, so later ones cover earlier
/// ones.
pub fn compose_ui(frame: &UiFrame, viewport: [f32; 2], font: &BitmapFont) -> SpriteBatch {
    let (scale, origin) = ui_view(viewport, frame.size);
    let at = |p: [f32; 2]| [origin[0] + p[0] * scale, origin[1] + p[1] * scale];
    let rgba = |c: [u8; 4]| c.map(|v| f32::from(v) / 255.0);
    let mut batch = SpriteBatch::new(viewport);
    for d in &frame.draws {
        match d {
            UiDraw::Rect { pos, size, color } => {
                batch.solid(at(*pos), [size[0] * scale, size[1] * scale], rgba(*color))
            }
            UiDraw::Text {
                pos,
                text,
                scale: s,
                color,
                clip,
            } => {
                let clip = clip.map(|[l, t, r, b]| {
                    let (a, z) = (at([l, t]), at([r, b]));
                    [a[0], a[1], z[0], z[1]]
                });
                for g in font.layout(text, at(*pos), scale * s) {
                    let (mut p0, mut p1) = (g.pos, [g.pos[0] + g.size[0], g.pos[1] + g.size[1]]);
                    let (mut uv0, mut uv1) = (g.uv_min, g.uv_max);
                    if let Some(c) = clip {
                        let (x0, y0) = (p0[0].max(c[0]), p0[1].max(c[1]));
                        let (x1, y1) = (p1[0].min(c[2]), p1[1].min(c[3]));
                        if x0 >= x1 || y0 >= y1 {
                            continue;
                        }
                        // Cut the texture coordinates by the same fractions.
                        let (fx0, fx1) = ((x0 - p0[0]) / g.size[0], (x1 - p0[0]) / g.size[0]);
                        let (fy0, fy1) = ((y0 - p0[1]) / g.size[1], (y1 - p0[1]) / g.size[1]);
                        let (du, dv) = (g.uv_max[0] - g.uv_min[0], g.uv_max[1] - g.uv_min[1]);
                        uv0 = [g.uv_min[0] + du * fx0, g.uv_min[1] + dv * fy0];
                        uv1 = [g.uv_min[0] + du * fx1, g.uv_min[1] + dv * fy1];
                        (p0, p1) = ([x0, y0], [x1, y1]);
                    }
                    batch.quad(
                        FONT,
                        p0,
                        [p1[0] - p0[0], p1[1] - p0[1]],
                        uv0,
                        uv1,
                        rgba(*color),
                    );
                }
            }
        }
    }
    batch
}

/// Reference rasterizer: draws a batch on the CPU with nearest sampling and
/// straight-alpha blending over `clear`, the way the GPU path should.
/// Returns `w * h` RGBA pixels, top row first. Quads must be axis-aligned,
/// which is all `SpriteBatch` can produce.
pub fn rasterize(
    batch: &SpriteBatch,
    textures: &[Image],
    clear: [u8; 4],
    size: [u32; 2],
) -> Vec<[u8; 4]> {
    let (w, h) = (size[0] as usize, size[1] as usize);
    let mut px: Vec<[f32; 4]> = vec![clear.map(|c| f32::from(c) / 255.0); w * h];
    for run in &batch.runs {
        let tex = &textures[run.texture.0 as usize];
        for quad in batch.indices
            [run.first_index as usize..(run.first_index + run.index_count) as usize]
            .chunks(6)
        {
            let v0 = batch.vertices[quad[0] as usize];
            let v2 = batch.vertices[quad[2] as usize];
            // Clip space back to pixels.
            let to_px = |p: [f32; 2]| {
                [
                    (p[0] + 1.0) / 2.0 * batch.viewport[0],
                    (1.0 - p[1]) / 2.0 * batch.viewport[1],
                ]
            };
            let (a, b) = (to_px(v0.pos), to_px(v2.pos));
            let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
            let (y0, y1) = (a[1].min(b[1]), a[1].max(b[1]));
            for y in 0..h {
                for x in 0..w {
                    let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                    if cx < x0 || cx >= x1 || cy < y0 || cy >= y1 {
                        continue;
                    }
                    let (fx, fy) = ((cx - a[0]) / (b[0] - a[0]), (cy - a[1]) / (b[1] - a[1]));
                    let u = v0.uv[0] + (v2.uv[0] - v0.uv[0]) * fx;
                    let v = v0.uv[1] + (v2.uv[1] - v0.uv[1]) * fy;
                    let tx = ((u * tex.width as f32).floor() as i64).clamp(0, tex.width as i64 - 1);
                    let ty =
                        ((v * tex.height as f32).floor() as i64).clamp(0, tex.height as i64 - 1);
                    let t = tex.pixel(tx as u32, ty as u32);
                    let src: Vec<f32> = (0..4)
                        .map(|i| f32::from(t[i]) / 255.0 * v0.color[i])
                        .collect();
                    let dst = &mut px[y * w + x];
                    for i in 0..3 {
                        dst[i] = src[i] * src[3] + dst[i] * (1.0 - src[3]);
                    }
                    dst[3] = src[3] + dst[3] * (1.0 - src[3]);
                }
            }
        }
    }
    px.iter()
        .map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_core::StableId;
    use sage_core::host::DrawRect;

    fn frame() -> Frame {
        let rect = |id, kind, center: [f32; 2], half: [f32; 2]| DrawRect {
            id: StableId(id),
            kind,
            center,
            half,
        };
        Frame {
            tick: 0,
            world_size: [160.0, 90.0],
            rects: vec![
                rect(40, DrawKind::Ball, [80.0, 45.0], [2.0, 2.0]),
                rect(30, DrawKind::Paddle, [5.0, 45.0], [1.5, 10.0]),
                rect(1, DrawKind::Court, [80.0, 45.0], [80.0, 45.0]),
            ],
            hud: vec!["1  :  2".into(), "Playing".into()],
        }
    }

    fn textures() -> Vec<Image> {
        vec![
            Image::solid(1, 1, [255; 4]),
            BitmapFont::builtin().atlas,
            Image::solid(2, 2, [255, 0, 255, 255]),
        ]
    }

    #[test]
    fn pixel_perfect_scale_is_the_floor_of_the_limiting_axis() {
        let v = View::new(
            [160.0, 90.0],
            [960.0, 600.0],
            0.0,
            0.0,
            ScaleMode::PixelPerfect,
        );
        assert_eq!(v.scale, 6.0);
        assert_eq!(v.origin, [0.0, 30.0]);
        let v = View::new(
            [160.0, 90.0],
            [1000.0, 560.0],
            0.0,
            0.0,
            ScaleMode::PixelPerfect,
        );
        assert_eq!(v.scale, 6.0); // 560/90 = 6.2 limits, not 1000/160 = 6.25
        assert_eq!(v.origin, [20.0, 10.0]);
        let (pos, size) = v.rect([80.0, 45.0], [80.0, 45.0]);
        assert_eq!((pos, size), ([20.0, 10.0], [960.0, 540.0]));
        // A small window falls back to fractional scale rather than overflowing.
        let v = View::new(
            [160.0, 90.0],
            [80.0, 45.0],
            0.0,
            0.0,
            ScaleMode::PixelPerfect,
        );
        assert_eq!(v.scale, 0.5);
    }

    #[test]
    fn pixel_perfect_neighbors_share_edges() {
        let v = View::new(
            [160.0, 90.0],
            [1000.0, 560.0],
            0.0,
            0.0,
            ScaleMode::PixelPerfect,
        );
        // Two rects touching at x = 10.3 in world units.
        let (a, sa) = v.rect([5.15, 45.0], [5.15, 5.0]);
        let (b, _) = v.rect([15.45, 45.0], [5.15, 5.0]);
        assert_eq!(a[0] + sa[0], b[0]);
        assert_eq!(a[0].fract(), 0.0);
    }

    #[test]
    fn the_margin_keeps_the_goal_zones_on_screen() {
        // A goal zone just outside the court, at x = -2..0, must land inside the viewport.
        let v = View::new(
            [160.0, 90.0],
            [960.0, 600.0],
            30.0,
            MARGIN,
            ScaleMode::PixelPerfect,
        );
        assert_eq!(v.scale, 5.0);
        let (pos, size) = v.rect([-2.0, 45.0], [2.0, 45.0]);
        assert!(
            pos[0] >= 0.0 && pos[0] + size[0] <= 960.0,
            "{pos:?} {size:?}"
        );
        let (pos, size) = v.rect([162.0, 45.0], [2.0, 45.0]);
        assert!(
            pos[0] >= 0.0 && pos[0] + size[0] <= 960.0,
            "{pos:?} {size:?}"
        );
        // Without a margin the same zone is off screen.
        let bare = View::new(
            [160.0, 90.0],
            [960.0, 600.0],
            30.0,
            0.0,
            ScaleMode::PixelPerfect,
        );
        assert!(bare.rect([-2.0, 45.0], [2.0, 45.0]).0[0] < 0.0);
    }

    #[test]
    fn fit_mode_keeps_fractional_scale() {
        let v = View::new([160.0, 90.0], [1000.0, 560.0], 0.0, 0.0, ScaleMode::Fit);
        assert!((v.scale - 560.0 / 90.0).abs() < 1e-6);
    }

    #[test]
    fn world_y_is_up_on_screen() {
        let v = View::new(
            [100.0, 100.0],
            [100.0, 100.0],
            0.0,
            0.0,
            ScaleMode::PixelPerfect,
        );
        let (low, _) = v.rect([50.0, 5.0], [1.0, 1.0]);
        let (high, _) = v.rect([50.0, 95.0], [1.0, 1.0]);
        assert!(high[1] < low[1]);
    }

    #[test]
    fn batch_merges_same_texture_runs_and_keeps_draw_order() {
        let mut b = SpriteBatch::new([100.0, 100.0]);
        b.solid([0.0, 0.0], [10.0, 10.0], [1.0; 4]);
        b.solid([20.0, 0.0], [10.0, 10.0], [1.0; 4]);
        b.quad(
            TextureId(2),
            [0.0, 0.0],
            [4.0, 4.0],
            [0.0; 2],
            [1.0; 2],
            [1.0; 4],
        );
        b.solid([0.0, 0.0], [1.0, 1.0], [1.0; 4]);
        assert_eq!(b.sprite_count(), 4);
        assert_eq!(b.vertices.len(), 16);
        let runs: Vec<_> = b
            .runs
            .iter()
            .map(|r| (r.texture.0, r.index_count))
            .collect();
        assert_eq!(runs, [(0, 12), (2, 6), (0, 6)]);
        // Top-left pixel (0,0) is clip (-1, 1); bottom-right (100,100) is (1,-1).
        assert_eq!(b.vertices[0].pos, [-1.0, 1.0]);
        let mut full = SpriteBatch::new([100.0, 100.0]);
        full.solid([0.0, 0.0], [100.0, 100.0], [1.0; 4]);
        assert_eq!(full.vertices[2].pos, [1.0, -1.0]);
    }

    #[test]
    fn compose_draws_court_first_then_ids_then_text() {
        let font = BitmapFont::builtin();
        let b = compose(
            &frame(),
            [960.0, 600.0],
            ScaleMode::PixelPerfect,
            &font,
            &Skin::default(),
        );
        // 3 rects + the HUD glyphs ("1  :  2    Playing" has 10 non-space characters).
        assert_eq!(b.sprite_count(), 3 + 10);
        assert_eq!(b.runs[0].texture, WHITE);
        assert_eq!(b.runs[0].index_count, 18);
        assert_eq!(b.runs[1].texture, FONT);
    }

    #[test]
    fn skin_swaps_a_solid_for_a_texture() {
        let font = BitmapFont::builtin();
        let skin = Skin {
            sprites: vec![(DrawKind::Ball, TextureId(2))],
        };
        let b = compose(
            &frame(),
            [960.0, 600.0],
            ScaleMode::PixelPerfect,
            &font,
            &skin,
        );
        let order: Vec<_> = b.runs.iter().map(|r| r.texture.0).collect();
        assert_eq!(order, [0, 2, 1]);
    }

    #[test]
    fn rasterized_frame_has_each_kind_and_text() {
        let font = BitmapFont::builtin();
        let size = [960u32, 600];
        let b = compose(
            &frame(),
            [960.0, 600.0],
            ScaleMode::PixelPerfect,
            &font,
            &Skin::default(),
        );
        let px = rasterize(&b, &textures(), [0, 0, 0, 255], size);
        let count = |kind: DrawKind| {
            let c = crate::kind_rgb(kind);
            px.iter()
                .filter(|p| p[0] == c[0] && p[1] == c[1] && p[2] == c[2])
                .count()
        };
        // 960x600 with a 30 px HUD strip and a 4 unit margin: 168x98 units fit at
        // 5.71, so the integer scale is 5. Paddle 3 x 20 units, ball 4 x 4.
        assert_eq!(count(DrawKind::Paddle), 15 * 100);
        assert_eq!(count(DrawKind::Ball), 20 * 20);
        assert!(count(DrawKind::Court) > 100_000);
        // Text is light grey (204): present only in the strip above the court.
        let text = px.iter().filter(|p| p[0] == 204 && p[1] == 204).count();
        assert!(text > 100, "text pixels: {text}");
    }

    #[test]
    fn ui_metrics_match_the_builtin_font() {
        use sage_core::host::{UI_ADVANCE, UI_GLYPH, UI_LINE_HEIGHT};
        let f = BitmapFont::builtin();
        assert_eq!(UI_GLYPH, [f.glyph[0] as f32, f.glyph[1] as f32]);
        assert_eq!(UI_ADVANCE, f.advance as f32);
        assert_eq!(UI_LINE_HEIGHT, f.line_height as f32);
    }

    #[test]
    fn ui_frames_scale_by_whole_numbers_center_and_keep_order() {
        let frame = UiFrame {
            size: [40.0, 20.0],
            draws: vec![
                UiDraw::Rect {
                    pos: [0.0, 0.0],
                    size: [40.0, 20.0],
                    color: [10, 20, 30, 255],
                },
                UiDraw::Rect {
                    pos: [10.0, 5.0],
                    size: [10.0, 10.0],
                    color: [200, 0, 0, 255],
                },
                UiDraw::Text {
                    pos: [1.0, 1.0],
                    text: "A".into(),
                    scale: 1.0,
                    color: [255, 255, 255, 255],
                    clip: None,
                },
            ],
        };
        // 100x70 fits the 40x20 UI at scale 2 (the width limits it), centered.
        let b = compose_ui(&frame, [100.0, 70.0], &BitmapFont::builtin());
        let px = rasterize(&b, &textures(), [0, 0, 0, 255], [100, 70]);
        let at = |x: usize, y: usize| px[y * 100 + x];
        // Origin: ((100 - 80) / 2, (70 - 40) / 2) = (10, 15).
        assert_eq!(
            at(9, 14),
            [0, 0, 0, 255],
            "outside the UI is the clear color"
        );
        assert_eq!(at(10, 15), [10, 20, 30, 255], "the panel's corner");
        assert_eq!(
            at(30, 25),
            [200, 0, 0, 255],
            "the second rect covers the first"
        );
        assert_eq!(at(89, 54), [10, 20, 30, 255], "the panel's far corner");
        assert_eq!(at(90, 55), [0, 0, 0, 255]);
        // The glyph is drawn on top, at twice the size: a lit pixel inside
        // the cell at (1, 1) * 2 + origin.
        let lit = (0..14).any(|dy| (0..10).any(|dx| at(12 + dx, 17 + dy) == [255, 255, 255, 255]));
        assert!(lit, "the glyph is visible");
    }

    #[test]
    fn clipped_ui_text_is_cut_at_the_clip_and_keeps_its_pixels() {
        // "I" at scale 1 is a 5x7 glyph; clip away its top three rows.
        let font = BitmapFont::builtin();
        let draw = |clip| UiFrame {
            size: [10.0, 10.0],
            draws: vec![UiDraw::Text {
                pos: [1.0, 1.0],
                text: "I".into(),
                scale: 1.0,
                color: [255; 4],
                clip,
            }],
        };
        let render = |frame: &UiFrame| {
            let b = compose_ui(frame, [10.0, 10.0], &font);
            rasterize(&b, &textures(), [0, 0, 0, 255], [10, 10])
        };
        let whole = render(&draw(None));
        let cut = render(&draw(Some([0.0, 4.0, 10.0, 10.0])));
        let lit = |px: &[[u8; 4]], rows: std::ops::Range<usize>| {
            rows.flat_map(|y| (0..10).map(move |x| (x, y)))
                .filter(|&(x, y)| px[y * 10 + x] != [0, 0, 0, 255])
                .count()
        };
        assert!(
            lit(&whole, 1..4) > 0,
            "the glyph has pixels in its top rows"
        );
        assert_eq!(lit(&cut, 0..4), 0, "nothing above the clip");
        assert_eq!(
            lit(&cut, 4..10),
            lit(&whole, 4..10),
            "below the clip it is unchanged"
        );
        // A clip that excludes the whole glyph draws nothing.
        let none = render(&draw(Some([8.0, 8.0, 10.0, 10.0])));
        assert_eq!(lit(&none, 0..10), 0);
    }

    #[test]
    fn ui_view_scales_by_whole_numbers_and_centers() {
        assert_eq!(ui_view([100.0, 70.0], [40.0, 20.0]), (2.0, [10.0, 15.0]));
        assert_eq!(ui_view([960.0, 600.0], [160.0, 100.0]), (6.0, [0.0, 0.0]));
        // Smaller than the UI: never below 1.
        assert_eq!(ui_view([10.0, 10.0], [40.0, 20.0]).0, 1.0);
        // The mapping the window loop inverts: pixel -> UI units.
        let (scale, origin) = ui_view([100.0, 70.0], [40.0, 20.0]);
        assert_eq!(
            ((30.0 - origin[0]) / scale, (25.0 - origin[1]) / scale),
            (10.0, 5.0)
        );
    }
}
