//! Spike S8: detection matches a brute-force reference on random layouts, and
//! output is identical across runs and across debug/release builds.

use sage_collision::{BoxCollider, brute_force_pairs, detect};
use sage_core::{Rng, StableId, StateHasher};

/// Random layout. Coordinates are multiples of 1/16 so exact edge contact
/// (touching but not overlapping) happens often and is tested.
fn layout(rng: &mut Rng, n: usize) -> Vec<BoxCollider> {
    let q = |rng: &mut Rng, lo: f32, hi: f32| (rng.range_f32(lo, hi) * 16.0).floor() / 16.0;
    let mut ids: Vec<u64> = (0..n as u64).map(|i| i * 7 + 3).collect();
    // Shuffle so input order is not ID order.
    for i in (1..ids.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        ids.swap(i, j);
    }
    ids.into_iter()
        .map(|id| BoxCollider {
            id: StableId(id),
            center: [q(rng, 0.0, 40.0), q(rng, 0.0, 40.0)],
            half: [q(rng, 0.25, 3.0), q(rng, 0.25, 3.0)],
        })
        .collect()
}

#[test]
fn matches_brute_force_on_random_layouts() {
    let mut rng = Rng::from_seed(8);
    let mut total_pairs = 0;
    for round in 0..200 {
        let n = 1 + (round % 60);
        let boxes = layout(&mut rng, n);
        let got: Vec<_> = detect(round as u64, &boxes)
            .iter()
            .map(|c| c.pair())
            .collect();
        let want = brute_force_pairs(&boxes);
        assert_eq!(got, want, "round {round}");
        total_pairs += want.len();
    }
    assert!(
        total_pairs > 1000,
        "layouts too sparse to be a test: {total_pairs}"
    );
}

fn output_hash() -> u64 {
    let mut rng = Rng::from_seed(88);
    let mut h = StateHasher::new();
    for round in 0..50 {
        for c in detect(round, &layout(&mut rng, 50)) {
            h.u64(c.a.0)
                .u64(c.b.0)
                .f32(c.normal[0])
                .f32(c.normal[1])
                .f32(c.depth);
        }
    }
    h.finish()
}

#[test]
fn identical_across_runs() {
    assert_eq!(output_hash(), output_hash());
}

/// Golden value: the same test runs under `cargo test` and
/// `cargo test --release`, so a mismatch in either build fails here.
/// If parry, the RNG or the layout generator changes intentionally, update it.
#[test]
fn identical_across_builds() {
    assert_eq!(output_hash(), GOLDEN, "got {:#018x}", output_hash());
}

const GOLDEN: u64 = 0xffe9_e4ed_e2ce_31d0;
