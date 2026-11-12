//! SVG documents drawn as UI triangles.
//!
//! [`parse`] reads an SVG document with `roxmltree` and [`draw`] turns it
//! into [`UiDraw`] commands fitted into a rectangle. It covers what icons and
//! diagrams use: `svg` (with `viewBox`), `g`, `path` (every command, arcs
//! included), `rect` (with `rx`), `circle`, `ellipse`, `line`, `polyline`
//! and `polygon`, the `fill`, `stroke`, `stroke-width`, `opacity`,
//! `fill-opacity`, `stroke-opacity` and `transform` attributes (also inside
//! `style`), and `translate`, `scale`, `rotate` and `matrix` transforms.
//! Fills are ear-clipped per subpath (a hole in a compound path is filled);
//! strokes are quads per segment with square ends; gradients, text, `<use>`,
//! clipping and CSS selectors are not drawn.
//!
//! Drawing is presentation: it uses trigonometry for arcs and rotations, so
//! nothing here feeds the world's hash.

use roxmltree::{Document, Node};
use sage_core::host::UiDraw;

/// One drawable shape, already flattened to points in document units and
/// transformed.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub subpaths: Vec<Subpath>,
    pub fill: Option<[u8; 4]>,
    pub stroke: Option<[u8; 4]>,
    pub stroke_width: f32,
}

/// A parsed document: its shapes and its own size (from `viewBox`, or
/// `width` and `height`).
#[derive(Clone, Debug, PartialEq)]
pub struct Svg {
    /// The top-left and size of the view box.
    pub view: [f32; 4],
    pub shapes: Vec<Shape>,
}

/// A run of points and whether it was closed.
pub type Subpath = (Vec<[f32; 2]>, bool);

type Matrix = [f32; 6]; // a b c d e f, as in SVG
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn mul(m: Matrix, n: Matrix) -> Matrix {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

fn apply(m: Matrix, p: [f32; 2]) -> [f32; 2] {
    [
        m[0] * p[0] + m[2] * p[1] + m[4],
        m[1] * p[0] + m[3] * p[1] + m[5],
    ]
}

fn numbers(text: &str) -> Vec<f32> {
    let mut out = vec![];
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'-' || c == b'+' || c == b'.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut dot = c == b'.';
            while i < b.len() {
                let d = b[i];
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == b'.' && !dot {
                    dot = true;
                    i += 1;
                } else if (d == b'e' || d == b'E')
                    && i + 1 < b.len()
                    && (b[i + 1].is_ascii_digit() || b[i + 1] == b'-' || b[i + 1] == b'+')
                {
                    i += 2;
                } else {
                    break;
                }
            }
            if let Ok(v) = text[start..i].parse() {
                out.push(v);
            }
        } else {
            i += 1;
        }
    }
    out
}

fn transform(text: &str) -> Result<Matrix, String> {
    let mut m = IDENTITY;
    let mut rest = text.trim();
    while !rest.is_empty() {
        let open = rest.find('(').ok_or("bad transform")?;
        let close = rest.find(')').ok_or("bad transform")?;
        let name = rest[..open].trim();
        let a = numbers(&rest[open + 1..close]);
        let t = match (name, a.as_slice()) {
            ("translate", [x]) => [1.0, 0.0, 0.0, 1.0, *x, 0.0],
            ("translate", [x, y]) => [1.0, 0.0, 0.0, 1.0, *x, *y],
            ("scale", [s]) => [*s, 0.0, 0.0, *s, 0.0, 0.0],
            ("scale", [x, y]) => [*x, 0.0, 0.0, *y, 0.0, 0.0],
            ("rotate", [d]) => rotation(*d, 0.0, 0.0),
            ("rotate", [d, x, y]) => rotation(*d, *x, *y),
            ("matrix", [a, b, c, d, e, f]) => [*a, *b, *c, *d, *e, *f],
            _ => return Err(format!("unsupported transform {name}")),
        };
        m = mul(m, t);
        rest = rest[close + 1..].trim_start_matches([' ', ',']);
    }
    Ok(m)
}

