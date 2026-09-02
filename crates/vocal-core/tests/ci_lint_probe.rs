//! **TEMPORARY DIAGNOSTIC — delete after one use.**
//!
//! The Actions log hosts are unreachable from the environment auditing this repo, so a red
//! `Linting (clippy)` job is a red box: no message, no file, no lint name. Two rounds of
//! plausible-looking guesses produced a green-to-green no-op, which is the argument for
//! measuring instead.
//!
//! This test runs clippy over the workspace into its **own** target directory (the outer
//! `cargo test` has released its locks by the time tests run, and a separate `CARGO_TARGET_DIR`
//! means no contention), parses `--message-format=json`, and prints the diagnostics as GitHub
//! workflow commands. It then panics so libtest flushes that output into the step stream — which
//! is what the runner parses into check-run annotations.
//!
//! Deliberately crude and deliberately not part of the deliverable: it makes `Unit Tests` red on
//! purpose for exactly one run. `ops/ci/README.md` documents the older `build.rustc-wrapper`
//! variant and why that one is worse (Windows cannot exec a `/bin/sh` wrapper, so it reddens six
//! jobs and masks the platform).

use std::process::Command;

#[cfg(target_os = "linux")]
#[test]
fn ci_lint_probe() {
    use std::path::PathBuf;

    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = std::env::temp_dir().join("ci-lint-probe-target");
    let out = Command::new("cargo")
        .args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--offline",
            "--message-format=json",
            "--target-dir",
        ])
        .arg(&target)
        .args(["--", "-D", "warnings"])
        .current_dir(&repo)
        .output();

    let out = match out {
        Ok(o) => o,
        Err(e) => {
            println!("::error::LINTPROBE cargo could not run: {e}");
            panic!("lint probe: cargo unavailable");
        }
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut found: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let Ok(d) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if d.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }
        let level = d.get("level").and_then(|l| l.as_str()).unwrap_or("?");
        if level != "error" && level != "warning" {
            continue;
        }
        let Some(spans) = d.get("spans").and_then(|s| s.as_array()) else { continue };
        let Some(ours) = spans.iter().find_map(|s| {
            let f = s.get("file_name")?.as_str()?;
            if !(f.starts_with("crates/") || f.starts_with("apps/")) {
                return None;
            }
            Some((f.to_string(), s.get("line_start")?.as_u64()?, s.get("column_start")?.as_u64()?))
        }) else {
            continue;
        };
        let msg = d.get("message").and_then(|m| m.as_str()).unwrap_or("").replace('\n', " ");
        let code = d
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
            .unwrap_or("(no code)");
        let fix = d
            .get("children")
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("code"))
            .and_then(|c| c.get("suggestions"))
            .and_then(|s| s.as_array())
            .and_then(|s| s.first())
            .and_then(|s| s.get("replacement"))
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .replace('\n', " ");
        let fix = if fix.is_empty() { String::new() } else { format!(" | fix: {fix}") };
        found.push(format!("{code} @ {}:{}:{} :: {msg}{fix}", ours.0, ours.1, ours.2));
        if found.len() >= 24 {
            break;
        }
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    if found.is_empty() {
        // Nothing ours matched. Say so, and hand back the tail so "clippy died in a
        // dependency" (this repo's earlier failure mode) is distinguishable from "clippy is
        // happy" — an empty annotation set cannot tell those apart.
        let tail = stderr.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" ~ ");
        let tail: String = tail.chars().take(1100).collect(); // never split a UTF-8 char by byte index
        println!("::error::LINTPROBE no-diagnostics exit={} tail: {}", out.status.code().unwrap_or(-1), tail);
    } else {
        let joined = found.join(" ;; ");
        let chunks: Vec<String> = joined.as_bytes().chunks(1200)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect();
        let total = chunks.len();
        for (i, chunk) in chunks.into_iter().take(7).enumerate() {
            println!("::error::LINTPROBE[{}/{}] found={} {}", i + 1, total, found.len(), chunk);
        }
    }

    // Fail on purpose: libtest only writes captured stdout into the step stream for a failing
    // test, and the stream is what turns `::error::` lines into readable annotations.
    panic!("TEMPORARY lint probe (delete this file); {} diagnostic(s) above", found.len());
}

#[cfg(not(target_os = "linux"))]
#[test]
fn ci_lint_probe() {
    // One runner is enough to learn the lints; keeping the other platforms untouched is the
    // whole advantage this has over a `build.rustc-wrapper`.
}
