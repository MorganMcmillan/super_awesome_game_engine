//! A shaped, proportional text face.
//!
//! [`ShapedFace`] shapes a line of text with `rustybuzz` (kerning, ligatures
//! and the font's own substitutions) and reports each glyph's advance in UI
//! units. It implements [`sage_core::host::TextMetrics`], so layout measures
//! and wraps static text with the same widths that [`crate::BitmapFont`]
//! draws it with.

use std::collections::BTreeMap;

use sage_core::host::TextMetrics;

/// The distance between lines, and so the height one em is fitted to, in UI
/// units (the same grid the built-in face uses).
pub const LINE: f32 = 9.0;

pub struct ShapedFace {
    /// The font file. It lives as long as the process: `rustybuzz::Face`
    /// borrows it, and a face is loaded once.
    bytes: &'static [u8],
    upem: f32,
    /// One em in UI units, chosen so ascent plus descent is one line.
    pub em: f32,
    /// Glyph id to atlas cell, for the characters the atlas holds.
    pub cells: BTreeMap<u16, u32>,
    pub space: u16,
    /// The cell of `?`, drawn for a glyph the atlas does not hold.
    pub question: Option<u32>,
}

impl std::fmt::Debug for ShapedFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ShapedFace {{ em: {}, glyphs: {} }}",
            self.em,
            self.cells.len()
        )
    }
}

impl ShapedFace {
    /// Reads the face and works out its size. `cells` is filled by the atlas
    /// builder.
    pub fn new(bytes: &[u8]) -> Result<ShapedFace, String> {
        let leaked: &'static [u8] = Box::leak(bytes.to_vec().into_boxed_slice());
        let face = rustybuzz::Face::from_slice(leaked, 0).ok_or("font: not a usable face")?;
        let upem = face.units_per_em() as f32;
        let line = (face.ascender() as f32 - face.descender() as f32).max(1.0);
        Ok(ShapedFace {
            bytes: leaked,
            upem,
            em: LINE * upem / line,
            cells: BTreeMap::new(),
            space: 0,
            question: None,
        })
    }

    fn face(&self) -> rustybuzz::Face<'static> {
        rustybuzz::Face::from_slice(self.bytes, 0).expect("checked when the face was made")
    }

    /// The glyphs of one line: each glyph's id and its advance in UI units
    /// at scale 1, kerning included.
    pub fn shape_line(&self, line: &str) -> Vec<(u16, f32)> {
        let face = self.face();
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(line);
        buf.guess_segment_properties();
        let out = rustybuzz::shape(&face, &[], buf);
        out.glyph_infos()
            .iter()
            .zip(out.glyph_positions())
            .map(|(i, p)| (i.glyph_id as u16, p.x_advance as f32 / self.upem * self.em))
            .collect()
    }
}

impl TextMetrics for ShapedFace {
    fn line_width(&self, line: &str) -> f32 {
        self.shape_line(line).iter().map(|g| g.1).sum()
    }

    fn line_height(&self) -> f32 {
        LINE
    }
}
