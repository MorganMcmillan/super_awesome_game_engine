//! Layout: a small flow layout over a tree of [`Node`]s (design doc 9).
//!
//! Pure data in, pure data out: no Flecs, no Python, no clock. The ECS
//! module (`ecs.rs`) builds a [`Tree`] from entities, calls [`layout`], and
//! writes the computed rectangles back, which is the authored/computed split
//! of 9.2.
//!
//! The model: a node is a box. It has a size per axis ([`Size`]: `Auto`
//! fits its content, `Fixed`, or `Fill(weight)` takes a share of what the
//! parent has left), padding, and flows its children in a row or a column
//! with a gap, a cross-axis alignment and a main-axis justification.
//! Absolute children sit at an offset from the parent's content box and do
//! not take part in the flow. All edges are snapped to whole units, computed
//! from the same boundary, so neighbors share edges (no gaps, no overlap).

use sage_core::host::{UI_ADVANCE, UI_GLYPH, UI_LINE_HEIGHT};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// As large as the content needs.
    Auto,
    Fixed(f32),
    /// A share of the space left in the parent's flow, by weight.
    Fill(f32),
}

impl Size {
    /// From the component form: kind 0 auto, 1 fixed, 2 fill.
    pub fn from_parts(kind: u32, value: f32) -> Size {
        match kind {
            1 => Size::Fixed(value.max(0.0)),
            2 => Size::Fill(value.max(0.0)),
            _ => Size::Auto,
        }
    }

    pub fn parts(self) -> (u32, f32) {
        match self {
            Size::Auto => (0, 0.0),
            Size::Fixed(v) => (1, v),
            Size::Fill(w) => (2, w),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Column,
    Row,
}

impl Direction {
    pub fn from_u32(v: u32) -> Direction {
        if v == 1 {
            Direction::Row
        } else {
            Direction::Column
        }
    }
}

/// Cross-axis alignment of children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

impl Align {
    pub fn from_u32(v: u32) -> Align {
        match v {
            1 => Align::Center,
            2 => Align::End,
            3 => Align::Stretch,
            _ => Align::Start,
        }
    }
}

/// Main-axis distribution of leftover space (when nothing fills).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

impl Justify {
    pub fn from_u32(v: u32) -> Justify {
        match v {
            1 => Justify::Center,
            2 => Justify::End,
            3 => Justify::SpaceBetween,
            _ => Justify::Start,
        }
    }
}

/// What a node does with the pointer and the keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Widget {
    /// Decoration: the pointer passes through it.
    #[default]
    None,
    Button,
    Toggle,
    Slider,
    TextInput,
    /// Absorbs the pointer without doing anything: a modal backdrop or a
    /// panel that must not click through.
    Block,
    /// A read-only bar showing a value from its `Slider` component (a
    /// progress bar). The pointer passes through it and it takes no focus.
    Meter,
}

impl Widget {
    pub fn from_u32(v: u32) -> Widget {
        match v {
            1 => Widget::Button,
            2 => Widget::Toggle,
            3 => Widget::Slider,
            4 => Widget::TextInput,
            5 => Widget::Block,
            6 => Widget::Meter,
            _ => Widget::None,
        }
    }