fn rotation(deg: f32, cx: f32, cy: f32) -> Matrix {
    let (s, c) = deg.to_radians().sin_cos();
    mul(
        mul([1.0, 0.0, 0.0, 1.0, cx, cy], [c, s, -s, c, 0.0, 0.0]),
        [1.0, 0.0, 0.0, 1.0, -cx, -cy],
    )
}

/// A color in SVG's forms: names, `#rgb`, `#rrggbb`, `rgb(r, g, b)`. `none`
/// gives `Ok(None)`.
pub fn color(text: &str) -> Result<Option<[u8; 4]>, String> {
    let t = text.trim().to_ascii_lowercase();
    let named = |c: [u8; 3]| Ok(Some([c[0], c[1], c[2], 255]));
    match t.as_str() {
        "none" | "transparent" => return Ok(None),
        "black" | "currentcolor" => return named([0, 0, 0]),
        "white" => return named([255, 255, 255]),
        "red" => return named([255, 0, 0]),
        "green" => return named([0, 128, 0]),
        "lime" => return named([0, 255, 0]),
        "blue" => return named([0, 0, 255]),
        "yellow" => return named([255, 255, 0]),
        "orange" => return named([255, 165, 0]),
        "gray" | "grey" => return named([128, 128, 128]),
        "teal" => return named([0, 128, 128]),
        "purple" => return named([128, 0, 128]),
        _ => {}
    }
    if let Some(h) = t.strip_prefix('#') {
        let d = |i: usize, n: usize| u8::from_str_radix(&h[i..i + n], 16);
        let bad = || format!("bad color {text:?}");
        return match h.len() {
            3 => {
                let v = |i| d(i, 1).map(|x| x * 17).map_err(|_| bad());
                Ok(Some([v(0)?, v(1)?, v(2)?, 255]))
            }
            6 => {
                let v = |i| d(i, 2).map_err(|_| bad());
                Ok(Some([v(0)?, v(2)?, v(4)?, 255]))
            }
            8 => {
                let v = |i| d(i, 2).map_err(|_| bad());
                Ok(Some([v(0)?, v(2)?, v(4)?, v(6)?]))
            }
            _ => Err(bad()),
        };
    }
    if let Some(args) = t.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let n = numbers(args);
        if n.len() == 3 {
            let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
            return Ok(Some([c(n[0]), c(n[1]), c(n[2]), 255]));
        }
    }
    Err(format!("unsupported color {text:?}"))
}

#[derive(Clone)]
struct Style {
    fill: Option<[u8; 4]>,
    stroke: Option<[u8; 4]>,
    width: f32,
    opacity: f32,
    fill_opacity: f32,
    stroke_opacity: f32,
}

impl Style {
    fn read(&mut self, n: Node) -> Result<(), String> {
        let mut set = |k: &str, v: &str| -> Result<(), String> {
            let f = |v: &str| v.trim().parse::<f32>().map_err(|_| format!("bad {k}"));
            match k {
                "fill" => self.fill = color(v)?,
                "stroke" => self.stroke = color(v)?,
                "stroke-width" => self.width = f(v)?,
                "opacity" => self.opacity *= f(v)?.clamp(0.0, 1.0),
                "fill-opacity" => self.fill_opacity = f(v)?.clamp(0.0, 1.0),
                "stroke-opacity" => self.stroke_opacity = f(v)?.clamp(0.0, 1.0),
                _ => {}
            }
            Ok(())
        };
        for a in n.attributes() {
            set(a.name(), a.value())?;
        }
        if let Some(style) = n.attribute("style") {
            for decl in style.split(';') {
                if let Some((k, v)) = decl.split_once(':') {
                    set(k.trim(), v.trim())?;
                }
            }
        }
        Ok(())
    }

    fn resolve(&self) -> (Option<[u8; 4]>, Option<[u8; 4]>) {
        let a = |c: [u8; 4], o: f32| [c[0], c[1], c[2], (f32::from(c[3]) * o).round() as u8];
        (
            self.fill.map(|c| a(c, self.opacity * self.fill_opacity)),
            self.stroke
                .map(|c| a(c, self.opacity * self.stroke_opacity)),
        )
    }
}

