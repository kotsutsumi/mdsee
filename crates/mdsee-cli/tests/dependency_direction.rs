//! 依存方向の検査（design.md §101、S1-2）。
//!
//! `cargo tree` で各crateから到達可能なmdsee系crateを列挙し、
//! §101の依存方向（特に `core → terminal` 禁止）に反していないことを確認する。

use std::path::PathBuf;
use std::process::Command;

const MDSEE_CRATES: [&str; 7] = [
    "mdsee-core",
    "mdsee-layout",
    "mdsee-render",
    "mdsee-terminal",
    "mdsee-image",
    "mdsee-reader",
    "mdsee",
];

/// §101で許可されるmdsee系への依存。
///
/// - core: 依存なし
/// - layout: core のみ（将来は image metadata のみ追加可）
/// - render: core / layout / terminal
/// - terminal: 依存なし
/// - image: core / terminal（§101「terminal ← image」）
/// - reader: layout / render / terminal / image（+ core）
/// - cli: everything
fn allowed_mdsee_deps(crate_name: &str) -> &'static [&'static str] {
    match crate_name {
        "mdsee-core" => &[],
        "mdsee-layout" => &["mdsee-core"],
        "mdsee-render" => &["mdsee-core", "mdsee-layout", "mdsee-terminal"],
        "mdsee-terminal" => &[],
        "mdsee-image" => &["mdsee-core", "mdsee-terminal"],
        "mdsee-reader" => &[
            "mdsee-core",
            "mdsee-layout",
            "mdsee-render",
            "mdsee-terminal",
            "mdsee-image",
        ],
        "mdsee" => &[
            "mdsee-core",
            "mdsee-layout",
            "mdsee-render",
            "mdsee-terminal",
            "mdsee-image",
            "mdsee-reader",
        ],
        _ => &[],
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("workspace root")
        .to_path_buf()
}

/// `cargo tree` で crate_name から（伝達含む）到達可能なmdsee系crateを列挙する。
fn reachable_mdsee_deps(crate_name: &str) -> Vec<String> {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree", "-p", crate_name, "-e", "normal", "--prefix", "none", "--format", "{p}",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run cargo tree");

    assert!(
        output.status.success(),
        "cargo tree failed for {crate_name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut deps: Vec<String> = stdout
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| name.starts_with("mdsee-") && *name != crate_name)
        .map(str::to_string)
        .collect();
    deps.sort();
    deps.dedup();
    deps
}

#[test]
fn dependency_direction_follows_design() {
    for crate_name in MDSEE_CRATES {
        let reachable = reachable_mdsee_deps(crate_name);
        let allowed = allowed_mdsee_deps(crate_name);

        let violations: Vec<&String> = reachable
            .iter()
            .filter(|dep| !allowed.contains(&dep.as_str()))
            .collect();
        assert!(
            violations.is_empty(),
            "{crate_name} は §101 で許可されていない依存を持つ: {violations:?} \
             (allowed: {allowed:?})"
        );
    }
}

#[test]
fn core_never_reaches_terminal() {
    // §101で明示禁止の core → terminal を最優先で検査
    let reachable = reachable_mdsee_deps("mdsee-core");
    assert!(
        !reachable.iter().any(|dep| dep == "mdsee-terminal"),
        "§101 違反: mdsee-core が mdsee-terminal に到達可能: {reachable:?}"
    );
}