    pub fn focusable(self) -> bool {
        matches!(
            self,
            Widget::Button | Widget::Toggle | Widget::Slider | Widget::TextInput
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// The overlap of two rectangles (zero-sized when they do not overlap).
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        Rect {
            x: x0,
            y: y0,
            w: (x1 - x0).max(0.0),
            h: (y1 - y0).max(0.0),
        }
    }

    pub fn contains(&self, p: [f32; 2]) -> bool {
        p[0] >= self.x && p[0] < self.x + self.w && p[1] >= self.y && p[1] < self.y + self.h
    }
}

/// Packed `0xRRGGBBAA`.
pub type Color = u32;

pub fn rgba(c: Color) -> [u8; 4] {
    c.to_be_bytes()
}

#[derive(Clone, Debug)]
pub struct Node {
    /// The StableId of the entity this node stands for.
    pub id: u64,
    /// Index of the parent in [`Tree::nodes`]; `None` for a root.
    pub parent: Option<usize>,
    pub width: Size,
    pub height: Size,
    pub direction: Direction,
    pub align: Align,
    pub justify: Justify,
    pub gap: f32,
    /// Left, top, right, bottom.
    pub padding: [f32; 4],
    /// Offset from the parent's content box (a root: from the screen's
    /// top-left). Takes the node out of the flow.
    pub absolute: Option<[f32; 2]>,
    /// Added to the parent's z; higher draws later (on top).
    pub z: i32,
    pub fill: Color,
    pub border: Color,
    pub border_width: f32,
    /// Slider fill, toggle mark and focus ring.
    pub accent: Color,
    pub text: String,
    pub text_color: Color,
    pub text_scale: f32,
    pub widget: Widget,
    pub enabled: bool,
    /// Tab order among focusable nodes; ties go in draw order.
    pub tab: i32,
    pub on: bool,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    /// A modal: while one exists, only its subtree can take keyboard focus.
    /// (Its `Block` widget or backdrop stops the pointer.) The topmost modal
    /// in draw order wins.
    pub modal: bool,
    /// Break the text into lines that fit the node's width.
    pub wrap: bool,
    /// A scroll container: its children are laid out as if it were as large
    /// as they need, shifted by `scroll_offset` along the flow direction, and
    /// cut off at its rectangle.
    pub scroll: bool,
    pub scroll_offset: f32,
    /// Text input: the caret as a character index.
    pub caret: usize,
    /// Set by interaction, read by drawing.
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    /// Computed by [`layout`].
    pub rect: Rect,
    /// The text as drawn: `text` broken into lines when `wrap` is set.
    pub lines: String,
    /// The size of a scroll container's content along its flow direction.
    pub content: f32,
    /// Where this node is visible: the rectangles of its scrolling
    /// ancestors, intersected. `None` means unclipped.
    pub clip: Option<Rect>,
}

impl Default for Node {
    fn default() -> Node {
        Node {
            id: 0,
            parent: None,
            width: Size::Auto,
            height: Size::Auto,
            direction: Direction::Column,
            align: Align::Start,
            justify: Justify::Start,
            gap: 0.0,
            padding: [0.0; 4],
            absolute: None,
            z: 0,
            fill: 0,
            border: 0,
            border_width: 0.0,
            accent: 0xffff_ffff,
            text: String::new(),
            text_color: 0xffff_ffff,
            text_scale: 1.0,
            widget: Widget::None,
            enabled: true,
            tab: 0,
            on: false,
            value: 0.0,
            min: 0.0,
            max: 1.0,
            step: 0.0,
            modal: false,
            wrap: false,
            scroll: false,
            scroll_offset: 0.0,
            caret: 0,
            hovered: false,
            pressed: false,
            focused: false,
            rect: Rect::default(),
            lines: String::new(),
            content: 0.0,
            clip: None,
        }
    }
}

/// Nodes in StableId order. Siblings flow in that order (creation order);
/// a `ZIndex` changes drawing and hit testing, never the flow.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub nodes: Vec<Node>,
}

/// Deepest nesting followed, so a malformed tree cannot recurse forever.
const MAX_DEPTH: usize = 64;

impl Tree {
    pub fn children(&self) -> Vec<Vec<usize>> {
        let mut out = vec![vec![]; self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(p) = n.parent
                && p < self.nodes.len()
                && p != i
            {
                out[p].push(i);
            }
        }
        out
    }