const CURVE_STEPS: usize = 16;

fn cubic(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2], out: &mut Vec<[f32; 2]>) {
    for i in 1..=CURVE_STEPS {
        let t = i as f32 / CURVE_STEPS as f32;
        let u = 1.0 - t;
        let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
        out.push([
            w[0] * a[0] + w[1] * b[0] + w[2] * c[0] + w[3] * d[0],
            w[0] * a[1] + w[1] * b[1] + w[2] * c[1] + w[3] * d[1],
        ]);
    }
}

/// An elliptical arc from `p0` to `p1` (the SVG endpoint form), as points.
#[allow(clippy::too_many_arguments)]
fn arc(
    p0: [f32; 2],
    mut rx: f32,
    mut ry: f32,
    rot: f32,
    large: bool,
    sweep: bool,
    p1: [f32; 2],
    out: &mut Vec<[f32; 2]>,
) {
    if rx == 0.0 || ry == 0.0 || p0 == p1 {
        out.push(p1);
        return;
    }
    rx = rx.abs();
    ry = ry.abs();
    let (s, c) = rot.to_radians().sin_cos();
    let (dx, dy) = ((p0[0] - p1[0]) / 2.0, (p0[1] - p1[1]) / 2.0);
    let (x1, y1) = (c * dx + s * dy, -s * dx + c * dy);
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let k = lambda.sqrt();
        rx *= k;
        ry *= k;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = (num / den).max(0.0).sqrt();
    if large == sweep {
        k = -k;
    }
    let (cxp, cyp) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    let cx = c * cxp - s * cyp + (p0[0] + p1[0]) / 2.0;
    let cy = s * cxp + c * cyp + (p0[1] + p1[1]) / 2.0;
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let th1 = angle(1.0, 0.0, (x1 - cxp) / rx, (y1 - cyp) / ry);
    let mut dth = angle(
        (x1 - cxp) / rx,
        (y1 - cyp) / ry,
        (-x1 - cxp) / rx,
        (-y1 - cyp) / ry,
    );
    if !sweep && dth > 0.0 {
        dth -= std::f32::consts::TAU;
    } else if sweep && dth < 0.0 {
        dth += std::f32::consts::TAU;
    }
    let steps = ((dth.abs() / std::f32::consts::TAU * 48.0).ceil() as usize).max(4);
    for i in 1..=steps {
        let t = th1 + dth * i as f32 / steps as f32;
        let (st, ct) = t.sin_cos();
        out.push([
            cx + c * rx * ct - s * ry * st,
            cy + s * rx * ct + c * ry * st,
        ]);
    }
    if let Some(last) = out.last_mut() {
        *last = p1;
    }
}

