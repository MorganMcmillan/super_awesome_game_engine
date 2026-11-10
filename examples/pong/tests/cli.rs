//! A1 across processes: two separate `pong replay` processes produce the
//! same per-tick hashes. In-process tests cannot catch nondeterminism that
//! differs per process (Python hash seeds, address-dependent ordering).

use std::path::PathBuf;
use std::process::Command;

const RECORDING: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/seed7_left_bot.txt");

fn replay(out: &PathBuf) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_pong"))
        .args(["replay", RECORDING, "--hashes"])
        .arg(out)
        .output()
        .expect("run pong");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

#[test]
fn a1_replay_matches_across_processes() {
    let dir = std::env::temp_dir().join(format!("pong-a1-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (h1, h2) = (dir.join("h1.txt"), dir.join("h2.txt"));
    let s1 = replay(&h1);
    let s2 = replay(&h2);
    assert_eq!(s1, s2);
    let a = std::fs::read_to_string(&h1).unwrap();
    let b = std::fs::read_to_string(&h2).unwrap();
    assert_eq!(a.lines().count(), 1281, "initial + 1280 ticks");
    assert!(a == b, "per-tick hashes differ between processes");
    std::fs::remove_dir_all(&dir).ok();
}

/// The committed recording keeps replaying to the same outcome. If game
/// rules change on purpose, re-record with `pong headless --seed 7 --record`.
#[test]
fn committed_recording_reaches_game_over() {
    let dir = std::env::temp_dir().join(format!("pong-rec-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = replay(&dir.join("h.txt"));
    assert!(s.contains("state=GameOver score=5:0 winner=left"), "{s}");
    // Same final hash on Windows and Linux, debug and release (verified
    // 2026-11-09). A failure on one platform only is a cross-platform
    // determinism bug; a failure everywhere means the rules or the hash changed.
    assert!(s.contains("hash=0x7f2402b63b589d67"), "{s}");
    std::fs::remove_dir_all(&dir).ok();
}

/// A2's windowed half. Needs a display, so it is ignored by default; CI runs
/// it explicitly (under xvfb on Linux).
#[test]
#[ignore = "needs a display"]
fn a2_window_opens_and_draws_court() {
    let o = Command::new(env!("CARGO_BIN_EXE_pong"))
        .args(["smoke", "--frames", "60"])
        .output()
        .expect("run pong");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success() && out.contains("smoke: ok"),
        "{out}{}",
        String::from_utf8_lossy(&o.stderr)
    );
}
