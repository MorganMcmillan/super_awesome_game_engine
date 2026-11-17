//! A bitmap font: a grid atlas image plus metrics. The built-in font is a
//! 5x7 pixel face generated in code, so the engine needs no font file; it is
//! upper case only (lower case maps to upper case). [`BitmapFont::from_ttf`]
//! rasterizes a TrueType or OpenType face into the same kind of atlas, with
//! lower case and Latin-1, so everything that draws text works with either.

use crate::Image;

/// One positioned glyph, ready to become a sprite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphQuad {
    /// Top-left corner in pixels, y down.
    pub pos: [f32; 2],
    pub size: [f32; 2],
    /// Normalized texture coordinates of the glyph in the atlas.
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct BitmapFont {
    pub atlas: Image,
    /// Atlas cell size in pixels.
    pub cell: [u32; 2],
    /// Visible glyph size inside a cell, in UI units at scale 1 (what text
    /// measures and a quad covers).
    pub glyph: [u32; 2],
    /// The same area in atlas pixels (equal to `glyph` for the built-in face;
    /// larger when the atlas is rasterized for a scaled window).
    pub glyph_px: [u32; 2],
    /// Horizontal pen advance and line height, in pixels at scale 1.
    pub advance: u32,
    pub line_height: u32,
    pub columns: u32,
    /// Character code of cell 0.
    pub first: u32,
    /// Lower case draws with the upper case glyphs (the built-in face has no
    /// lower case).
    pub fold_case: bool,
    /// A shaped, proportional face: glyphs are placed by their own advances
    /// (see [`BitmapFont::from_ttf_proportional`]); `None` is the fixed grid.
    pub shaped: Option<std::rc::Rc<crate::proportional::ShapedFace>>,
    /// The atlas holds four stacked copies of the glyphs: regular, bold,
    /// italic, and bold italic, in that order (`layout_styled`).
    pub styled: bool,
}

/// Stacks the regular atlas with bold, italic and bold italic copies made
/// from it (each cell treated alone): bold strikes each row again a little to
/// the right, italic slants rows about the baseline.
fn with_styles(regular: Image, cell: [u32; 2], baseline: i32, k: u32) -> Image {
    let (w, h) = (regular.width, regular.height);
    let mut out = Image::solid(w, h * 4, [255, 255, 255, 0]);
    let bold_by = (k as f32 * 0.7).ceil().max(1.0) as i32;
    for style in 0..4u32 {
        for y in 0..h {
            let row = (y % cell[1]) as i32;
            let slant = if style & 2 != 0 {
                ((baseline - row) as f32 * 0.2).round() as i32
            } else {
                0
            };
            for x in 0..w {
                let col = (x % cell[0]) as i32;
                let src = |dx: i32| {
                    let c = col - slant - dx;
                    if c < 0 || c >= cell[0] as i32 {
                        0
                    } else {
                        regular.pixel((x as i32 - slant - dx) as u32, y)[3]
                    }
                };
                let mut a = src(0);
                if style & 1 != 0 {
                    for d in 1..=bold_by {
                        a = a.max(src(d));
                    }
                }
                if a > 0 {
                    out.set_pixel(x, style * h + y, [255, 255, 255, a]);
                }
            }
        }
    }
    out
}

/// The characters of a proportional atlas: Latin through Latin Extended-B,
/// Greek and Coptic, and Cyrillic.
pub const PROPORTIONAL_RANGES: [std::ops::Range<u32>; 3] = [32..0x250, 0x370..0x400, 0x400..0x500];

impl BitmapFont {
    /// The atlas cell for `c`, or `?` when the font has no glyph for it.
    fn cell_of(&self, c: char) -> u32 {
        let c = if self.fold_case {
            c.to_ascii_uppercase()
        } else {
            c
        };
        let rows = self.atlas.height / self.cell[1];
        let code = c as u32;
        if code >= self.first && code - self.first < self.columns * rows {
            code - self.first
        } else {
            '?' as u32 - self.first
        }
    }