/// Path data as subpaths (points and whether it was closed).
pub fn flatten_path(d: &str) -> Result<Vec<Subpath>, String> {
    let b = d.as_bytes();
    let mut subs = vec![];
    let mut pts: Vec<[f32; 2]> = vec![];
    let (mut cur, mut start) = ([0.0f32; 2], [0.0f32; 2]);
    let mut last_ctrl: Option<[f32; 2]> = None;
    let mut last_cmd = b' ';
    let mut i = 0;
    let finish = |pts: &mut Vec<[f32; 2]>, closed: bool, subs: &mut Vec<_>| {
        if pts.len() > 1 || (closed && !pts.is_empty()) {
            subs.push((std::mem::take(pts), closed));
        } else {
            pts.clear();
        }
    };
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_alphabetic() && c != b'e' && c != b'E' {
            i += 1;
            let mut end = i;
            while end < b.len()
                && !(b[end].is_ascii_alphabetic() && b[end] != b'e' && b[end] != b'E')
            {
                end += 1;
            }
            let args = numbers(&d[i..end]);
            i = end;
            let rel = c.is_ascii_lowercase();
            let up = c.to_ascii_uppercase();
            if up == b'Z' {
                finish(&mut pts, true, &mut subs);
                cur = start;
                last_ctrl = None;
                last_cmd = c;
                continue;
            }
            let per = match up {
                b'M' | b'L' | b'T' => 2,
                b'H' | b'V' => 1,
                b'C' => 6,
                b'S' | b'Q' => 4,
                b'A' => 7,
                _ => return Err(format!("unknown path command {}", c as char)),
            };
            if args.is_empty() || !args.len().is_multiple_of(per) {
                return Err(format!("path command {} needs {per} numbers", c as char));
            }
            for (k, a) in args.chunks(per).enumerate() {
                let o = if rel { cur } else { [0.0, 0.0] };
                match up {
                    b'M' => {
                        let p = [o[0] + a[0], o[1] + a[1]];
                        if k == 0 {
                            finish(&mut pts, false, &mut subs);
                            start = p;
                        }
                        pts.push(p);
                        cur = p;
                        last_ctrl = None;
                    }
                    b'L' => {
                        cur = [o[0] + a[0], o[1] + a[1]];
                        pts.push(cur);
                        last_ctrl = None;
                    }
                    b'H' => {
                        cur = [if rel { cur[0] + a[0] } else { a[0] }, cur[1]];
                        pts.push(cur);
                        last_ctrl = None;
                    }
                    b'V' => {
                        cur = [cur[0], if rel { cur[1] + a[0] } else { a[0] }];
                        pts.push(cur);
                        last_ctrl = None;
                    }
                    b'C' | b'S' => {
                        let (c1, c2, p) = if up == b'C' {
                            (
                                [o[0] + a[0], o[1] + a[1]],
                                [o[0] + a[2], o[1] + a[3]],
                                [o[0] + a[4], o[1] + a[5]],
                            )
                        } else {
                            let reflected = last_ctrl
                                .filter(|_| matches!(last_cmd.to_ascii_uppercase(), b'C' | b'S'))
                                .map_or(cur, |q| [2.0 * cur[0] - q[0], 2.0 * cur[1] - q[1]]);
                            (
                                reflected,
                                [o[0] + a[0], o[1] + a[1]],
                                [o[0] + a[2], o[1] + a[3]],
                            )
                        };
                        cubic(cur, c1, c2, p, &mut pts);
                        last_ctrl = Some(c2);
                        cur = p;
                    }
                    b'Q' | b'T' => {
                        let (q, p) = if up == b'Q' {
                            ([o[0] + a[0], o[1] + a[1]], [o[0] + a[2], o[1] + a[3]])
                        } else {
                            let reflected = last_ctrl
                                .filter(|_| matches!(last_cmd.to_ascii_uppercase(), b'Q' | b'T'))
                                .map_or(cur, |q| [2.0 * cur[0] - q[0], 2.0 * cur[1] - q[1]]);
                            (reflected, [o[0] + a[0], o[1] + a[1]])
                        };
                        // A quadratic is a cubic with these controls.
                        let c1 = [
                            cur[0] + 2.0 / 3.0 * (q[0] - cur[0]),
                            cur[1] + 2.0 / 3.0 * (q[1] - cur[1]),
                        ];
                        let c2 = [
                            p[0] + 2.0 / 3.0 * (q[0] - p[0]),
                            p[1] + 2.0 / 3.0 * (q[1] - p[1]),
                        ];
                        cubic(cur, c1, c2, p, &mut pts);
                        last_ctrl = Some(q);
                        cur = p;
                    }
                    b'A' => {
                        let p = [o[0] + a[5], o[1] + a[6]];
                        arc(cur, a[0], a[1], a[2], a[3] != 0.0, a[4] != 0.0, p, &mut pts);
                        cur = p;
                        last_ctrl = None;
                    }
                    _ => unreachable!(),
                }
                last_cmd = if up == b'M' && k == 0 {
                    c
                } else if up == b'M' {
                    b'L'
                } else {
                    c
                };
            }
        } else {
            i += 1;
        }
    }
    finish(&mut pts, false, &mut subs);
    Ok(subs)
}