    pub fn roots(&self) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|&i| {
                self.nodes[i]
                    .parent
                    .is_none_or(|p| p >= self.nodes.len() || p == i)
            })
            .collect()
    }

    /// Back to front: depth first, children in order, then a stable sort on
    /// the z accumulated down the tree.
    pub fn draw_order(&self) -> Vec<usize> {
        let kids = self.children();
        let mut seen = vec![false; self.nodes.len()];
        let mut order: Vec<(i32, usize)> = vec![];
        fn walk(
            tree: &Tree,
            kids: &[Vec<usize>],
            seen: &mut [bool],
            order: &mut Vec<(i32, usize)>,
            i: usize,
            z: i32,
            depth: usize,
        ) {
            if seen[i] || depth > MAX_DEPTH {
                return;
            }
            seen[i] = true;
            let z = z.saturating_add(tree.nodes[i].z);
            order.push((z, i));
            for &c in &kids[i] {
                walk(tree, kids, seen, order, c, z, depth + 1);
            }
        }
        for r in self.roots() {
            walk(self, &kids, &mut seen, &mut order, r, 0, 0);
        }
        order.sort_by_key(|&(z, _)| z);
        order.into_iter().map(|(_, i)| i).collect()
    }
}

/// Size of `text` at `scale`, matching `BitmapFont::measure`.
pub fn text_size(text: &str, scale: f32) -> [f32; 2] {
    if text.is_empty() {
        return [0.0, 0.0];
    }
    let lines = text.split('\n');
    let count = lines.clone().count();
    let widest = lines.map(|l| l.chars().count()).max().unwrap_or(0);
    let w = if widest == 0 {
        0.0
    } else {
        ((widest - 1) as f32 * UI_ADVANCE + UI_GLYPH[0]) * scale
    };
    let h = ((count - 1) as f32 * UI_LINE_HEIGHT + UI_GLYPH[1]) * scale;
    [w, h]
}

/// How many characters fit in `width` at `scale`.
pub fn chars_per_line(width: f32, scale: f32) -> usize {
    let cols = ((width / scale - UI_GLYPH[0]) / UI_ADVANCE).floor() + 1.0;
    if cols.is_finite() && cols >= 1.0 {
        cols as usize
    } else {
        1
    }
}

/// Breaks `text` into lines of at most `width` units at `scale`: at spaces
/// when it can, in the middle of a word when a word is longer than a line.
/// Existing `\n` stay. Runs of spaces collapse only where a line breaks.
pub fn wrap_text(text: &str, scale: f32, width: f32) -> String {
    let max = chars_per_line(width, scale);
    let mut out: Vec<String> = vec![];
    for para in text.split('\n') {
        let mut line = String::new();
        let mut len = 0usize;
        for word in para.split(' ') {
            let mut word: Vec<char> = word.chars().collect();
            // Break a word longer than a line.
            while word.len() > max {
                if len > 0 {
                    out.push(std::mem::take(&mut line));
                    len = 0;
                }
                out.push(word.drain(..max).collect());
            }
            let need = word.len() + usize::from(len > 0);
            if len > 0 && len + need > max {
                out.push(std::mem::take(&mut line));
                len = 0;
            }
            if len > 0 {
                line.push(' ');
                len += 1;
            }
            line.extend(word.iter());
            len += word.len();
        }
        out.push(line);
    }
    out.join("\n")
}

fn spec(n: &Node, axis: usize) -> Size {
    if axis == 0 { n.width } else { n.height }
}

struct Pass<'a> {
    tree: &'a mut Tree,
    kids: Vec<Vec<usize>>,
    intrinsic: Vec<[f32; 2]>,
    /// Content width each wrapping node got on the previous pass.
    hints: Vec<Option<f32>>,
    /// Content width each wrapping node gets on this pass.
    next: Vec<Option<f32>>,
}