    /// Lays out `text` with its top-left at `origin`. `\n` starts a new line.
    /// Spaces advance the pen but produce no quad.
    pub fn layout(&self, text: &str, origin: [f32; 2], scale: f32) -> Vec<GlyphQuad> {
        if let Some(face) = &self.shaped {
            return self.layout_shaped(face, text, origin, scale);
        }
        let (aw, ah) = (self.atlas.width as f32, self.atlas.height as f32);
        let mut quads = vec![];
        let (mut x, mut y) = (origin[0], origin[1]);
        for c in text.chars() {
            if c == '\n' {
                x = origin[0];
                y += self.line_height as f32 * scale;
                continue;
            }
            if c != ' ' {
                let cell = self.cell_of(c);
                let (cx, cy) = (cell % self.columns, cell / self.columns);
                let (u0, v0) = (
                    (cx * self.cell[0]) as f32 / aw,
                    (cy * self.cell[1]) as f32 / ah,
                );
                quads.push(GlyphQuad {
                    pos: [x, y],
                    size: [self.glyph[0] as f32 * scale, self.glyph[1] as f32 * scale],
                    uv_min: [u0, v0],
                    uv_max: [
                        u0 + self.glyph_px[0] as f32 / aw,
                        v0 + self.glyph_px[1] as f32 / ah,
                    ],
                });
            }
            x += self.advance as f32 * scale;
        }
        quads
    }

    /// [`layout`](BitmapFont::layout) with a style (`STYLE_BOLD`,
    /// `STYLE_ITALIC` of `sage_core::host`): a face with styled glyphs picks
    /// them from its atlas; one without (the built-in face) lays out as
    /// regular, and the caller decides how to fake bold.
    pub fn layout_styled(
        &self,
        text: &str,
        origin: [f32; 2],
        scale: f32,
        style: u8,
    ) -> Vec<GlyphQuad> {
        let mut quads = self.layout(text, origin, scale);
        if self.styled && style & 3 != 0 {
            let shift = f32::from(style & 3) / 4.0;
            for q in &mut quads {
                q.uv_min[1] += shift;
                q.uv_max[1] += shift;
            }
        }
        quads
    }

    fn layout_shaped(
        &self,
        face: &crate::proportional::ShapedFace,
        text: &str,
        origin: [f32; 2],
        scale: f32,
    ) -> Vec<GlyphQuad> {
        let (aw, ah) = (self.atlas.width as f32, self.atlas.height as f32);
        let mut quads = vec![];
        let mut y = origin[1];
        for line in text.split('\n') {
            let mut x = origin[0];
            for (gid, adv) in face.shape_line(line) {
                if gid != face.space {
                    // A glyph the atlas does not hold draws as `?`.
                    let q = char::from_u32('?' as u32).and_then(|_| face.cells.values().next());
                    let cell = face
                        .cells
                        .get(&gid)
                        .copied()
                        .or_else(|| q.copied())
                        .unwrap_or(0);
                    let (cx, cy) = (cell % self.columns, cell / self.columns);
                    let (u0, v0) = (
                        (cx * self.cell[0]) as f32 / aw,
                        (cy * self.cell[1]) as f32 / ah,
                    );
                    quads.push(GlyphQuad {
                        pos: [x, y],
                        size: [self.glyph[0] as f32 * scale, self.glyph[1] as f32 * scale],
                        uv_min: [u0, v0],
                        uv_max: [
                            u0 + self.glyph_px[0] as f32 / aw,
                            v0 + self.glyph_px[1] as f32 / ah,
                        ],
                    });
                }
                x += adv * scale;
            }
            y += self.line_height as f32 * scale;
        }
        quads
    }

    /// Width and height of `text` in pixels.
    pub fn measure(&self, text: &str, scale: f32) -> [f32; 2] {
        if let Some(face) = &self.shaped {
            use sage_core::host::TextMetrics;
            let lines: Vec<&str> = text.split('\n').collect();
            let w = lines.iter().map(|l| face.line_width(l)).fold(0.0, f32::max);
            return [
                w * scale,
                ((lines.len() - 1) as u32 * self.line_height + 7) as f32 * scale,
            ];
        }
        let lines = text.split('\n');
        let count = lines.clone().count();
        let widest = lines.map(|l| l.chars().count()).max().unwrap_or(0);
        let w = if widest == 0 {
            0.0
        } else {
            ((widest as u32 - 1) * self.advance + self.glyph[0]) as f32 * scale
        };
        let h = ((count as u32 - 1) * self.line_height + self.glyph[1]) as f32 * scale;
        [w, h]
    }