fn walk(n: Node, m: Matrix, style: &Style, out: &mut Vec<Shape>) -> Result<(), String> {
    for child in n.children().filter(Node::is_element) {
        let mut st = style.clone();
        st.read(child)?;
        let mut mm = m;
        if let Some(t) = child.attribute("transform") {
            mm = mul(m, transform(t)?);
        }
        let f = |k: &str| -> f32 {
            child
                .attribute(k)
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0.0)
        };
        let tag = child.tag_name().name();
        let mut subs: Vec<(Vec<[f32; 2]>, bool)> = match tag {
            "g" | "svg" => {
                walk(child, mm, &st, out)?;
                continue;
            }
            "path" => flatten_path(child.attribute("d").unwrap_or(""))?,
            "rect" => {
                let (x, y, w, h) = (f("x"), f("y"), f("width"), f("height"));
                let rx = f("rx").max(f("ry")).min(w / 2.0).min(h / 2.0);
                if rx > 0.0 {
                    let mut p = vec![];
                    let corner = |cx: f32, cy: f32, a0: f32, p: &mut Vec<[f32; 2]>| {
                        for i in 0..=6 {
                            let t = a0 + i as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
                            p.push([cx + rx * t.cos(), cy + rx * t.sin()]);
                        }
                    };
                    corner(x + w - rx, y + rx, -std::f32::consts::FRAC_PI_2, &mut p);
                    corner(x + w - rx, y + h - rx, 0.0, &mut p);
                    corner(x + rx, y + h - rx, std::f32::consts::FRAC_PI_2, &mut p);
                    corner(x + rx, y + rx, std::f32::consts::PI, &mut p);
                    vec![(p, true)]
                } else {
                    vec![(vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]], true)]
                }
            }
            "circle" | "ellipse" => {
                let (cx, cy) = (f("cx"), f("cy"));
                let (rx, ry) = if tag == "circle" {
                    (f("r"), f("r"))
                } else {
                    (f("rx"), f("ry"))
                };
                let n = ((rx.max(ry) * 2.0) as usize).clamp(16, 96);
                let p = (0..n)
                    .map(|k| {
                        let t = k as f32 / n as f32 * std::f32::consts::TAU;
                        [cx + rx * t.cos(), cy + ry * t.sin()]
                    })
                    .collect();
                vec![(p, true)]
            }
            "line" => {
                st.fill = None;
                vec![(vec![[f("x1"), f("y1")], [f("x2"), f("y2")]], false)]
            }
            "polygon" | "polyline" => {
                let n = numbers(child.attribute("points").unwrap_or(""));
                let p = n.as_chunks::<2>().0.iter().map(|c| [c[0], c[1]]).collect();
                vec![(p, tag == "polygon")]
            }
            _ => continue,
        };
        for (pts, _) in &mut subs {
            for p in pts.iter_mut() {
                *p = apply(mm, *p);
            }
        }
        // The stroke width scales with the transform.
        let scale = (mm[0] * mm[3] - mm[1] * mm[2]).abs().sqrt();
        let (fill, stroke) = st.resolve();
        out.push(Shape {
            subpaths: subs,
            fill,
            stroke,
            stroke_width: st.width * scale,
        });
    }
    Ok(())
}

/// Parses an SVG document.
pub fn parse(markup: &str) -> Result<Svg, String> {
    let doc = Document::parse(markup).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return Err("the document is not an <svg>".into());
    }
    let size = |k: &str| root.attribute(k).and_then(|v| numbers(v).first().copied());
    let view = match root.attribute("viewBox").map(numbers) {
        Some(v) if v.len() == 4 => [v[0], v[1], v[2], v[3]],
        _ => [
            0.0,
            0.0,
            size("width").unwrap_or(100.0),
            size("height").unwrap_or(100.0),
        ],
    };
    let base = Style {
        fill: Some([0, 0, 0, 255]),
        stroke: None,
        width: 1.0,
        opacity: 1.0,
        fill_opacity: 1.0,
        stroke_opacity: 1.0,
    };
    let mut st = base.clone();
    st.read(root)?;
    let mut shapes = vec![];
    let m = match root.attribute("transform") {
        Some(t) => transform(t)?,
        None => IDENTITY,
    };
    walk(root, m, &st, &mut shapes)?;
    Ok(Svg { view, shapes })
}