impl Pass<'_> {
    fn measure(&mut self, i: usize, depth: usize) -> [f32; 2] {
        if depth > MAX_DEPTH {
            return [0.0; 2];
        }
        let kids = self.kids[i].clone();
        let sizes: Vec<[f32; 2]> = kids.iter().map(|&k| self.measure(k, depth + 1)).collect();
        let n = &self.tree.nodes[i];
        let m = if n.direction == Direction::Row { 0 } else { 1 };
        let c = 1 - m;
        let flow: Vec<[f32; 2]> = kids
            .iter()
            .zip(&sizes)
            .filter(|&(&k, _)| self.tree.nodes[k].absolute.is_none())
            .map(|(&k, s)| {
                let kn = &self.tree.nodes[k];
                let mut s = *s;
                for (axis, v) in s.iter_mut().enumerate() {
                    if let Size::Fixed(f) = spec(kn, axis) {
                        *v = f;
                    }
                }
                s
            })
            .collect();
        let mut main: f32 = flow.iter().map(|s| s[m]).sum();
        if flow.len() > 1 {
            main += n.gap * (flow.len() - 1) as f32;
        }
        let cross = flow.iter().map(|s| s[c]).fold(0.0, f32::max);
        let mut content = [0.0; 2];
        content[m] = main;
        content[c] = cross;
        let shown = match (n.wrap, self.hints[i]) {
            (true, Some(width)) => wrap_text(&n.text, n.text_scale, width),
            _ => n.text.clone(),
        };
        let text = text_size(&shown, n.text_scale);
        let w = content[0].max(text[0]) + n.padding[0] + n.padding[2];
        let h = content[1].max(text[1]) + n.padding[1] + n.padding[3];
        self.intrinsic[i] = [w, h];
        [w, h]
    }

    fn arrange(&mut self, i: usize, r: Rect, clip: Option<Rect>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let (x0, y0) = (r.x.round(), r.y.round());
        let (x1, y1) = ((r.x + r.w).round(), (r.y + r.h).round());
        let rect = Rect {
            x: x0,
            y: y0,
            w: (x1 - x0).max(0.0),
            h: (y1 - y0).max(0.0),
        };
        self.tree.nodes[i].rect = rect;
        self.tree.nodes[i].clip = clip;
        let n = self.tree.nodes[i].clone();
        let child_clip = if n.scroll {
            Some(clip.map_or(rect, |c| c.intersect(&rect)))
        } else {
            clip
        };
        let content = Rect {
            x: rect.x + n.padding[0],
            y: rect.y + n.padding[1],
            w: (rect.w - n.padding[0] - n.padding[2]).max(0.0),
            h: (rect.h - n.padding[1] - n.padding[3]).max(0.0),
        };
        let origin = [content.x, content.y];
        let mut extent = [content.w, content.h];
        let m = if n.direction == Direction::Row { 0 } else { 1 };
        let c = 1 - m;
        if n.wrap {
            self.next[i] = Some(content.w);
        }
        // A scroll container lays its children out in as much room as they
        // need along the flow, then shifts them by the clamped offset.
        let mut scroll = 0.0;
        if n.scroll {
            let pad_main = n.padding[m] + n.padding[2 + m];
            let need = (self.intrinsic[i][m] - pad_main).max(0.0);
            scroll = n.scroll_offset.clamp(0.0, (need - extent[m]).max(0.0));
            let node = &mut self.tree.nodes[i];
            node.scroll_offset = scroll;
            node.content = need;
            extent[m] = extent[m].max(need);
        }
        let (flow, absolute): (Vec<usize>, Vec<usize>) = self.kids[i]
            .iter()
            .partition(|&&k| self.tree.nodes[k].absolute.is_none());

        let base: Vec<f32> = flow
            .iter()
            .map(|&k| match spec(&self.tree.nodes[k], m) {
                Size::Fixed(v) => v,
                Size::Auto => self.intrinsic[k][m],
                Size::Fill(_) => 0.0,
            })
            .collect();
        let weights: Vec<f32> = flow
            .iter()
            .map(|&k| match spec(&self.tree.nodes[k], m) {
                Size::Fill(w) => w,
                _ => 0.0,
            })
            .collect();
        let weight_sum: f32 = weights.iter().sum();
        let gaps = if flow.len() > 1 {
            n.gap * (flow.len() - 1) as f32
        } else {
            0.0
        };
        let free = extent[m] - base.iter().sum::<f32>() - gaps;
        let (mut offset, mut extra_gap) = (0.0, 0.0);
        if weight_sum <= 0.0 {
            let left = free.max(0.0);
            match n.justify {
                Justify::Start => {}
                Justify::Center => offset = left / 2.0,
                Justify::End => offset = left,
                Justify::SpaceBetween => {
                    if flow.len() > 1 {
                        extra_gap = left / (flow.len() - 1) as f32;
                    }
                }
            }
        }
        let mut pen = origin[m] + offset - scroll;
        for (idx, &k) in flow.iter().enumerate() {
            let main = if weight_sum > 0.0 && weights[idx] > 0.0 {
                free.max(0.0) * weights[idx] / weight_sum
            } else {
                base[idx]
            };
            let kn = &self.tree.nodes[k];
            let cross = match spec(kn, c) {
                Size::Fixed(v) => v,
                Size::Fill(_) => extent[c],
                Size::Auto => {
                    if n.align == Align::Stretch {
                        extent[c]
                    } else {
                        self.intrinsic[k][c]
                    }
                }
            };
            let cross_pos = origin[c]
                + match n.align {
                    Align::Start | Align::Stretch => 0.0,
                    Align::Center => (extent[c] - cross) / 2.0,
                    Align::End => extent[c] - cross,
                };
            let mut kr = [0.0; 4];
            kr[m] = pen;
            kr[c] = cross_pos;
            kr[2 + m] = main;
            kr[2 + c] = cross;
            self.arrange(
                k,
                Rect {
                    x: kr[0],
                    y: kr[1],
                    w: kr[2],
                    h: kr[3],
                },
                child_clip,
                depth + 1,
            );
            pen += main + n.gap + extra_gap;
        }
        for k in absolute {
            let [ax, ay] = self.tree.nodes[k].absolute.unwrap_or([0.0; 2]);
            let kn = &self.tree.nodes[k];
            let w = match kn.width {
                Size::Fixed(v) => v,
                Size::Fill(_) => extent[0],
                Size::Auto => self.intrinsic[k][0],
            };
            let h = match kn.height {
                Size::Fixed(v) => v,
                Size::Fill(_) => extent[1],
                Size::Auto => self.intrinsic[k][1],
            };
            let mut at = [origin[0] + ax, origin[1] + ay];
            at[m] -= scroll;
            self.arrange(
                k,
                Rect {
                    x: at[0],
                    y: at[1],
                    w,
                    h,
                },
                child_clip,
                depth + 1,
            );
        }
    }
}