    /// The built-in 5x7 font: a 128x48 atlas of 16x6 cells of 8x8 pixels
    /// covering ASCII 32 to 127. White glyphs on transparent black.
    pub fn builtin() -> BitmapFont {
        let mut atlas = Image::solid(128, 48, [255, 255, 255, 0]);
        for &(c, rows) in GLYPHS {
            let cell = c as u32 - 32;
            let (ox, oy) = ((cell % 16) * 8, (cell / 16) * 8);
            for (y, row) in rows.iter().enumerate() {
                for (x, ch) in row.bytes().enumerate() {
                    if ch == b'#' {
                        atlas.set_pixel(ox + x as u32, oy + y as u32, [255, 255, 255, 255]);
                    }
                }
            }
        }
        // Lower case shares the upper case cells, handled in `cell_of`.
        BitmapFont {
            atlas,
            cell: [8, 8],
            glyph: [5, 7],
            glyph_px: [5, 7],
            advance: 6,
            line_height: 9,
            columns: 16,
            first: 32,
            fold_case: true,
            shaped: None,
            styled: false,
        }
    }

    /// Rasterizes a monospaced TrueType or OpenType face (`bytes`) into an
    /// atlas of the characters 32 to 255. Text is laid out on the UI's grid
    /// (6 by 9 units a character, as the built-in face), and `px_per_unit` is
    /// how many device pixels a UI unit is (the window's scale), so glyphs are
    /// rasterized at the size they are shown and stay sharp. A face whose
    /// characters are wider than a cell is scaled down to fit it. Proportional
    /// faces are drawn on the same grid, which spaces them unevenly.
    pub fn from_ttf(bytes: &[u8], px_per_unit: u32) -> Result<BitmapFont, String> {
        const ADVANCE: u32 = 6;
        const LINE: u32 = 9;
        let k = px_per_unit.max(1);
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|e| format!("font: {e}"))?;
        let (cw, ch) = (ADVANCE * k, LINE * k);
        // Pick the size where an 'M' fits the cell's width and the whole
        // line (ascent to descent) fits its height.
        let unit = font.metrics('M', 100.0);
        let lines = font
            .horizontal_line_metrics(100.0)
            .ok_or("font: no line metrics")?;
        let by_width = cw as f32 * 100.0 / unit.advance_width.max(1.0);
        let by_height = ch as f32 * 100.0 / (lines.ascent - lines.descent).max(1.0);
        let px = by_width.min(by_height).max(4.0);
        let lines = font
            .horizontal_line_metrics(px)
            .ok_or("font: no line metrics")?;
        let baseline = ((ch as f32 + lines.ascent + lines.descent) / 2.0).round() as i32;
        let columns = 16u32;
        let first = 32u32;
        let count = 256 - first;
        let rows = count.div_ceil(columns);
        let mut atlas = Image::solid(columns * cw, rows * ch, [255, 255, 255, 0]);
        for code in first..256 {
            let c = char::from_u32(code).unwrap_or('?');
            if font.lookup_glyph_index(c) == 0 && c != ' ' {
                continue; // the face has no glyph: the cell stays empty
            }
            let (m, coverage) = font.rasterize(c, px);
            let cell = code - first;
            let (ox, oy) = ((cell % columns * cw) as i32, (cell / columns * ch) as i32);
            // Center the glyph's advance in the cell, sit it on the baseline.
            let left = ((cw as f32 - m.advance_width) / 2.0).round() as i32 + m.xmin;
            let top = baseline - m.ymin - m.height as i32;
            for y in 0..m.height {
                for x in 0..m.width {
                    let a = coverage[y * m.width + x];
                    let (px_x, px_y) = (left + x as i32, top + y as i32);
                    if a == 0 || px_x < 0 || px_y < 0 || px_x >= cw as i32 || px_y >= ch as i32 {
                        continue;
                    }
                    atlas.set_pixel((ox + px_x) as u32, (oy + px_y) as u32, [255, 255, 255, a]);
                }
            }
        }
        let atlas = with_styles(atlas, [cw, ch], baseline, k);
        Ok(BitmapFont {
            atlas,
            cell: [cw, ch],
            // A quad covers the whole cell: the glyph's size in UI units.
            glyph: [ADVANCE, LINE],
            glyph_px: [cw, ch],
            advance: ADVANCE,
            line_height: LINE,
            columns,
            first,
            fold_case: false,
            shaped: None,
            styled: true,
        })
    }

    /// Rasterizes any TrueType or OpenType face for proportional, shaped
    /// text: each glyph keeps its own advance and kerning (`rustybuzz`), and
    /// the same face gives layout its widths (`shaped`, a
    /// `sage_core::host::TextMetrics`). Atlas cells are wide enough for the
    /// widest glyph; `px_per_unit` is the window's scale, as in `from_ttf`.
    pub fn from_ttf_proportional(bytes: &[u8], px_per_unit: u32) -> Result<BitmapFont, String> {
        use crate::proportional::{LINE, ShapedFace};
        let k = px_per_unit.max(1);
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|e| format!("font: {e}"))?;
        let mut shaped = ShapedFace::new(bytes)?;
        let px = shaped.em * k as f32;
        let lines = font
            .horizontal_line_metrics(px)
            .ok_or("font: no line metrics")?;
        // The widest glyph decides the cell width.
        let cell_units = (shaped.em * 1.25).ceil() as u32;
        let (cw, ch) = (cell_units * k, LINE as u32 * k);
        let baseline = ((ch as f32 + lines.ascent + lines.descent) / 2.0).round() as i32;
        let columns = 32u32;
        let first = 32u32;
        // Latin through Latin Extended-B, Greek and Cyrillic: the characters
        // the atlas holds, in cells numbered in this order.
        let codes: Vec<u32> = PROPORTIONAL_RANGES.iter().flat_map(|r| r.clone()).collect();
        let rows = (codes.len() as u32).div_ceil(columns);
        let mut atlas = Image::solid(columns * cw, rows * ch, [255, 255, 255, 0]);
        for (cell, &code) in codes.iter().enumerate() {
            let cell = cell as u32;
            let Some(c) = char::from_u32(code) else {
                continue;
            };
            let gid = font.lookup_glyph_index(c);
            if gid == 0 && c != ' ' {
                continue;
            }
            shaped.cells.insert(gid, cell);
            if c == ' ' {
                shaped.space = gid;
            }
            if c == '?' {
                shaped.question = Some(cell);
            }
            let (m, coverage) = font.rasterize_indexed(gid, px);
            let (ox, oy) = ((cell % columns * cw) as i32, (cell / columns * ch) as i32);
            let top = baseline - m.ymin - m.height as i32;
            for y in 0..m.height {
                for x in 0..m.width {
                    let a = coverage[y * m.width + x];
                    let (gx, gy) = (m.xmin.max(0) + x as i32, top + y as i32);
                    if a == 0 || gx < 0 || gy < 0 || gx >= cw as i32 || gy >= ch as i32 {
                        continue;
                    }
                    atlas.set_pixel((ox + gx) as u32, (oy + gy) as u32, [255, 255, 255, a]);
                }
            }
        }
        let atlas = with_styles(atlas, [cw, ch], baseline, k);
        Ok(BitmapFont {
            atlas,
            cell: [cw, ch],
            glyph: [cell_units, LINE as u32],
            glyph_px: [cw, ch],
            advance: 6,
            line_height: LINE as u32,
            columns,
            first,
            fold_case: false,
            shaped: Some(std::rc::Rc::new(shaped)),
            styled: true,
        })
    }
}

