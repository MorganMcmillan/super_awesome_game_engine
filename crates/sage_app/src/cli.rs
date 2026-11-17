//! The command line of an example: `command [FILE] [--flag VALUE] [--switch]`.
//!
//! Every example is script-checkable: a command either succeeds or prints
//! `error: ...` and exits non-zero. The pieces here are the ones each `main`
//! had its own copy of.

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::str::FromStr;

use sage_core::{InputRecording, Simulation, run_recording};

/// Parsed arguments. A flag is declared as taking a value or not, so a
/// misspelled one is an error instead of being swallowed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// The first argument (`play`, `render`...).
    pub cmd: String,
    /// The first argument that is not a flag (a recording to replay).
    pub file: Option<String>,
    values: BTreeMap<String, Vec<String>>,
    switches: Vec<String>,
}

impl Args {
    /// Parses the process's arguments. `valued` flags take the next argument;
    /// `switches` take none. Without a command, `default_cmd` is used.
    pub fn from_env(default_cmd: &str, valued: &[&str], switches: &[&str]) -> Result<Args, String> {
        Self::parse(std::env::args().skip(1), default_cmd, valued, switches)
    }

    pub fn parse(
        args: impl IntoIterator<Item = String>,
        default_cmd: &str,
        valued: &[&str],
        switches: &[&str],
    ) -> Result<Args, String> {
        let mut it = args.into_iter();
        let mut a = Args {
            cmd: it.next().unwrap_or_else(|| default_cmd.to_string()),
            ..Args::default()
        };
        while let Some(arg) = it.next() {
            if valued.contains(&arg.as_str()) {
                let value = it.next().ok_or(format!("{arg} needs a value"))?;
                a.values.entry(arg).or_default().push(value);
            } else if switches.contains(&arg.as_str()) {
                a.switches.push(arg);
            } else if !arg.starts_with("--") && a.file.is_none() {
                a.file = Some(arg);
            } else {
                return Err(format!("unknown argument '{arg}'"));
            }
        }
        Ok(a)
    }

    /// Whether a switch was given.
    pub fn has(&self, switch: &str) -> bool {
        self.switches.iter().any(|s| s == switch)
    }

    /// The (last) value of a flag.
    pub fn value(&self, flag: &str) -> Option<&str> {
        self.values.get(flag)?.last().map(String::as_str)
    }

    /// Every value of a flag that may be repeated, in order.
    pub fn values(&self, flag: &str) -> &[String] {
        self.values.get(flag).map_or(&[], Vec::as_slice)
    }

    /// A flag's value parsed (a number, say), or `default` without the flag.
    pub fn get<T: FromStr>(&self, flag: &str, default: T) -> Result<T, String> {
        match self.value(flag) {
            Some(v) => v.parse().map_err(|_| format!("bad {flag} '{v}'")),
            None => Ok(default),
        }
    }
}

/// Runs a command and turns its result into the process's exit code,
/// printing the error.
pub fn exit(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Reads a text file; the error names the path.
pub fn read_text(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

/// Writes a file; the error names the path.
pub fn write_file(path: &str, bytes: impl AsRef<[u8]>) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{path}: {e}"))
}

/// The `replay FILE [--hashes OUT]` command of a game: reads a recording,
/// makes the simulation for its seed with `make`, replays every tick and,
/// when asked, writes the per-tick hashes (`RunTrace::to_text`). Returns the
/// simulation as the replay left it.
pub fn replay<S: Simulation>(
    recording: Option<&str>,
    hashes: Option<&str>,
    make: impl FnOnce(u64) -> Result<S, String>,
) -> Result<S, String> {
    let path = recording.ok_or("replay needs a recording file")?;
    let rec = InputRecording::from_text(&read_text(path)?).map_err(|e| e.to_string())?;
    let mut sim = make(rec.seed)?;
    let trace = run_recording(&mut sim, &rec);
    if let Some(out) = hashes {
        write_file(out, trace.to_text())?;
    }
    Ok(sim)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        Args::parse(
            args.iter().map(|s| s.to_string()),
            "play",
            &["--seed", "--click"],
            &["--mute"],
        )
    }

    #[test]
    fn a_command_a_file_values_and_switches() {
        let a = parse(&["replay", "run.txt", "--seed", "7", "--mute"]).unwrap();
        assert_eq!(a.cmd, "replay");
        assert_eq!(a.file.as_deref(), Some("run.txt"));
        assert_eq!(a.get("--seed", 1u64), Ok(7));
        assert!(a.has("--mute"));
        assert_eq!(a.get("--frames", 120u64), Ok(120));
    }

    #[test]
    fn no_arguments_is_the_default_command() {
        let a = parse(&[]).unwrap();
        assert_eq!(a.cmd, "play");
        assert!(!a.has("--mute"));
        assert_eq!(a.value("--seed"), None);
    }

    #[test]
    fn a_repeated_flag_keeps_every_value_in_order() {
        let a = parse(&["render", "--click", "a", "--click", "b"]).unwrap();
        assert_eq!(a.values("--click"), ["a", "b"]);
        assert_eq!(a.value("--click"), Some("b"));
    }

    #[test]
    fn mistakes_are_errors_that_name_the_argument() {
        assert_eq!(parse(&["play", "--sed", "1"]).unwrap_err(), "unknown argument '--sed'");
        assert_eq!(parse(&["play", "--seed"]).unwrap_err(), "--seed needs a value");
        assert_eq!(parse(&["play", "a", "b"]).unwrap_err(), "unknown argument 'b'");
        let a = parse(&["play", "--seed", "x"]).unwrap();
        assert_eq!(a.get("--seed", 1u64).unwrap_err(), "bad --seed 'x'");
    }
}