/// Computes every node's `rect`, `clip`, `lines` and scroll state for a
/// screen of `viewport` units. Wrapping text makes a node's height depend on
/// its width, which the flow decides, so the pass repeats (at most four
/// times) until the widths wrapping nodes got stop changing.
pub fn layout(tree: &mut Tree, viewport: [f32; 2]) {
    let kids = tree.children();
    let roots = tree.roots();
    let count = tree.nodes.len();
    let mut hints: Vec<Option<f32>> = vec![None; count];
    for _ in 0..4 {
        let mut pass = Pass {
            tree,
            kids: kids.clone(),
            intrinsic: vec![[0.0; 2]; count],
            hints: hints.clone(),
            next: vec![None; count],
        };
        for &r in &roots {
            pass.measure(r, 0);
        }
        for &r in &roots {
            let n = &pass.tree.nodes[r];
            let [ox, oy] = n.absolute.unwrap_or([0.0; 2]);
            let size = |s: Size, own: f32, screen: f32| match s {
                Size::Fixed(v) => v,
                Size::Fill(_) => screen,
                Size::Auto => own,
            };
            let w = size(n.width, pass.intrinsic[r][0], viewport[0]);
            let h = size(n.height, pass.intrinsic[r][1], viewport[1]);
            pass.arrange(r, Rect { x: ox, y: oy, w, h }, None, 0);
        }
        let next = pass.next;
        if next == hints {
            break;
        }
        hints = next;
    }
    for n in &mut tree.nodes {
        n.lines = if n.wrap {
            let width = n.rect.w - n.padding[0] - n.padding[2];
            wrap_text(&n.text, n.text_scale, width)
        } else {
            n.text.clone()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(parent: Option<usize>) -> Node {
        Node {
            parent,
            ..Node::default()
        }
    }

    fn fixed(parent: Option<usize>, w: f32, h: f32) -> Node {
        Node {
            width: Size::Fixed(w),
            height: Size::Fixed(h),
            ..node(parent)
        }
    }

    fn rects(t: &Tree) -> Vec<[f32; 4]> {
        t.nodes
            .iter()
            .map(|n| [n.rect.x, n.rect.y, n.rect.w, n.rect.h])
            .collect()
    }

    #[test]
    fn a_column_stacks_children_with_gap_and_padding() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    padding: [4.0; 4],
                    gap: 2.0,
                    ..node(None)
                },
                fixed(Some(0), 20.0, 10.0),
                fixed(Some(0), 30.0, 12.0),
            ],
        };
        layout(&mut t, [200.0, 100.0]);
        // Auto root: widest child + padding, children + gap + padding.
        assert_eq!(
            rects(&t),
            vec![
                [0.0, 0.0, 38.0, 32.0],
                [4.0, 4.0, 20.0, 10.0],
                [4.0, 16.0, 30.0, 12.0],
            ]
        );
    }

    #[test]
    fn a_row_with_fill_children_splits_the_rest_by_weight() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    direction: Direction::Row,
                    width: Size::Fixed(100.0),
                    height: Size::Fixed(10.0),
                    gap: 5.0,
                    ..node(None)
                },
                fixed(Some(0), 20.0, 10.0),
                Node {
                    width: Size::Fill(1.0),
                    ..node(Some(0))
                },
                Node {
                    width: Size::Fill(3.0),
                    ..node(Some(0))
                },
            ],
        };
        layout(&mut t, [200.0, 100.0]);
        // 100 - 20 - two gaps of 5 = 70 shared 1:3 = 17.5 and 52.5.
        let r = rects(&t);
        assert_eq!(r[1], [0.0, 0.0, 20.0, 10.0]);
        assert_eq!(r[2][0], 25.0);
        assert_eq!(r[3][0] + r[3][2], 100.0, "the last child ends at the edge");
        // Fill on the cross axis stretches; an Auto child here has no
        // content, so it is 0 high only if nothing stretches it.
        assert_eq!(r[2][3], 0.0);
    }

    #[test]
    fn neighbors_share_edges_after_snapping() {
        let mut t = Tree {
            nodes: vec![Node {
                direction: Direction::Row,
                width: Size::Fixed(100.0),
                height: Size::Fixed(10.0),
                ..node(None)
            }],
        };
        for _ in 0..3 {
            t.nodes.push(Node {
                width: Size::Fill(1.0),
                height: Size::Fixed(10.0),
                ..node(Some(0))
            });
        }
        layout(&mut t, [100.0, 10.0]);
        let r = rects(&t);
        for k in 1..3 {
            assert_eq!(r[k][0] + r[k][2], r[k + 1][0], "no gap or overlap");
        }
        assert_eq!(r[3][0] + r[3][2], 100.0);
        // Every edge is a whole number.
        assert!(r.iter().flatten().all(|v| v.fract() == 0.0), "{r:?}");
    }

    #[test]
    fn alignment_and_justification() {
        let make = |align, justify| {
            let mut t = Tree {
                nodes: vec![
                    Node {
                        direction: Direction::Row,
                        width: Size::Fixed(100.0),
                        height: Size::Fixed(40.0),
                        align,
                        justify,
                        ..node(None)
                    },
                    fixed(Some(0), 10.0, 10.0),
                    fixed(Some(0), 10.0, 20.0),
                ],
            };
            layout(&mut t, [100.0, 40.0]);
            rects(&t)
        };
        let r = make(Align::Center, Justify::Center);
        assert_eq!(r[1], [40.0, 15.0, 10.0, 10.0]);
        assert_eq!(r[2], [50.0, 10.0, 10.0, 20.0]);
        let r = make(Align::End, Justify::End);
        assert_eq!(r[1], [80.0, 30.0, 10.0, 10.0]);
        let r = make(Align::Stretch, Justify::SpaceBetween);
        assert_eq!(r[1][0], 0.0);
        assert_eq!(r[2][0], 90.0);
        assert_eq!(r[1][3], 10.0, "a fixed height is kept even when stretching");
    }

    #[test]
    fn text_sets_the_size_of_a_leaf() {
        let mut t = Tree {
            nodes: vec![Node {
                text: "Hi".into(),
                text_scale: 2.0,
                padding: [3.0, 2.0, 3.0, 2.0],
                ..node(None)
            }],
        };
        layout(&mut t, [100.0, 100.0]);
        // "Hi": (1 * 6 + 5) * 2 = 22 wide, 7 * 2 = 14 high, plus padding.
        assert_eq!(rects(&t), vec![[0.0, 0.0, 28.0, 18.0]]);
        assert_eq!(text_size("", 3.0), [0.0, 0.0]);
        assert_eq!(text_size("a\nbcd", 1.0), [17.0, 16.0]);
    }

    #[test]
    fn absolute_children_leave_the_flow() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    padding: [5.0; 4],
                    ..fixed(None, 100.0, 50.0)
                },
                fixed(Some(0), 10.0, 10.0),
                Node {
                    absolute: Some([20.0, 7.0]),
                    ..fixed(Some(0), 4.0, 4.0)
                },
                fixed(Some(0), 10.0, 10.0),
            ],
        };
        layout(&mut t, [200.0, 200.0]);
        let r = rects(&t);
        assert_eq!(r[2], [25.0, 12.0, 4.0, 4.0]);
        assert_eq!(
            r[3][1], 15.0,
            "the third child follows the first, not the absolute one"
        );
    }

    #[test]
    fn draw_order_follows_the_tree_and_z() {
        let mut t = Tree {
            nodes: vec![
                node(None),
                node(Some(0)),
                node(Some(0)),
                node(Some(1)),
                Node { z: 5, ..node(None) },
            ],
        };
        assert_eq!(t.draw_order(), vec![0, 1, 3, 2, 4]);
        // A child's z lifts its whole subtree above its siblings.
        t.nodes[1].z = 1;
        assert_eq!(t.draw_order(), vec![0, 2, 1, 3, 4]);
    }

    #[test]
    fn a_malformed_tree_cannot_loop() {
        let mut t = Tree {
            nodes: vec![node(Some(1)), node(Some(0))],
        };
        layout(&mut t, [10.0, 10.0]);
        assert!(t.draw_order().len() <= 2);
        let mut s = Tree {
            nodes: vec![node(Some(0)), node(Some(9))],
        };
        layout(&mut s, [10.0, 10.0]);
        assert_eq!(
            s.roots(),
            vec![0, 1],
            "self and dangling parents make roots"
        );
    }

    #[test]
    fn text_wraps_at_spaces_and_breaks_long_words() {
        // 11 columns: 5 + 10 * 6 = 65 units at scale 1.
        assert_eq!(chars_per_line(65.0, 1.0), 11);
        assert_eq!(chars_per_line(64.9, 1.0), 10);
        assert_eq!(chars_per_line(0.0, 1.0), 1, "never fewer than one column");
        assert_eq!(chars_per_line(130.0, 2.0), 11, "scale divides the width");
        assert_eq!(
            wrap_text("the quick brown fox", 1.0, 65.0),
            "the quick\nbrown fox"
        );
        assert_eq!(wrap_text("short", 1.0, 65.0), "short");
        assert_eq!(
            wrap_text("abcdefghijklmnop", 1.0, 65.0),
            "abcdefghijk\nlmnop"
        );
        assert_eq!(
            wrap_text("a\n\nb", 1.0, 65.0),
            "a\n\nb",
            "explicit breaks stay"
        );
        assert_eq!(wrap_text("", 1.0, 65.0), "");
        // One column: every character is its own line.
        assert_eq!(wrap_text("aa bb", 1.0, 5.0), "a\na\nb\nb");
    }

    #[test]
    fn a_wrapping_label_gets_taller_when_its_width_is_narrow() {
        let make = |width: f32| {
            let mut t = Tree {
                nodes: vec![
                    Node {
                        width: Size::Fixed(width),
                        padding: [2.0; 4],
                        ..node(None)
                    },
                    Node {
                        width: Size::Fill(1.0),
                        text: "the quick brown fox jumps".into(),
                        wrap: true,
                        ..node(Some(0))
                    },
                    fixed(Some(0), 10.0, 5.0),
                ],
            };
            layout(&mut t, [300.0, 300.0]);
            t
        };
        // Wide: one line of 7 units, then the box below it.
        let wide = make(300.0);
        assert_eq!(wide.nodes[1].lines, "the quick brown fox jumps");
        assert_eq!(wide.nodes[1].rect.h, 7.0);
        // 71 units wide inside the padding: 11 columns, so three lines, and the
        // box below moves down by the extra two lines.
        let narrow = make(75.0);
        assert_eq!(narrow.nodes[1].lines, "the quick\nbrown fox\njumps");
        assert_eq!(narrow.nodes[1].rect.h, 7.0 + 2.0 * 9.0);
        assert_eq!(narrow.nodes[2].rect.y, 2.0 + 25.0);
        // The parent grew to hold it (it is Auto high).
        assert_eq!(narrow.nodes[0].rect.h, 2.0 + 25.0 + 5.0 + 2.0);
    }

    #[test]
    fn a_scroll_container_shifts_and_clips_its_children() {
        let mut nodes = vec![Node {
            width: Size::Fixed(50.0),
            height: Size::Fixed(30.0),
            scroll: true,
            scroll_offset: 25.0,
            padding: [0.0, 5.0, 0.0, 5.0],
            ..node(None)
        }];
        for _ in 0..4 {
            nodes.push(fixed(Some(0), 50.0, 20.0));
        }
        let mut t = Tree { nodes };
        layout(&mut t, [200.0, 200.0]);
        // Content: 4 * 20 = 80 inside a 20-unit view (30 - padding).
        assert_eq!(t.nodes[0].content, 80.0);
        assert_eq!(t.nodes[0].scroll_offset, 25.0);
        assert_eq!(t.nodes[1].rect.y, 5.0 - 25.0);
        assert_eq!(t.nodes[2].rect.y, 0.0, "5 + 20 - 25");
        let clip = t.nodes[1]
            .clip
            .expect("children of a scroll container are clipped");
        assert_eq!((clip.x, clip.y, clip.w, clip.h), (0.0, 0.0, 50.0, 30.0));
        assert_eq!(
            t.nodes[0].clip, None,
            "the container itself is not clipped by itself"
        );
        // An offset past the end is clamped.
        t.nodes[0].scroll_offset = 1000.0;
        layout(&mut t, [200.0, 200.0]);
        assert_eq!(t.nodes[0].scroll_offset, 60.0);
        t.nodes[0].scroll_offset = -5.0;
        layout(&mut t, [200.0, 200.0]);
        assert_eq!(t.nodes[0].scroll_offset, 0.0);
    }

    #[test]
    fn nested_scroll_containers_intersect_their_clips() {
        let mut t = Tree {
            nodes: vec![
                Node {
                    scroll: true,
                    ..fixed(None, 100.0, 50.0)
                },
                Node {
                    scroll: true,
                    scroll_offset: 0.0,
                    ..fixed(Some(0), 100.0, 80.0)
                },
                fixed(Some(1), 40.0, 200.0),
            ],
        };
        layout(&mut t, [300.0, 300.0]);
        let c = t.nodes[2].clip.unwrap();
        assert_eq!(
            (c.w, c.h),
            (100.0, 50.0),
            "the outer view is the smaller one"
        );
        assert_eq!(t.nodes[1].clip.map(|c| c.h), Some(50.0));
    }

    #[test]
    fn rect_intersection() {
        let a = Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        };
        let b = Rect {
            x: 5.0,
            y: 8.0,
            w: 10.0,
            h: 10.0,
        };
        assert_eq!(
            a.intersect(&b),
            Rect {
                x: 5.0,
                y: 8.0,
                w: 5.0,
                h: 2.0
            }
        );
        let far = Rect {
            x: 50.0,
            y: 50.0,
            w: 1.0,
            h: 1.0,
        };
        let none = a.intersect(&far);
        assert!(none.w == 0.0 || none.h == 0.0);
    }
}
