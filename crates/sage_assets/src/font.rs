//! A bitmap font: a grid atlas image plus metrics. The built-in font is a
//! 5x7 pixel face generated in code, so the engine needs no font file.
//! Text is upper case only (lower case maps to upper case).

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
    /// Visible glyph size inside a cell.
    pub glyph: [u32; 2],
    /// Horizontal pen advance and line height, in pixels at scale 1.
    pub advance: u32,
    pub line_height: u32,
    pub columns: u32,
    /// Character code of cell 0.
    pub first: u32,
}

impl BitmapFont {
    /// The atlas cell for `c`, or `?` when the font has no glyph for it.
    fn cell_of(&self, c: char) -> u32 {
        let c = c.to_ascii_uppercase();
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
                        u0 + self.glyph[0] as f32 / aw,
                        v0 + self.glyph[1] as f32 / ah,
                    ],
                });
            }
            x += self.advance as f32 * scale;
        }
        quads
    }

    /// Width and height of `text` in pixels.
    pub fn measure(&self, text: &str, scale: f32) -> [f32; 2] {
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
            advance: 6,
            line_height: 9,
            columns: 16,
            first: 32,
        }
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
