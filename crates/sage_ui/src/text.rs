//! The text editing engine (design doc 9.5, parity plan F4): pure logic over
//! a string and two character indices, so single-line fields, text areas and
//! number fields share one set of rules. Nothing here knows about nodes,
//! components or fonts; wrapping asks for a column count, because the only
//! face today is a fixed-advance bitmap.
//!
//! A field is `text` plus a caret and an anchor (both character indices).
//! The selection is the range between them; when they are equal there is
//! none. History is a bounded undo and redo stack of whole-text snapshots,
//! small enough to live in a component.

/// How many snapshots each of the undo and redo stacks keeps.
pub const HISTORY: usize = 6;

/// What a field accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// The longest text in characters.
    pub max_len: usize,
    /// Newlines are allowed (a text area).
    pub multiline: bool,
    /// Only characters a number is written with: digits, `-`, `+`, `.`, `e`.
    pub numeric: bool,
}

impl Default for Rules {
    fn default() -> Rules {
        Rules {
            max_len: 256,
            multiline: false,
            numeric: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub text: String,
    pub caret: usize,
}

/// What the last edit was, so a run of typing is one undo step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    None,
    Typing,
    Deleting,
    Other,
}

impl Kind {
    pub fn from_u32(v: u32) -> Kind {
        match v {
            1 => Kind::Typing,
            2 => Kind::Deleting,
            3 => Kind::Other,
            _ => Kind::None,
        }
    }

