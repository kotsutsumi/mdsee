//! Integration test（design.md §83、S3-7）。
//!
//! バイナリを実行して exit status / stdout / stderr を確認する。

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_mdsee")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn renders_basic_fixture_to_stdout() {
    let output = Command::new(bin())
        .arg(fixture("basic.md"))
        .output()
        .expect("failed to run mdsee");

    assert_eq!(output.status.code(), Some(0), "exit status");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("mdsee"),
        "stdout should contain rendered text"
    );
    assert!(stdout.contains("Usage"));
    // stdoutがpipeなのでplain rendering（§71）。ANSI escapeは入らない
    assert!(
        !stdout.contains('\x1b'),
        "plain output must not contain ANSI"
    );
    assert!(
        output.stderr.is_empty(),
        "stderr must be empty: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reads_stdin_via_dash() {
    let mut child = Command::new(bin())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn mdsee");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"# Title\n\nbody\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Title"));
    assert!(!stdout.contains('\x1b'));
}

#[test]
fn reads_stdin_when_piped_without_dash() {
    let mut child = Command::new(bin())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn mdsee");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"# Piped\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8(output.stdout).unwrap().contains("Piped"));
}

#[test]
fn missing_file_is_runtime_error() {
    // §68: runtime error = 1
    let output = Command::new(bin())
        .arg("definitely/missing/file.md")
        .output()
        .expect("failed to run mdsee");
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.is_empty());
}

#[test]
fn invalid_flag_is_cli_error() {
    // §68: CLI argument error = 2
    let output = Command::new(bin())
        .arg("--no-such-flag")
        .output()
        .expect("failed to run mdsee");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn version_prints_and_exits_zero() {
    let output = Command::new(bin()).arg("--version").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("mdsee "));
}