fn cross(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Splits a simple polygon into triangles by ear clipping.
pub fn triangulate(poly: &[[f32; 2]]) -> Vec<[[f32; 2]; 3]> {
    let mut p: Vec<[f32; 2]> = poly.to_vec();
    if p.len() > 1 && p.first() == p.last() {
        p.pop();
    }
    if p.len() < 3 {
        return vec![];
    }
    let area: f32 = (0..p.len())
        .map(|i| p[i][0] * p[(i + 1) % p.len()][1] - p[(i + 1) % p.len()][0] * p[i][1])
        .sum();
    if area < 0.0 {
        p.reverse();
    }
    let mut out = vec![];
    let mut guard = p.len() * p.len() + 8;
    while p.len() > 3 && guard > 0 {
        guard -= 1;
        let n = p.len();
        let ear = (0..n).find(|&i| {
            let (a, b, c) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n]);
            cross(a, b, c) > 0.0
                && !p.iter().any(|&q| {
                    q != a
                        && q != b
                        && q != c
                        && cross(a, b, q) >= 0.0
                        && cross(b, c, q) >= 0.0
                        && cross(c, a, q) >= 0.0
                })
        });
        match ear {
            Some(i) => {
                let n = p.len();
                out.push([p[(i + n - 1) % n], p[i], p[(i + 1) % n]]);
                p.remove(i);
            }
            None => break,
        }
    }
    if p.len() == 3 && cross(p[0], p[1], p[2]) != 0.0 {
        out.push([p[0], p[1], p[2]]);
    }
    out
}