    pub fn to_u32(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    /// Oldest first.
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub last: Kind,
}

impl History {
    /// Records the state before an edit of `kind`. Consecutive typing, or
    /// consecutive deleting, joins the step already on the stack.
    pub fn record(&mut self, before: Snapshot, kind: Kind) {
        let joins = kind != Kind::Other && kind == self.last && !self.undo.is_empty();
        if !joins {
            self.undo.push(before);
            if self.undo.len() > HISTORY {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last = kind;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Field {
    pub text: String,
    pub caret: usize,
    pub anchor: usize,
}

pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

pub fn byte_of(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map_or(s.len(), |(b, _)| b)
}

fn chars_of(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The start of the word at or before `pos`: skips spaces to the left, then
/// the word itself. A run of punctuation counts as a word.
pub fn word_left(text: &str, pos: usize) -> usize {
    let c = chars_of(text);
    let mut i = pos.min(c.len());
    while i > 0 && c[i - 1].is_whitespace() && c[i - 1] != '\n' {
        i -= 1;
    }
    if i > 0 && c[i - 1] == '\n' {
        return i - 1;
    }
    if i == 0 {
        return 0;
    }
    let w = is_word(c[i - 1]);
    while i > 0 && !c[i - 1].is_whitespace() && is_word(c[i - 1]) == w {
        i -= 1;
    }
    i
}

/// The end of the word at or after `pos`.
pub fn word_right(text: &str, pos: usize) -> usize {
    let c = chars_of(text);
    let n = c.len();
    let mut i = pos.min(n);
    while i < n && c[i].is_whitespace() && c[i] != '\n' {
        i += 1;
    }
    if i < n && c[i] == '\n' {
        return i + 1;
    }
    if i >= n {
        return n;
    }
    let w = is_word(c[i]);
    while i < n && !c[i].is_whitespace() && is_word(c[i]) == w {
        i += 1;
    }
    i
}

/// `(start, end)` of the logical line containing `pos` (the end is before
/// the `\n`).
pub fn line_bounds(text: &str, pos: usize) -> (usize, usize) {
    let c = chars_of(text);
    let pos = pos.min(c.len());
    let start = c[..pos]
        .iter()
        .rposition(|&x| x == '\n')
        .map_or(0, |i| i + 1);
    let end = c[pos..]
        .iter()
        .position(|&x| x == '\n')
        .map_or(c.len(), |i| pos + i);
    (start, end)
}

/// One displayed row of text: the characters `start..end` of the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub start: usize,
    pub end: usize,
}

/// Rows of at most `cols` characters: breaks after a space when it can, in
/// the middle of a word when it must, and at every `\n`. A character index
/// belongs to exactly one row (a break after a space belongs to the row it
/// ends); the caret at a row's end is drawn there.
pub fn rows(text: &str, cols: usize, wrap: bool) -> Vec<Row> {
    let c = chars_of(text);
    let cols = cols.max(1);
    let mut out = vec![];
    let mut start = 0;
    while start <= c.len() {
        let nl = c[start..]
            .iter()
            .position(|&x| x == '\n')
            .map_or(c.len(), |i| start + i);
        let mut s = start;
        loop {
            if !wrap || nl - s <= cols {
                out.push(Row { start: s, end: nl });
                break;
            }
            // Prefer the last space inside the window; the space stays on
            // the row it ends.
            let window = &c[s..s + cols + 1];
            let cut = window
                .iter()
                .rposition(|&x| x == ' ')
                .filter(|&i| i > 0)
                .map_or(s + cols, |i| s + i + 1);
            out.push(Row { start: s, end: cut });
            s = cut;
        }
        if nl >= c.len() {
            break;
        }
        start = nl + 1;
    }
    out
}

/// The row and column of the character index `pos`.
pub fn locate(rows: &[Row], pos: usize) -> (usize, usize) {
    for (i, r) in rows.iter().enumerate() {
        // A position at the end of a wrapped row belongs to the next row; at
        // the end of a hard line it belongs to that row.
        let last = rows.get(i + 1).is_none_or(|n| n.start != r.end);
        if pos >= r.start && (pos < r.end || (pos == r.end && last)) {
            return (i, pos - r.start);
        }
    }
    let i = rows.len().saturating_sub(1);
    (i, rows.get(i).map_or(0, |r| pos.saturating_sub(r.start)))
}

/// The character index at `col` of `row`, clamped to the row.
pub fn index_at(rows: &[Row], row: usize, col: usize) -> usize {
    let Some(r) = rows.get(row.min(rows.len().saturating_sub(1))) else {
        return 0;
    };
    let wrapped = rows.get(row + 1).is_some_and(|n| n.start == r.end);
    // On a wrapped row the last position is the one before the break.
    let max = if wrapped {
        r.end.saturating_sub(1).max(r.start)
    } else {
        r.end
    };
    (r.start + col).min(max)
}

impl Field {
    pub fn new(text: &str) -> Field {
        let n = char_len(text);
        Field {
            text: text.to_string(),
            caret: n,
            anchor: n,
        }
    }

    pub fn len(&self) -> usize {
        char_len(&self.text)
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The selected range, low to high; empty when nothing is selected.
    pub fn selection(&self) -> (usize, usize) {
        let n = self.len();
        let (a, c) = (self.anchor.min(n), self.caret.min(n));
        (a.min(c), a.max(c))
    }

    pub fn has_selection(&self) -> bool {
        let (lo, hi) = self.selection();
        lo != hi
    }

    pub fn selected(&self) -> &str {
        let (lo, hi) = self.selection();
        &self.text[byte_of(&self.text, lo)..byte_of(&self.text, hi)]
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            caret: self.caret,
        }
    }

    /// Moves the caret; the anchor follows unless `extend`.
    pub fn move_to(&mut self, pos: usize, extend: bool) {
        self.caret = pos.min(self.len());
        if !extend {
            self.anchor = self.caret;
        }
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.len();
    }

    /// Left or right by a character or a word. With a selection and no
    /// `extend`, the caret goes to that edge of the selection.
    pub fn step(&mut self, dir: i32, word: bool, extend: bool) {
        let (lo, hi) = self.selection();
        if !extend && self.has_selection() {
            self.move_to(if dir < 0 { lo } else { hi }, false);
            return;
        }
        let to = match (dir < 0, word) {
            (true, false) => self.caret.saturating_sub(1),
            (false, false) => (self.caret + 1).min(self.len()),
            (true, true) => word_left(&self.text, self.caret),
            (false, true) => word_right(&self.text, self.caret),
        };
        self.move_to(to, extend);
    }

    /// Start or end of the logical line (or of the text with `whole`).
    pub fn home_end(&mut self, end: bool, whole: bool, extend: bool) {
        let to = if whole {
            if end { self.len() } else { 0 }
        } else {
            let (s, e) = line_bounds(&self.text, self.caret);
            if end { e } else { s }
        };
        self.move_to(to, extend);
    }

    fn replace(&mut self, lo: usize, hi: usize, with: &str) {
        let (b0, b1) = (byte_of(&self.text, lo), byte_of(&self.text, hi));
        self.text.replace_range(b0..b1, with);
        self.caret = lo + char_len(with);
        self.anchor = self.caret;
    }

    /// Characters of `s` a field with `rules` takes, in order.
    pub fn filter(s: &str, rules: &Rules) -> String {
        s.chars()
            .filter(|&c| {
                if c == '\n' {
                    rules.multiline
                } else if rules.numeric {
                    c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')
                } else {
                    c == '\t' && rules.multiline || !c.is_control()
                }
            })
            .collect()
    }

    /// Types or pastes `s` over the selection. Returns whether the text
    /// changed. The result never exceeds `rules.max_len`.
    pub fn insert(&mut self, s: &str, rules: &Rules, hist: &mut History, kind: Kind) -> bool {
        let mut add = Field::filter(s, rules);
        let (lo, hi) = self.selection();
        let room = rules.max_len.saturating_sub(self.len() - (hi - lo));
        if char_len(&add) > room {
            add = add.chars().take(room).collect();
        }
        if add.is_empty() && lo == hi {
            return false;
        }
        hist.record(self.snapshot(), kind);
        self.replace(lo, hi, &add);
        true
    }

    /// Backspace or delete: the selection, else a character or a word.
    pub fn delete(&mut self, forward: bool, word: bool, hist: &mut History) -> bool {
        let (mut lo, mut hi) = self.selection();
        if lo == hi {
            if forward {
                hi = if word {
                    word_right(&self.text, lo)
                } else {
                    (lo + 1).min(self.len())
                };
            } else {
                lo = if word {
                    word_left(&self.text, hi)
                } else {
                    lo.saturating_sub(1)
                };
            }
        }
        if lo == hi {
            return false;
        }
        hist.record(self.snapshot(), Kind::Deleting);
        self.replace(lo, hi, "");
        true
    }

    /// Removes the selection and returns it (cut). `None` when empty.
    pub fn cut(&mut self, hist: &mut History) -> Option<String> {
        if !self.has_selection() {
            return None;
        }
        let s = self.selected().to_string();
        let (lo, hi) = self.selection();
        hist.record(self.snapshot(), Kind::Other);
        self.replace(lo, hi, "");
        Some(s)
    }

    pub fn undo(&mut self, hist: &mut History) -> bool {
        let Some(prev) = hist.undo.pop() else {
            return false;
        };
        hist.redo.push(self.snapshot());
        self.text = prev.text;
        self.caret = prev.caret.min(self.len());
        self.anchor = self.caret;
        hist.last = Kind::None;
        true
    }

    pub fn redo(&mut self, hist: &mut History) -> bool {
        let Some(next) = hist.redo.pop() else {
            return false;
        };
        hist.undo.push(self.snapshot());
        self.text = next.text;
        self.caret = next.caret.min(self.len());
        self.anchor = self.caret;
        hist.last = Kind::None;
        true
    }

    /// Moves the caret up or down one displayed row, keeping the column
    /// (`want` is the column to aim for; `None` uses the current one).
    pub fn vertical(&mut self, dir: i32, rows: &[Row], want: Option<usize>, extend: bool) -> usize {
        let (r, c) = locate(rows, self.caret);
        let col = want.unwrap_or(c);
        let to = if dir < 0 {
            if r == 0 {
                0
            } else {
                index_at(rows, r - 1, col)
            }
        } else if r + 1 >= rows.len() {
            self.len()
        } else {
            index_at(rows, r + 1, col)
        };
        self.move_to(to, extend);
        col
    }
}

/// How one character of rich text is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub code: bool,
    /// `0xRRGGBBAA`, or 0 for the node's own text color.
    pub color: u32,
    /// 1 + the index into the link list, or 0 for none.
    pub link: u32,
}

/// Rich text parsed from the small markup form: styled characters and the
/// link targets they point at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rich {
    pub text: String,
    /// One style per character of `text`.
    pub styles: Vec<Style>,
    pub links: Vec<String>,
}

/// Parses the markup form: `**bold**`, `` `code` ``, `[label](target)` for a
/// link, `{#RRGGBB text}` for color, and a backslash before any of
/// `* \` [ ] ( ) { } #` to write it literally. Markup that does not close is
/// shown as typed. Inside code nothing but the closing backtick is special.
pub fn parse_markup(src: &str) -> Rich {
    let c: Vec<char> = src.chars().collect();
    let mut out = Rich::default();
    let mut bold = false;
    let mut code = false;
    let mut colors: Vec<u32> = vec![];
    let mut link = 0u32;
    let mut i = 0;
    let push = |out: &mut Rich, ch: char, st: Style| {
        out.text.push(ch);
        out.styles.push(st);
    };
    while i < c.len() {
        let st = Style {
            bold,
            code,
            color: colors.last().copied().unwrap_or(0),
            link,
        };
        let ch = c[i];
        if code {
            if ch == '`' {
                code = false;
            } else {
                push(&mut out, ch, st);
            }
            i += 1;
            continue;
        }
        match ch {
            '\\' if i + 1 < c.len() => {
                push(&mut out, c[i + 1], st);
                i += 2;
            }
            '*' if c.get(i + 1) == Some(&'*') => {
                bold = !bold;
                i += 2;
            }
            '`' if c[i + 1..].contains(&'`') => {
                code = true;
                i += 1;
            }
            '[' if link == 0 => {
                // [label](target): find the closing "](" and ")".
                let close =
                    (i + 1..c.len().saturating_sub(1)).find(|&k| c[k] == ']' && c[k + 1] == '(');
                let end = close.and_then(|k| (k + 2..c.len()).find(|&e| c[e] == ')'));
                if let (Some(k), Some(e)) = (close, end) {
                    out.links.push(c[k + 2..e].iter().collect());
                    link = out.links.len() as u32;
                    // Parse the label in place by continuing; remember where
                    // the link ends so the target is skipped.
                    let label = parse_markup_inner(
                        &c[i + 1..k],
                        bold,
                        colors.last().copied().unwrap_or(0),
                        link,
                    );
                    out.text.push_str(&label.text);
                    out.styles.extend(label.styles);
                    link = 0;
                    i = e + 1;
                } else {
                    push(&mut out, ch, st);
                    i += 1;
                }
            }
            '{' if c.get(i + 1) == Some(&'#') && i + 8 < c.len() && c[i + 8] == ' ' => {
                let hex: String = c[i + 2..i + 8].iter().collect();
                if let Ok(v) = u32::from_str_radix(&hex, 16)
                    && c[i + 9..].contains(&'}')
                {
                    colors.push(v << 8 | 0xFF);
                    i += 9;
                } else {
                    push(&mut out, ch, st);
                    i += 1;
                }
            }
            '}' if !colors.is_empty() => {
                colors.pop();
                i += 1;
            }
            _ => {
                push(&mut out, ch, st);
                i += 1;
            }
        }
    }
    out
}

/// A link's label: the same markup, every character linked.
fn parse_markup_inner(src: &[char], bold: bool, color: u32, link: u32) -> Rich {
    let text: String = src.iter().collect();
    let mut r = parse_markup(&text);
    for st in &mut r.styles {
        st.bold |= bold;
        if st.color == 0 {
            st.color = color;
        }
        st.link = link;
    }
    r.links.clear();
    r
}

/// What a masked (password) field shows for `text`.
pub fn mask(text: &str) -> String {
    "*".repeat(char_len(text))
}

/// A number field's text, parsed. `None` for text that is not a number.
pub fn parse_number(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() || t.ends_with(['e', 'E']) {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// `v` limited to `[min, max]` (when `max > min`) and snapped to `step` from
/// `min` (when `step > 0`).
pub fn snap(v: f64, min: f64, max: f64, step: f64) -> f64 {
    let mut v = v;
    if step > 0.0 {
        v = min + ((v - min) / step).round() * step;
    }
    if max > min {
        v = v.clamp(min, max);
    }
    v
}

/// Shortest text for `v`: integers without a point, decimals trimmed to
/// `decimals` places (so repeated steps do not show float noise).
pub fn format_number(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        Rules::default()
    }

    #[test]
    fn words_are_runs_of_letters_or_runs_of_punctuation() {
        let t = "foo, bar_baz  qux";
        assert_eq!(word_right(t, 0), 3);
        assert_eq!(word_right(t, 3), 4, "the comma is its own word");
        assert_eq!(
            word_right(t, 4),
            12,
            "spaces are skipped, bar_baz is one word"
        );
        assert_eq!(word_left(t, 17), 14);
        assert_eq!(word_left(t, 14), 5);
        assert_eq!(word_left(t, 3), 0);
        assert_eq!(word_left(t, 0), 0);
        assert_eq!(word_right(t, 17), 17);
    }

    #[test]
    fn line_bounds_are_between_newlines() {
        let t = "ab\ncde\n\nf";
        assert_eq!(line_bounds(t, 0), (0, 2));
        assert_eq!(line_bounds(t, 2), (0, 2), "at the end of a line");
        assert_eq!(line_bounds(t, 3), (3, 6));
        assert_eq!(line_bounds(t, 7), (7, 7), "an empty line");
        assert_eq!(line_bounds(t, 9), (8, 9));
    }

    #[test]
    fn rows_wrap_at_spaces_then_inside_long_words_and_keep_hard_breaks() {
        let r = rows("hello big world", 8, true);
        let s: Vec<_> = r.iter().map(|r| (r.start, r.end)).collect();
        assert_eq!(s, vec![(0, 6), (6, 10), (10, 15)]);
        let r = rows("abcdefghij", 4, true);
        assert_eq!(r.len(), 3);
        assert_eq!((r[0].end, r[1].end, r[2].end), (4, 8, 10));
        let r = rows("a\n\nb", 10, true);
        assert_eq!(r.len(), 3);
        assert_eq!(rows("", 5, true), vec![Row { start: 0, end: 0 }]);
        assert_eq!(rows("abc", 1, false).len(), 1, "no wrap, one row");
    }

    #[test]
    fn a_position_maps_to_a_row_and_column_and_back() {
        let r = rows("hello big world", 8, true);
        assert_eq!(locate(&r, 0), (0, 0));
        assert_eq!(locate(&r, 6), (1, 0), "the break belongs to the next row");
        assert_eq!(locate(&r, 15), (2, 5), "the very end");
        assert_eq!(index_at(&r, 1, 99), 9, "clamped before the break");
        assert_eq!(index_at(&r, 2, 99), 15, "the last row reaches the end");
        let hard = rows("ab\ncd", 9, true);
        assert_eq!(locate(&hard, 2), (0, 2), "end of a hard line stays on it");
        assert_eq!(locate(&hard, 3), (1, 0));
    }

    #[test]
    fn typing_replaces_the_selection_and_respects_the_limit() {
        let mut f = Field::new("hello");
        let mut h = History::default();
        f.move_to(1, false);
        f.move_to(4, true);
        assert_eq!(f.selected(), "ell");
        assert!(f.insert("X", &rules(), &mut h, Kind::Typing));
        assert_eq!((f.text.as_str(), f.caret, f.anchor), ("hXo", 2, 2));
        let small = Rules {
            max_len: 4,
            ..rules()
        };
        assert!(f.insert("abc", &small, &mut h, Kind::Typing));
        assert_eq!(f.text, "hXao");
        assert_eq!(f.len(), 4, "capped");
        assert!(!f.insert("z", &small, &mut h, Kind::Typing), "full");
    }

    #[test]
    fn control_characters_are_dropped_and_newlines_need_a_text_area() {
        let one = rules();
        let area = Rules {
            multiline: true,
            ..rules()
        };
        assert_eq!(Field::filter("a\nb\tc\u{7}", &one), "abc");
        assert_eq!(Field::filter("a\nb\tc\u{7}", &area), "a\nb\tc");
        let num = Rules {
            numeric: true,
            ..rules()
        };
        assert_eq!(Field::filter("-1.5e3 apples", &num), "-1.5e3e");
    }

    #[test]
    fn delete_removes_a_character_a_word_or_the_selection() {
        let mut h = History::default();
        let mut f = Field::new("one two three");
        f.delete(false, false, &mut h);
        assert_eq!(f.text, "one two thre");
        f.delete(false, true, &mut h);
        assert_eq!(f.text, "one two ");
        f.move_to(0, false);
        f.delete(true, true, &mut h);
        assert_eq!(f.text, " two ");
        f.select_all();
        f.delete(true, false, &mut h);
        assert_eq!((f.text.as_str(), f.caret), ("", 0));
        assert!(!f.delete(false, false, &mut h), "nothing to delete");
    }

    #[test]
    fn arrows_collapse_a_selection_and_extend_with_shift() {
        let mut f = Field::new("abcdef");
        f.move_to(2, false);
        f.step(1, false, true);
        f.step(1, false, true);
        assert_eq!(f.selection(), (2, 4));
        f.step(-1, false, false);
        assert_eq!((f.caret, f.anchor), (2, 2), "left goes to the start");
        f.move_to(1, false);
        f.step(1, true, true);
        assert_eq!(f.selection(), (1, 6));
        f.home_end(false, true, true);
        assert_eq!(f.selection(), (0, 1));
    }

    #[test]
    fn typing_is_one_undo_step_and_undo_redo_round_trip() {
        let mut f = Field::new("");
        let mut h = History::default();
        for c in "abc".chars() {
            f.insert(&c.to_string(), &rules(), &mut h, Kind::Typing);
        }
        f.delete(false, false, &mut h);
        f.delete(false, false, &mut h);
        assert_eq!(f.text, "a");
        assert_eq!(h.undo.len(), 2, "one step of typing, one of deleting");
        assert!(f.undo(&mut h));
        assert_eq!(f.text, "abc");
        assert!(f.undo(&mut h));
        assert_eq!(f.text, "");
        assert!(!f.undo(&mut h));
        assert!(f.redo(&mut h));
        assert_eq!(f.text, "abc");
        assert!(f.redo(&mut h));
        assert_eq!(f.text, "a");
        assert!(!f.redo(&mut h));
        // A new edit clears redo.
        f.undo(&mut h);
        f.insert("z", &rules(), &mut h, Kind::Typing);
        assert!(h.redo.is_empty());
    }

    #[test]
    fn history_is_bounded() {
        let mut f = Field::new("");
        let mut h = History::default();
        for i in 0..20 {
            f.insert(&i.to_string(), &rules(), &mut h, Kind::Other);
        }
        assert_eq!(h.undo.len(), HISTORY);
    }

    #[test]
    fn cut_returns_the_selection_and_is_undoable() {
        let mut f = Field::new("hello world");
        let mut h = History::default();
        f.move_to(0, false);
        f.move_to(5, true);
        assert_eq!(f.cut(&mut h).as_deref(), Some("hello"));
        assert_eq!(f.text, " world");
        assert!(f.undo(&mut h));
        assert_eq!(f.text, "hello world");
        assert_eq!(f.cut(&mut h), None, "nothing selected after undo");
    }

    #[test]
    fn vertical_motion_keeps_the_column_across_short_rows() {
        let t = "abcdef\nab\nabcdef";
        let r = rows(t, 20, true);
        let mut f = Field::new(t);
        f.move_to(5, false);
        let col = f.vertical(1, &r, None, false);
        assert_eq!((col, f.caret), (5, 9), "the short row ends at 2 columns");
        f.vertical(1, &r, Some(col), false);
        assert_eq!(f.caret, 15, "the wanted column returns on the next row");
        f.vertical(1, &r, Some(col), false);
        assert_eq!(f.caret, f.len(), "down on the last row goes to the end");
        f.move_to(2, false);
        f.vertical(-1, &r, None, false);
        assert_eq!(f.caret, 0, "up on the first row goes to the start");
    }

    #[test]
    fn numbers_parse_snap_and_print_without_noise() {
        assert_eq!(parse_number(" 12.5 "), Some(12.5));
        assert_eq!(parse_number("1e3"), Some(1000.0));
        assert_eq!(parse_number("-"), None);
        assert_eq!(parse_number("1e"), None);
        assert_eq!(parse_number("NaN"), None);
        assert_eq!(snap(7.3, 0.0, 10.0, 0.5), 7.5);
        assert_eq!(snap(99.0, 0.0, 10.0, 0.0), 10.0);
        assert_eq!(snap(-5.0, 0.0, 10.0, 1.0), 0.0);
        assert_eq!(snap(123.0, 0.0, 0.0, 0.0), 123.0, "no range: unclamped");
        assert_eq!(format_number(0.1 + 0.2, 2), "0.3");
        assert_eq!(format_number(5.0, 2), "5");
        assert_eq!(format_number(2.50, 2), "2.5");
        assert_eq!(mask("abc"), "***");
    }

    #[test]
    fn markup_gives_styled_characters() {
        let r = parse_markup("a **b** `c*d` {#FF0000 e} [f](go:home) \\*g");
        assert_eq!(r.text, "a b c*d e f *g");
        let st = |k: usize| r.styles[k];
        assert!(!st(0).bold && st(2).bold && !st(3).bold);
        assert!(
            st(4).code && st(6).code && !st(7).code,
            "code protects its asterisk"
        );
        assert_eq!(st(8).color, 0xFF0000FF);
        assert_eq!(st(9).color, 0);
        assert_eq!(r.links, vec!["go:home".to_string()]);
        assert_eq!(st(10).link, 1);
        assert_eq!(st(11).link, 0);
        assert_eq!(r.styles.len(), r.text.chars().count());
    }

    #[test]
    fn markup_that_does_not_close_is_shown_as_typed() {
        for src in ["[a](b", "{#12345 x}", "`open", "a ] b", "{#GGGGGG x}"] {
            let r = parse_markup(src);
            assert_eq!(r.text.replace('`', ""), src.replace('`', ""), "{src}");
        }
        assert_eq!(parse_markup("").text, "");
    }

    #[test]
    fn rows_break_where_wrap_text_would_so_layout_and_drawing_agree() {
        for text in [
            "the quick brown fox jumps over the lazy dog",
            "a  b   c",
            "supercalifragilisticexpialidocious word",
            "one\ntwo three four five\n\nsix",
        ] {
            for cols in [3usize, 5, 9, 12, 40] {
                let width = (cols as f32 - 1.0) * 6.0 + 5.0;
                let lines = crate::layout::wrap_text(text, 1.0, width);
                let r = rows(text, cols, true);
                assert_eq!(lines.split('\n').count(), r.len(), "{text:?} at {cols}");
            }
        }
    }
}