const BLANK: &str = ".....";

#[rustfmt::skip]
const GLYPHS: &[(char, [&str; 7])] = &[
    ('0', [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('2', [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"]),
    ('3', ["#####", "...#.", "..#..", "...#.", "....#", "#...#", ".###."]),
    ('4', ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', ["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', [".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##.."]),
    ('A', [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('B', ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."]),
    ('C', [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."]),
    ('D', ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."]),
    ('E', ["#####", "#....", "#....", "####.", "#....", "#....", "#####"]),
    ('F', ["#####", "#....", "#....", "####.", "#....", "#....", "#...."]),
    ('G', [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".####"]),
    ('H', ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('I', [".###.", "..#..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('J', ["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."]),
    ('K', ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"]),
    ('L', ["#....", "#....", "#....", "#....", "#....", "#....", "#####"]),
    ('M', ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"]),
    ('N', ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"]),
    ('O', [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('P', ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('Q', [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"]),
    ('S', [".####", "#....", "#....", ".###.", "....#", "....#", "####."]),
    ('T', ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('U', ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#"]),
    ('X', ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"]),
    ('Y', ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."]),
    ('Z', ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
    (':', [BLANK, "..#..", "..#..", BLANK, "..#..", "..#..", BLANK]),
    ('.', [BLANK, BLANK, BLANK, BLANK, BLANK, "..#..", "..#.."]),
    (',', [BLANK, BLANK, BLANK, BLANK, "..#..", "..#..", ".#..."]),
    ('-', [BLANK, BLANK, BLANK, "#####", BLANK, BLANK, BLANK]),
    ('+', [BLANK, "..#..", "..#..", "#####", "..#..", "..#..", BLANK]),
    // The password mask: without it a masked field draws nothing.
    ('*', [BLANK, "#.#.#", ".###.", "#####", ".###.", "#.#.#", BLANK]),
    ('=', [BLANK, BLANK, "#####", BLANK, "#####", BLANK, BLANK]),
    ('!', ["..#..", "..#..", "..#..", "..#..", "..#..", BLANK, "..#.."]),
    ('?', [".###.", "#...#", "....#", "...#.", "..#..", BLANK, "..#.."]),
    ('/', ["....#", "....#", "...#.", "..#..", ".#...", "#....", "#...."]),
    ('(', ["...#.", "..#..", ".#...", ".#...", ".#...", "..#..", "...#."]),
    (')', [".#...", "..#..", "...#.", "...#.", "...#.", "..#..", ".#..."]),
    ('_', [BLANK, BLANK, BLANK, BLANK, BLANK, BLANK, "#####"]),
    ('\'', ["..#..", "..#..", BLANK, BLANK, BLANK, BLANK, BLANK]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_password_mask_has_a_glyph() {
        let f = BitmapFont::builtin();
        // '*' is cell 10 (column 10, row 0): it must have ink.
        let inked = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .filter(|&(x, y)| f.atlas.pixel(10 * 8 + x, y)[3] == 255)
            .count();
        assert!(inked >= 10, "{inked}");
    }

    #[test]
    fn every_glyph_row_is_five_wide_and_cells_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for &(c, rows) in GLYPHS {
            assert!(seen.insert(c), "duplicate glyph {c}");
            assert!((c as u32) >= 32 && (c as u32) < 128);
            for r in rows {
                assert_eq!(r.len(), 5, "glyph {c}");
            }
        }
    }

    #[test]
    fn layout_places_glyphs_on_the_advance_grid() {
        let f = BitmapFont::builtin();
        let q = f.layout("A b\nC", [10.0, 20.0], 2.0);
        // 'A', 'B' (lower case shares upper case), 'C'; the space makes no quad.
        assert_eq!(q.len(), 3);
        assert_eq!(q[0].pos, [10.0, 20.0]);
        assert_eq!(q[1].pos, [10.0 + 2.0 * 12.0, 20.0]);
        assert_eq!(q[2].pos, [10.0, 20.0 + 18.0]);
        assert_eq!(q[0].size, [10.0, 14.0]);
        // 'A' is cell 33: column 1, row 2 of a 128x48 atlas.
        assert_eq!(q[0].uv_min, [8.0 / 128.0, 16.0 / 48.0]);
    }

    #[test]
    fn measure_matches_layout_extent() {
        let f = BitmapFont::builtin();
        assert_eq!(f.measure("", 1.0), [0.0, 7.0]);
        assert_eq!(f.measure("AB", 1.0), [11.0, 7.0]);
        assert_eq!(f.measure("AB\nC", 2.0), [22.0, 32.0]);
    }

    #[test]
    fn unknown_characters_draw_a_question_mark() {
        let f = BitmapFont::builtin();
        let a = f.layout("\u{e9}", [0.0, 0.0], 1.0);
        let b = f.layout("?", [0.0, 0.0], 1.0);
        assert_eq!(a, b);
    }

    #[test]
    fn atlas_pixels_follow_the_glyph_bitmaps() {
        let f = BitmapFont::builtin();
        // 'I' is cell 41 (column 9, row 2); its top row is ".###." so x=0 is clear, x=1 is set.
        let (ox, oy) = (9 * 8, 2 * 8);
        assert_eq!(f.atlas.pixel(ox, oy)[3], 0);
        assert_eq!(f.atlas.pixel(ox + 1, oy)[3], 255);
        assert_eq!(f.atlas.pixel(ox + 2, oy + 6)[3], 255);
        // The eighth row of every cell is empty.
        assert_eq!(f.atlas.pixel(ox + 2, oy + 7)[3], 0);
    }
}

#[cfg(test)]
mod ttf_tests {
    use super::*;

    /// Any monospaced face the machine has; the test is skipped without one
    /// (no font file is committed).
    fn face() -> Option<Vec<u8>> {
        [
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
            "/System/Library/Fonts/Menlo.ttc",
            "C:\\Windows\\Fonts\\consola.ttf",
        ]
        .iter()
        .find_map(|p| std::fs::read(p).ok())
    }

    fn ink(f: &BitmapFont, c: char) -> Vec<(u32, u32)> {
        let q = f.layout(&c.to_string(), [0.0, 0.0], 1.0);
        let q = q.first().expect("the character has a glyph");
        let (aw, ah) = (f.atlas.width as f32, f.atlas.height as f32);
        let (x0, y0) = (
            (q.uv_min[0] * aw).round() as u32,
            (q.uv_min[1] * ah).round() as u32,
        );
        let mut out = vec![];
        for y in 0..f.glyph_px[1] {
            for x in 0..f.glyph_px[0] {
                if f.atlas.pixel(x0 + x, y0 + y)[3] > 100 {
                    out.push((x, y));
                }
            }
        }
        out
    }

    #[test]
    fn a_truetype_face_gives_lower_case_latin_1_and_the_same_grid() {
        let Some(bytes) = face() else {
            eprintln!("skipped: no monospaced font on this machine");
            return;
        };
        let f = BitmapFont::from_ttf(&bytes, 3).unwrap();
        assert_eq!(
            (f.advance, f.line_height, f.glyph, f.glyph_px, f.cell),
            (6, 9, [6, 9], [18, 27], [18, 27])
        );
        assert!(!f.fold_case);
        // Lower case is its own glyph, not the capital's.
        assert_ne!(ink(&f, 'a'), ink(&f, 'A'));
        assert!(ink(&f, 'a').len() > 20);
        // A descender reaches lower than a letter that sits on the line, and
        // a capital reaches higher than a lower-case x.
        let lowest = |c| ink(&f, c).iter().map(|p| p.1).max().unwrap();
        let highest = |c| ink(&f, c).iter().map(|p| p.1).min().unwrap();
        assert!(lowest('g') > lowest('x') + 2, "g has a tail");
        assert!(
            highest('H') < highest('x') - 2,
            "capitals are taller than x-height"
        );
        // Latin-1 is in the atlas: an e with an acute accent has ink.
        assert!(ink(&f, '\u{e9}').len() > 20);
        // Everything stays inside its cell: no glyph's ink touches a
        // neighbor's (sample the whole printable ASCII range).
        for c in '!'..='~' {
            assert!(ink(&f, c).iter().all(|&(x, y)| x < 18 && y < 27), "{c}");
        }
        // Layout is the grid: three characters are 18 units wide.
        assert_eq!(f.measure("abc", 1.0)[0], 6.0 * 2.0 + 6.0);
        // Garbage is an error, not a panic.
        assert!(BitmapFont::from_ttf(b"not a font", 2).is_err());
    }

    #[test]
    fn the_builtin_face_still_folds_case_and_uses_its_own_pixels() {
        let f = BitmapFont::builtin();
        assert!(f.fold_case);
        assert_eq!(f.glyph_px, f.glyph);
        assert_eq!(f.layout("a", [0.0; 2], 1.0), f.layout("A", [0.0; 2], 1.0));
    }
}

#[cfg(test)]
mod proportional_tests {
    use super::*;
    use sage_core::host::TextMetrics;

    fn face() -> Option<Vec<u8>> {
        [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        ]
        .iter()
        .find_map(|p| std::fs::read(p).ok())
    }

    #[test]
    fn a_shaped_face_gives_each_glyph_its_own_advance_and_kerning() {
        let Some(bytes) = face() else {
            eprintln!("skipped: no proportional font on this machine");
            return;
        };
        let f = BitmapFont::from_ttf_proportional(&bytes, 2).unwrap();
        let face = f.shaped.clone().expect("a shaped face");
        let w = |s: &str| face.line_width(s);
        // Letters are not all one width.
        assert!(
            w("iiii") < w("nnnn") && w("nnnn") < w("WWWW"),
            "{} {} {}",
            w("iiii"),
            w("nnnn"),
            w("WWWW")
        );
        // The font kerns A against V, so together they are narrower than
        // apart; the spaces are a width of their own.
        assert!(
            w("AV") < w("A") + w("V") - 0.05,
            "{} vs {}",
            w("AV"),
            w("A") + w("V")
        );
        assert!(w("a b") > w("ab") + 1.0);
        // Layout places glyphs by those advances: the second glyph of "iW" is
        // one `i` advance from the first.
        let q = f.layout("iW", [10.0, 0.0], 1.0);
        assert_eq!(q.len(), 2);
        assert!((q[1].pos[0] - (10.0 + w("i"))).abs() < 0.01);
        // Scale multiplies advances; measure is the shaped width.
        let q2 = f.layout("iW", [0.0, 0.0], 2.0);
        assert!((q2[1].pos[0] - 2.0 * w("i")).abs() < 0.01);
        assert!((f.measure("iW", 1.0)[0] - w("iW")).abs() < 0.01);
        // Lines advance by the line height, and spaces make no quad.
        let two = f.layout("a\nb c", [0.0, 0.0], 1.0);
        assert_eq!(two.len(), 3);
        assert_eq!(two[1].pos[1], 9.0);
        // The atlas has ink for lower case, and a glyph's quad covers its cell.
        let ink = |c: char| {
            let q = &f.layout(&c.to_string(), [0.0; 2], 1.0)[0];
            let (aw, ah) = (f.atlas.width as f32, f.atlas.height as f32);
            let (x0, y0) = (
                (q.uv_min[0] * aw).round() as u32,
                (q.uv_min[1] * ah).round() as u32,
            );
            (0..f.glyph_px[1])
                .flat_map(|y| (0..f.glyph_px[0]).map(move |x| (x, y)))
                .filter(|&(x, y)| f.atlas.pixel(x0 + x, y0 + y)[3] > 100)
                .count()
        };
        assert!(ink('a') > 20 && ink('W') > ink('i'));
        // Beyond Latin-1: Greek, Cyrillic and Latin Extended have glyphs.
        for c in [
            '\u{3a9}', '\u{3b1}', '\u{416}', '\u{44f}', '\u{141}', '\u{15f}',
        ] {
            assert!(ink(c) > 15, "{c}");
        }
        assert!(w("\u{416}\u{416}") > w("\u{44f}\u{44f}"));
        assert!(!f.fold_case);
        assert!(BitmapFont::from_ttf_proportional(b"nope", 2).is_err());
    }
}
