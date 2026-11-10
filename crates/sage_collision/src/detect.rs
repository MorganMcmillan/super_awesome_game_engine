//! AABB overlap detection producing canonical contacts.
//!
//! Input order never matters: colliders are sorted by `StableId` first, the
//! broadphase is a sweep over a stable sort, and output is sorted by `(a, b)`.
//! Flecs iteration order (which changes as entities change archetype) can be
//! fed in directly.

use parry2d::math::{Pose, Vector};
use parry2d::query;
use parry2d::shape::Cuboid;
use sage_core::StableId;

use crate::table::Contact;

/// An axis-aligned box collider: center and half extents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxCollider {
    pub id: StableId,
    pub center: [f32; 2],
    pub half: [f32; 2],
}

impl BoxCollider {
    fn min_x(&self) -> f32 {
        self.center[0] - self.half[0]
    }
    fn max_x(&self) -> f32 {
        self.center[0] + self.half[0]
    }
}

/// Finds every strictly overlapping pair (touching edges do not count).
/// Returns canonical contacts: `a < b`, sorted by `(a, b)`.
///
/// Panics if two colliders share a `StableId`.
pub fn detect(tick: u64, colliders: &[BoxCollider]) -> Vec<Contact> {
    let mut sorted: Vec<BoxCollider> = colliders.to_vec();
    sorted.sort_by_key(|c| c.id);
    for w in sorted.windows(2) {
        assert!(w[0].id != w[1].id, "duplicate StableId {:?}", w[0].id);
    }

    // Broadphase: sweep and prune on x. Sort by (min_x, id): total, stable,
    // and independent of input order. total_cmp keeps the order defined even
    // for NaN, though a NaN collider never overlaps anything.
    let mut order: Vec<usize> = (0..sorted.len()).collect();
    order.sort_by(|&i, &j| {
        sorted[i]
            .min_x()
            .total_cmp(&sorted[j].min_x())
            .then(sorted[i].id.cmp(&sorted[j].id))
    });

    let mut out = Vec::new();
    let mut active: Vec<usize> = Vec::new();
    for &i in &order {
        let ci = sorted[i];
        active.retain(|&j| sorted[j].max_x() > ci.min_x());
        for &j in &active {
            let (a, b) = if sorted[j].id < ci.id {
                (sorted[j], ci)
            } else {
                (ci, sorted[j])
            };
            if let Some(c) = narrow(tick, &a, &b) {
                out.push(c);
            }
        }
        active.push(i);
    }
    out.sort_by_key(|c| c.pair());
    out
}

/// Narrow phase through parry. `a.id < b.id`.
fn narrow(tick: u64, a: &BoxCollider, b: &BoxCollider) -> Option<Contact> {
    let sa = Cuboid::new(Vector::new(a.half[0], a.half[1]));
    let sb = Cuboid::new(Vector::new(b.half[0], b.half[1]));
    let pa = Pose::translation(a.center[0], a.center[1]);
    let pb = Pose::translation(b.center[0], b.center[1]);
    let hit =
        query::contact(&pa, &sa, &pb, &sb, 0.0).expect("cuboid-cuboid contact is supported")?;
    if hit.dist >= 0.0 {
        return None; // touching or separated
    }
    Some(Contact {
        tick,
        a: a.id,
        b: b.id,
        normal: [hit.normal1.x, hit.normal1.y],
        depth: -hit.dist,
    })
}

/// Reference implementation for tests: all pairs, plain interval overlap,
/// no parry. Returns canonical pairs only.
pub fn brute_force_pairs(colliders: &[BoxCollider]) -> Vec<(StableId, StableId)> {
    let mut out = Vec::new();
    for (i, p) in colliders.iter().enumerate() {
        for q in &colliders[i + 1..] {
            let overlap = (0..2).all(|k| (p.center[k] - q.center[k]).abs() < p.half[k] + q.half[k]);
            if overlap {
                out.push(if p.id < q.id {
                    (p.id, q.id)
                } else {
                    (q.id, p.id)
                });
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(id: u64, x: f32, y: f32, hw: f32, hh: f32) -> BoxCollider {
        BoxCollider {
            id: StableId(id),
            center: [x, y],
            half: [hw, hh],
        }
    }

    #[test]
    fn overlap_normal_and_depth() {
        let c = detect(3, &[bx(2, 1.5, 0.0, 1.0, 1.0), bx(1, 0.0, 0.0, 1.0, 1.0)]);
        assert_eq!(c.len(), 1);
        let c = c[0];
        assert_eq!((c.tick, c.a, c.b), (3, StableId(1), StableId(2)));
        assert!((c.normal[0] - 1.0).abs() < 1e-6 && c.normal[1].abs() < 1e-6);
        assert!((c.depth - 0.5).abs() < 1e-6);
    }

    #[test]
    fn touching_is_not_contact() {
        assert!(detect(0, &[bx(1, 0.0, 0.0, 1.0, 1.0), bx(2, 2.0, 0.0, 1.0, 1.0)]).is_empty());
    }

    #[test]
    fn input_order_does_not_matter() {
        let a = vec![
            bx(5, 0.0, 0.0, 1.0, 1.0),
            bx(3, 0.5, 0.5, 1.0, 1.0),
            bx(9, 1.0, 0.0, 1.0, 1.0),
        ];
        let mut b = a.clone();
        b.reverse();
        assert_eq!(detect(0, &a), detect(0, &b));
    }
}
