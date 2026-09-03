//! End-to-end tests that drive `sample_reader` as a subprocess, against the packs this repository
//! actually ships in `voice-packs/dist/`.
//!
//! Why subprocesses: the point is to check the *integrator's* experience. Compiling against the crates
//! would let anything `pub(crate)` or awkwardly-typed slide; running the binary checks that pack
//! loading, the persona's chunking policy, planning and rendering compose through the public API and
//! that a failure arrives as a message rather than a panic.

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_sample_reader"))
}

fn in_repo(rest: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rest)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chiti-sample-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir for the sample run");
    dir
}

/// Runs the sample with `--out` pointed into a private temp dir, returning (code, stdout, stderr).
fn run(tag: &str, args: &[&str]) -> (Option<i32>, String, String) {
    let dir = temp_dir(tag);
    let out = dir.join("sample-out.wav");
    let output = Command::new(binary())
        .args(args)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("sample_reader must be runnable; CI builds it as part of --all-targets");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn value_after<'a>(haystack: &'a str, key: &str) -> Option<&'a str> {
    haystack
        .split_whitespace()
        .find(|token| token.starts_with(key))
        .map(|token| &token[key.len()..])
}

/// The pack every test uses, chosen so the assertion is about integration, not about fixtures.
fn shipped_pack() -> String {
    in_repo("../../voice-packs/dist/tara.cvpack").to_string_lossy().into_owned()
}

#[test]
fn renders_the_shipped_pack_through_the_public_api() {
    let pack = shipped_pack();
    let lines = in_repo("fixtures/lines.txt");
    let lines = lines.to_string_lossy().into_owned();
    let (code, stdout, stderr) = run("basic", &["--pack", pack.as_str(), "--lines", lines.as_str()]);
    assert_eq!(code, Some(0), "sample failed: {stdout}\n{stderr}");

    assert!(
        stdout.contains("id=tara"),
        "the pack's identity should be visible in the report: {stdout}"
    );
    assert!(
        stdout.contains("declared=pack"),
        "tara declares persona.chunking, so the policy must be reported as declared rather than \
         as the engine default: {stdout}"
    );
    assert!(
        !stdout.contains("row_matches_units=false"),
        "a style row that disagrees with its unit count means the voice-vector index moved:\n{stdout}"
    );
    assert!(
        stdout.contains("line 4 chunks="),
        "every line of the fixture should be reported:\n{stdout}"
    );
    assert!(
        stdout.contains("placeholder=true"),
        "the shipped packs contain placeholder model files; the sample must not hide that:\n{stdout}"
    );
    assert!(
        stdout.contains("REAL_SYNTHESIS_AVAILABLE=false"),
        "the sample has to say what the bytes are, or a reader will assume they are speech:\n{stdout}"
    );
}

#[test]
fn the_declared_policy_is_a_usable_number_not_a_string() {
    let pack = shipped_pack();
    let (_, stdout, _) = run("policy", &["--pack", pack.as_str(), "--text", "hələʊ wɜːld"]);
    let max_units = value_after(&stdout, "max_units=")
        .and_then(|v| v.parse::<usize>().ok())
        .expect("the report must print max_units as a number");
    let min_chunk = value_after(&stdout, "min_chunk_units=")
        .and_then(|v| v.parse::<usize>().ok())
        .expect("the report must print min_chunk_units as a number");
    // Deliberately not `assert_eq!(max_units, 509)`: the number is the engine's and the pack's to
    // agree on, and a third copy here would only pin this file to whoever last touched it. What the
    // consumer needs is that the resolved policy is inside the model's window and coherent.
    assert!(max_units >= 1 && min_chunk >= 1, "counts are budgets, not hints: {stdout}");
    assert!(min_chunk <= max_units, "a floor above the ceiling can never close a chunk: {stdout}");
}

#[test]
fn long_input_is_planned_into_several_chunks() {
    let pack = shipped_pack();
    let long: String = "hələʊ wɜːld əv ˈaʊdʒoʊ. ".repeat(90);
    let long = long.trim();
    let (code, stdout, stderr) = run("chunking", &["--pack", pack.as_str(), "--text", long]);
    assert_eq!(code, Some(0), "sample failed: {stderr}");
    let chunks = value_after(&stdout, "chunks=")
        .and_then(|v| v.parse::<usize>().ok())
        .expect("the report must print chunks=");
    assert!(
        chunks > 1,
        "~1200 phoneme units under a {long:?}-sized input must not stay one utterance: {stdout}"
    );
}

#[test]
fn the_same_input_produces_the_same_report_and_file() {
    let pack = shipped_pack();
    let lines = in_repo("fixtures/lines.txt");
    let lines = lines.to_string_lossy().into_owned();
    let (_, first, _) = run("det1", &["--pack", pack.as_str(), "--lines", lines.as_str()]);
    let (_, second, _) = run("det2", &["--pack", pack.as_str(), "--lines", lines.as_str()]);
    assert_eq!(
        first, second,
        "the report differs between identical runs, so a render cannot be cited by its numbers"
    );
    assert!(first.contains("silent=true") || first.contains("silent=false"));

    let a = temp_dir("det1").join("sample-out.wav");
    let b = temp_dir("det2").join("sample-out.wav");
    if let (Ok(a), Ok(b)) = (std::fs::read(&a), std::fs::read(&b)) {
        assert_eq!(a.len(), b.len(), "the written WAVs differ in length");
        assert_eq!(a, b, "the written WAVs differ in bytes");
    }
}

#[test]
fn a_missing_pack_fails_with_a_message_not_a_panic() {
    let (code, _stdout, stderr) = run(
        "missing",
        &["--pack", "/nonexistent/no-such-pack.cvpack", "--text", "hələʊ"],
    );
    assert_eq!(code, Some(1), "a bad pack path must be a clean failure");
    assert!(
        stderr.contains("not found") || stderr.contains("No such file"),
        "the loader's own error should reach the user, not be swallowed: {stderr}"
    );
}