/// The draw commands for `svg` fitted (contain, centered) into the box
/// `pos` and `size`, cut to `clip` when given.
pub fn draw(svg: &Svg, pos: [f32; 2], size: [f32; 2], clip: Option<[f32; 4]>) -> Vec<UiDraw> {
    let [vx, vy, vw, vh] = svg.view;
    if vw <= 0.0 || vh <= 0.0 {
        return vec![];
    }
    let scale = (size[0] / vw).min(size[1] / vh);
    let origin = [
        pos[0] + (size[0] - vw * scale) / 2.0 - vx * scale,
        pos[1] + (size[1] - vh * scale) / 2.0 - vy * scale,
    ];
    let bounds = clip.unwrap_or([pos[0], pos[1], pos[0] + size[0], pos[1] + size[1]]);
    let at = |p: [f32; 2]| [origin[0] + p[0] * scale, origin[1] + p[1] * scale];
    let mut out = vec![];
    let emit = |poly: &[[f32; 2]], color: [u8; 4], out: &mut Vec<UiDraw>| {
        let cut = crate::draw::clip_polygon(poly, bounds);
        for k in 1..cut.len().saturating_sub(1) {
            out.push(UiDraw::Triangle {
                pts: [cut[0], cut[k], cut[k + 1]],
                colors: [color; 3],
            });
        }
    };
    for s in &svg.shapes {
        for (pts, closed) in &s.subpaths {
            let pts: Vec<[f32; 2]> = pts.iter().map(|&p| at(p)).collect();
            if let Some(fill) = s.fill.filter(|c| c[3] > 0)
                && pts.len() >= 3
            {
                for t in triangulate(&pts) {
                    emit(&t, fill, &mut out);
                }
            }
            if let Some(stroke) = s.stroke.filter(|c| c[3] > 0) {
                let half = (s.stroke_width * scale).max(1.0) / 2.0;
                let mut edges: Vec<([f32; 2], [f32; 2])> =
                    pts.windows(2).map(|w| (w[0], w[1])).collect();
                if *closed && pts.len() > 2 {
                    edges.push((pts[pts.len() - 1], pts[0]));
                }
                for (a, b) in edges {
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let len = (dx * dx + dy * dy).sqrt();
                    if len == 0.0 {
                        continue;
                    }
                    let (nx, ny) = (-dy / len * half, dx / len * half);
                    // Extend each end by half a width so corners meet.
                    let (ex, ey) = (dx / len * half, dy / len * half);
                    let quad = [
                        [a[0] - ex + nx, a[1] - ey + ny],
                        [b[0] + ex + nx, b[1] + ey + ny],
                        [b[0] + ex - nx, b[1] + ey - ny],
                        [a[0] - ex - nx, a[1] - ey - ny],
                    ];
                    emit(&quad, stroke, &mut out);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(t: &[[[f32; 2]; 3]]) -> f32 {
        t.iter().map(|t| cross(t[0], t[1], t[2]).abs() / 2.0).sum()
    }

    #[test]
    fn path_data_covers_every_command() {
        let sq = flatten_path("M0 0 L10 0 V10 h-10 z").unwrap();
        assert_eq!(
            sq,
            vec![(
                vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
                true
            )]
        );
        // Relative and implicit repeats: "m 1 1 2 0" is a move then a line.
        let m = flatten_path("m1 1 2 0 0 2").unwrap();
        assert_eq!(m[0].0, vec![[1.0, 1.0], [3.0, 1.0], [3.0, 3.0]]);
        assert_eq!(flatten_path("M0 0 L1 1 M5 5 L6 6").unwrap().len(), 2);
        // A cubic and a quadratic end where they say; smooth ones reflect.
        let c = flatten_path("M0 0 C 10 0 10 10 0 10").unwrap();
        assert_eq!(c[0].0.len(), 1 + CURVE_STEPS);
        assert_eq!(*c[0].0.last().unwrap(), [0.0, 10.0]);
        let s = flatten_path("M0 0 C 0 5 5 5 5 0 S 10 -5 10 0").unwrap();
        assert_eq!(*s[0].0.last().unwrap(), [10.0, 0.0]);
        let q = flatten_path("M0 0 Q 5 10 10 0 T 20 0").unwrap();
        assert_eq!(*q[0].0.last().unwrap(), [20.0, 0.0]);
        // Errors name the command.
        assert!(flatten_path("M0 0 L5").unwrap_err().contains("needs 2"));
        assert!(flatten_path("M0 0 X5 5").is_err());
    }

    #[test]
    fn an_arc_is_a_circle_through_its_end_points() {
        // A half circle of radius 5 from (0,0) to (10,0): every point is 5
        // from the center (5, 0), and it bulges the way the sweep says.
        let up = flatten_path("M0 0 A5 5 0 0 1 10 0").unwrap();
        for p in &up[0].0 {
            let d = ((p[0] - 5.0).powi(2) + p[1].powi(2)).sqrt();
            assert!((d - 5.0).abs() < 0.01, "{p:?} {d}");
        }
        assert!(
            up[0].0.iter().any(|p| p[1] < -4.9),
            "sweep 1 goes up (y negative)"
        );
        let down = flatten_path("M0 0 A5 5 0 0 0 10 0").unwrap();
        assert!(down[0].0.iter().any(|p| p[1] > 4.9));
        assert_eq!(*down[0].0.last().unwrap(), [10.0, 0.0]);
        // Radii too small are scaled up to reach.
        let r = flatten_path("M0 0 A1 1 0 0 1 10 0").unwrap();
        assert_eq!(*r[0].0.last().unwrap(), [10.0, 0.0]);
    }

    #[test]
    fn ear_clipping_keeps_the_area_of_a_concave_polygon() {
        let star = [
            [5.0, 0.0],
            [6.2, 3.5],
            [10.0, 3.5],
            [7.0, 5.8],
            [8.0, 10.0],
            [5.0, 7.5],
            [2.0, 10.0],
            [3.0, 5.8],
            [0.0, 3.5],
            [3.8, 3.5],
        ];
        let t = triangulate(&star);
        assert_eq!(t.len(), 8);
        assert!((area(&t) - 33.8).abs() < 0.05, "{}", area(&t));
        // Clockwise input works too.
        let mut rev = star;
        rev.reverse();
        assert!((area(&triangulate(&rev)) - 33.8).abs() < 0.05);
    }

    #[test]
    fn documents_inherit_style_and_transform_through_groups() {
        let svg = parse(
            r##"<svg viewBox="0 0 20 10" fill="#f00">
                 <g transform="translate(10 0) scale(2)" stroke="blue" stroke-width="1" opacity="0.5">
                   <rect x="0" y="0" width="2" height="3" fill="lime"/>
                   <circle cx="1" cy="1" r="1" style="fill:#00f;stroke:none"/>
                 </g>
                 <line x1="0" y1="0" x2="5" y2="5" stroke="black"/>
               </svg>"##,
        )
        .unwrap();
        assert_eq!(svg.view, [0.0, 0.0, 20.0, 10.0]);
        assert_eq!(svg.shapes.len(), 3);
        let rect = &svg.shapes[0];
        assert_eq!(
            rect.subpaths[0].0[1],
            [14.0, 0.0],
            "translated by 10 and scaled by 2"
        );
        assert_eq!(rect.fill, Some([0, 255, 0, 128]), "half opacity");
        assert_eq!(rect.stroke, Some([0, 0, 255, 128]));
        assert_eq!(rect.stroke_width, 2.0, "the stroke scales");
        assert_eq!(svg.shapes[1].fill, Some([0, 0, 255, 128]));
        assert_eq!(svg.shapes[1].stroke, None, "style: overrides");
        assert_eq!(svg.shapes[2].fill, None, "a line has no fill");
        // Rotation about a point, matrices, and rounded rectangles.
        let r = parse(r#"<svg viewBox="0 0 10 10"><rect width="4" height="2" transform="rotate(90 5 5)"/></svg>"#).unwrap();
        let p = &r.shapes[0].subpaths[0].0;
        assert!(
            (p[0][0] - 10.0).abs() < 1e-4 && (p[0][1] - 0.0).abs() < 1e-4,
            "{p:?}"
        );
        let rr =
            parse(r#"<svg viewBox="0 0 10 10"><rect width="8" height="8" rx="2"/></svg>"#).unwrap();
        assert!(rr.shapes[0].subpaths[0].0.len() > 20, "corners are curved");
    }

    #[test]
    fn bad_documents_and_colors_are_errors_not_silence() {
        assert!(parse("<svg><path d=\"M0 0 L5\"/></svg>").is_err());
        assert!(parse("<html/>").unwrap_err().contains("not an <svg>"));
        assert!(parse("<svg").is_err());
        assert!(parse("<svg><rect fill=\"nope\"/></svg>").is_err());
        assert!(parse("<svg><rect transform=\"skew(1)\"/></svg>").is_err());
        assert_eq!(color("#abc").unwrap(), Some([170, 187, 204, 255]));
        assert_eq!(color("rgb(1, 2, 3)").unwrap(), Some([1, 2, 3, 255]));
        assert_eq!(color("none").unwrap(), None);
        assert_eq!(color("#11223344").unwrap(), Some([17, 34, 51, 68]));
    }

    #[test]
    fn drawing_fits_the_view_box_into_the_box_and_cuts_to_the_clip() {
        let svg =
            parse(r#"<svg viewBox="0 0 10 10"><rect width="10" height="10" fill="red"/></svg>"#)
                .unwrap();
        // A 10x10 view in a 40x20 box: scale 2, centered sideways.
        let d = draw(&svg, [100.0, 0.0], [40.0, 20.0], None);
        let pts: Vec<[f32; 2]> = d
            .iter()
            .flat_map(|d| match d {
                UiDraw::Triangle { pts, .. } => pts.to_vec(),
                _ => vec![],
            })
            .collect();
        let (x0, x1) = (
            pts.iter().map(|p| p[0]).fold(f32::MAX, f32::min),
            pts.iter().map(|p| p[0]).fold(f32::MIN, f32::max),
        );
        assert_eq!((x0, x1), (110.0, 130.0));
        // A clip tighter than the drawing cuts it.
        let cut = draw(
            &svg,
            [100.0, 0.0],
            [40.0, 20.0],
            Some([115.0, 0.0, 125.0, 20.0]),
        );
        for d in &cut {
            if let UiDraw::Triangle { pts, .. } = d {
                assert!(
                    pts.iter()
                        .all(|p| p[0] >= 115.0 - 1e-3 && p[0] <= 125.0 + 1e-3)
                );
            }
        }
        assert!(!cut.is_empty());
    }
}
