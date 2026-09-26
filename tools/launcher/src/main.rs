//! Release-archive launcher (`D-072`): runs the fastest build of an example that
//! this CPU supports, from the archive root.
//!
//! Archives ship one copy of this binary per example, named like the example
//! (`example-01-platformer[.exe]`), next to `bin/<level>/<example>` builds for
//! several x86-64 levels. The launcher picks the highest level whose directory
//! exists and whose features the CPU reports, then runs that build with the
//! archive root as the working directory, where the examples read
//! `tungsten.json`, `input.json` and their assets. It names the chosen level on
//! stderr; `TUNGSTEN_CPU_LEVEL` forces one. Standard library only; the levels
//! must match `LEVELS` in `scripts/release.py`.

use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, ExitCode};

/// Levels the launcher knows, fastest first. `x86-64` is the portable baseline.
const LEVELS: [&str; 4] = ["x86-64-v4", "x86-64-v3", "x86-64-v2", "x86-64"];
/// Environment override naming one of [`LEVELS`].
const OVERRIDE: &str = "TUNGSTEN_CPU_LEVEL";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("tungsten-launcher: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let exe = env::current_exe().map_err(|e| format!("cannot locate this executable: {e}"))?;
    let root = exe.parent().ok_or("executable has no parent directory")?;
    let name = exe.file_name().ok_or("executable has no file name")?;
    let bin = root.join("bin");
    let available: Vec<&str> = LEVELS
        .into_iter()
        .filter(|level| bin.join(level).join(name).is_file())
        .collect();
    let forced = env::var(OVERRIDE).ok();
    let level = choose(&available, supports, forced.as_deref())?;
    eprintln!("tungsten-launcher: running the {level} build");
    launch(
        &bin.join(level).join(name),
        root,
        env::args_os().skip(1).collect(),
    )
}

/// The level to run: the override if given and shipped, else the fastest
/// shipped level the CPU supports.
fn choose<'a>(
    available: &[&'a str],
    supported: impl Fn(&str) -> bool,
    forced: Option<&str>,
) -> Result<&'a str, String> {
    if let Some(forced) = forced {
        return available
            .iter()
            .copied()
            .find(|level| *level == forced)
            .ok_or_else(|| {
                format!(
                    "{OVERRIDE}={forced} is not shipped here; available: {}",
                    available.join(", ")
                )
            });
    }
    available
        .iter()
        .copied()
        .find(|level| supported(level))
        .ok_or_else(|| {
            format!(
                "no shipped build runs on this CPU; available: {}",
                available.join(", ")
            )
        })
}

/// CPU features per level, matching `rustc --print cfg -C target-cpu=<level>`.
#[cfg(target_arch = "x86_64")]
fn supports(level: &str) -> bool {
    use std::arch::is_x86_feature_detected as has;
    let v2 = || {
        has!("sse3")
            && has!("ssse3")
            && has!("sse4.1")
            && has!("sse4.2")
            && has!("popcnt")
            && has!("cmpxchg16b")
    };
    let v3 = || {
        v2() && has!("avx")
            && has!("avx2")
            && has!("bmi1")
            && has!("bmi2")
            && has!("f16c")
            && has!("fma")
            && has!("lzcnt")
            && has!("movbe")
            && has!("xsave")
    };
    let v4 = || {
        v3() && has!("avx512f")
            && has!("avx512bw")
            && has!("avx512cd")
            && has!("avx512dq")
            && has!("avx512vl")
    };
    match level {
        "x86-64-v4" => v4(),
        "x86-64-v3" => v3(),
        "x86-64-v2" => v2(),
        "x86-64" => true,
        _ => false,
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn supports(level: &str) -> bool {
    level == "x86-64"
}

#[cfg(unix)]
fn launch(target: &Path, root: &Path, args: Vec<OsString>) -> Result<ExitCode, String> {
    use std::os::unix::process::CommandExt;
    // `exec` only returns on failure; on success the example replaces this process.
    let error = Command::new(target).args(args).current_dir(root).exec();
    Err(format!("cannot start {}: {error}", target.display()))
}

#[cfg(not(unix))]
fn launch(target: &Path, root: &Path, args: Vec<OsString>) -> Result<ExitCode, String> {
    let status = Command::new(target)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot start {}: {e}", target.display()))?;
    Ok(status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map_or(ExitCode::FAILURE, ExitCode::from))
}

#[cfg(test)]
mod tests {
    use super::choose;

    const SHIPPED: [&str; 2] = ["x86-64-v3", "x86-64"];

    #[test]
    fn picks_the_fastest_supported_level() {
        assert_eq!(choose(&SHIPPED, |_| true, None), Ok("x86-64-v3"));
        assert_eq!(
            choose(&SHIPPED, |level| level == "x86-64", None),
            Ok("x86-64")
        );
    }

    #[test]
    fn override_selects_a_shipped_level_even_if_unsupported() {
        assert_eq!(
            choose(&SHIPPED, |_| false, Some("x86-64-v3")),
            Ok("x86-64-v3")
        );
        assert!(
            choose(&SHIPPED, |_| true, Some("x86-64-v4"))
                .unwrap_err()
                .contains("not shipped")
        );
    }

    #[test]
    fn nothing_runnable_is_an_error() {
        assert!(
            choose(&["x86-64-v3"], |_| false, None)
                .unwrap_err()
                .contains("no shipped build")
        );
        assert!(choose(&[], |_| true, None).is_err());
    }
}
