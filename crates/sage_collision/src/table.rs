//! Contacts as data (design doc 8.2).
//!
//! ```text
//! Contact(tick, a, b, normal, depth)      -- a < b, sorted by (a, b)
//! begin = Contact(now) EXCEPT Contact(prev)
//! end   = Contact(prev) EXCEPT Contact(now)
//! stay  = Contact(now) INTERSECT Contact(prev)
//! ```
//! Set operations compare pairs `(a, b)` only; normal and depth are payload.

use sage_core::StableId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub tick: u64,
    /// Always `a < b`.
    pub a: StableId,
    pub b: StableId,
    /// Unit normal pointing from `a` towards `b`.
    pub normal: [f32; 2],
    /// Penetration depth (positive when overlapping).
    pub depth: f32,
}

impl Contact {
    pub fn pair(&self) -> (StableId, StableId) {
        (self.a, self.b)
    }

    /// The same contact seen from `id`'s side: `(other, normal from id to other)`.
    /// Returns `None` if `id` is not in the pair. This is the "both
    /// orientations" view that rules match against.
    pub fn seen_from(&self, id: StableId) -> Option<(StableId, [f32; 2])> {
        if id == self.a {
            Some((self.b, self.normal))
        } else if id == self.b {
            Some((self.a, [-self.normal[0], -self.normal[1]]))
        } else {
            None
        }
    }
}

/// The current and previous tick's contact sets. The previous set is part of
/// the simulation state: it must be hashed and snapshotted, or the first tick
/// after a rewind reports every contact as a new begin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContactTable {
    prev: Vec<Contact>,
    now: Vec<Contact>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transitions {
    pub begin: Vec<Contact>,
    pub stay: Vec<Contact>,
    /// Rows from the previous tick that no longer exist.
    pub end: Vec<Contact>,
}

impl ContactTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the current set with `contacts` (moving the old current set
    /// to `prev`). `contacts` must already be canonical: `a < b`, sorted by
    /// `(a, b)`, no duplicates. Panics otherwise, because a non-canonical
    /// table is a determinism bug.
    pub fn advance(&mut self, contacts: Vec<Contact>) {
        assert_canonical(&contacts);
        self.prev = std::mem::replace(&mut self.now, contacts);
    }

    pub fn now(&self) -> &[Contact] {
        &self.now
    }

    pub fn prev(&self) -> &[Contact] {
        &self.prev
    }

    /// Begin, stay and end, each in canonical `(a, b)` order. A sorted merge,
    /// so the output order never depends on anything but the pair IDs.
    pub fn transitions(&self) -> Transitions {
        let mut t = Transitions::default();
        let (mut i, mut j) = (0, 0);
        while i < self.now.len() || j < self.prev.len() {
            match (self.now.get(i), self.prev.get(j)) {
                (Some(n), Some(p)) if n.pair() == p.pair() => {
                    t.stay.push(*n);
                    i += 1;
                    j += 1;
                }
                (Some(n), Some(p)) if n.pair() < p.pair() => {
                    t.begin.push(*n);
                    i += 1;
                }
                (Some(_), Some(p)) => {
                    t.end.push(*p);
                    j += 1;
                }
                (Some(n), None) => {
                    t.begin.push(*n);
                    i += 1;
                }
                (None, Some(p)) => {
                    t.end.push(*p);
                    j += 1;
                }
                (None, None) => unreachable!(),
            }
        }
        t
    }

    /// Feeds both sets into a world-state hash.
    pub fn hash_into(&self, h: &mut sage_core::StateHasher) {
        for (name, set) in [("contacts.prev", &self.prev), ("contacts.now", &self.now)] {
            h.tag(name).u64(set.len() as u64);
            for c in set {
                h.u64(c.tick)
                    .u64(c.a.0)
                    .u64(c.b.0)
                    .f32(c.normal[0])
                    .f32(c.normal[1])
                    .f32(c.depth);
            }
        }
    }
}

pub fn assert_canonical(contacts: &[Contact]) {
    for c in contacts {
        assert!(c.a < c.b, "contact pair not canonical: {:?}", c.pair());
    }
    for w in contacts.windows(2) {
        assert!(
            w[0].pair() < w[1].pair(),
            "contacts not sorted or duplicated: {:?} then {:?}",
            w[0].pair(),
            w[1].pair()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(tick: u64, a: u64, b: u64) -> Contact {
        Contact {
            tick,
            a: StableId(a),
            b: StableId(b),
            normal: [1.0, 0.0],
            depth: 0.1,
        }
    }

    fn pairs(v: &[Contact]) -> Vec<(u64, u64)> {
        v.iter().map(|c| (c.a.0, c.b.0)).collect()
    }

    #[test]
    fn begin_stay_end() {
        let mut t = ContactTable::new();
        t.advance(vec![c(1, 1, 2), c(1, 1, 3)]);
        let tr = t.transitions();
        assert_eq!(pairs(&tr.begin), vec![(1, 2), (1, 3)]);
        assert!(tr.stay.is_empty() && tr.end.is_empty());

        t.advance(vec![c(2, 1, 3), c(2, 2, 4)]);
        let tr = t.transitions();
        assert_eq!(pairs(&tr.begin), vec![(2, 4)]);
        assert_eq!(pairs(&tr.stay), vec![(1, 3)]);
        assert_eq!(pairs(&tr.end), vec![(1, 2)]);

        t.advance(vec![]);
        let tr = t.transitions();
        assert_eq!(pairs(&tr.end), vec![(1, 3), (2, 4)]);
    }

    /// The exercise-8 scenario as a trace: ball (1) touches paddle (2) for
    /// three ticks; on the third it also touches the top wall (3).
    /// Begin fires once per contact, not once per tick.
    #[test]
    fn bounce_begins_once_per_contact() {
        let mut t = ContactTable::new();
        let mut begins = vec![];
        for (_tick, set) in [
            (1, vec![c(1, 1, 2)]),
            (2, vec![c(2, 1, 2)]),
            (3, vec![c(3, 1, 2), c(3, 1, 3)]),
            (4, vec![]),
        ] {
            t.advance(set);
            begins.extend(pairs(&t.transitions().begin));
        }
        assert_eq!(begins, vec![(1, 2), (1, 3)]);
    }

    #[test]
    fn seen_from_both_sides() {
        let k = c(0, 5, 9);
        assert_eq!(k.seen_from(StableId(5)), Some((StableId(9), [1.0, 0.0])));
        assert_eq!(k.seen_from(StableId(9)), Some((StableId(5), [-1.0, -0.0])));
        assert_eq!(k.seen_from(StableId(1)), None);
    }

    #[test]
    #[should_panic(expected = "not canonical")]
    fn rejects_unordered_pair() {
        ContactTable::new().advance(vec![c(0, 2, 1)]);
    }
}
