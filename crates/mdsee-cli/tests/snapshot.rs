//! Snapshot test（design.md §81、S3-5）。
//!
//! `tests/fixtures/*.md` を固定幅でlayoutし、plain / ANSI tokenized の
//! 2種類のsnapshotと比較する。ANSI escapeは人間が読めるtokenへ
//! normalizeしてから保存する。
//!
//! snapshotの更新:
//!
//! ```sh
//! MDSEE_UPDATE_SNAPSHOTS=1 cargo test -p mdsee-cli --test snapshot
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use mdsee_core::{load_source, parse, InputSource};
use mdsee_layout::{layout, LayoutOptions};
use mdsee_render::{render, RenderOptions, Theme};
use mdsee_terminal::ColorLevel;

const TERMINAL_WIDTH: u16 = 60;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn snapshots_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/snapshots")
}

fn render_fixture(path: &Path, color: ColorLevel) -> String {
    let source = load_source(InputSource::File(path.to_path_buf())).unwrap();
    let document = parse(&source).unwrap();
    let options = LayoutOptions {
        terminal_width: TERMINAL_WIDTH,
        max_width: 100,
        margin: 2,
    };
    let laid = layout(&document, &options).unwrap();
    let render_options = RenderOptions {
        color_level: color,
        margin: options.margin,
        osc8: color != ColorLevel::None,
        theme: Theme::dark(),
    };
    let mut buffer: Vec<u8> = Vec::new();
    render(&laid, &mut buffer, &render_options).unwrap();
    String::from_utf8(buffer).unwrap()
}

/// ANSI escape sequenceを読みやすいtokenへnormalizeする（§81）。
///
/// - SGR（`ESC [ ... m`）は `{...}`（resetは `{/}`）
/// - OSC 8（`ESC ] 8 ;; URL ST`）は `{link:URL}`
fn normalize_ansi(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                let mut params = String::new();
                for p in chars.by_ref() {
                    if p == 'm' {
                        break;
                    }
                    params.push(p);
                }
                if params == "0" {
                    out.push_str("{/}");
                } else {
                    out.push('{');
                    out.push_str(&params);
                    out.push('}');
                }
            }
            Some(']') => {
                let mut body = String::new();
                loop {
                    match chars.next() {
                        Some('\x07') => break,
                        Some('\x1b') => {
                            chars.next();
                            break;
                        }
                        Some(p) => body.push(p),
                        None => break,
                    }
                }
                if let Some(url) = body.strip_prefix("8;;") {
                    out.push_str(&format!("{{link:{url}}}"));
                }
            }
            _ => {}
        }
    }
    out
}

fn check_snapshot(name: &str, kind: &str, content: String) {
    let path = snapshots_dir().join(format!("{name}.{kind}.txt"));
    if std::env::var("MDSEE_UPDATE_SNAPSHOTS").is_ok() {
        fs::write(&path, &content).expect("failed to write snapshot");
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "snapshot not found: {}. run with MDSEE_UPDATE_SNAPSHOTS=1",
            path.display()
        )
    });
    assert_eq!(content, expected, "snapshot mismatch: {}", path.display());
}

fn snapshot_test(name: &str) {
    let fixture = fixtures_dir().join(format!("{name}.md"));
    assert!(fixture.exists(), "fixture not found: {}", fixture.display());

    let plain = render_fixture(&fixture, ColorLevel::None);
    check_snapshot(name, "plain", plain);

    let ansi = normalize_ansi(&render_fixture(&fixture, ColorLevel::TrueColor));
    check_snapshot(name, "ansi", ansi);
}

macro_rules! snapshot {
    ($test_name:ident, $fixture:literal) => {
        #[test]
        fn $test_name() {
            snapshot_test($fixture);
        }
    };
}

snapshot!(snapshot_headings, "headings");
snapshot!(snapshot_tables, "tables");
snapshot!(snapshot_japanese, "japanese");
snapshot!(snapshot_emoji, "emoji");
snapshot!(snapshot_code, "code");
snapshot!(snapshot_alerts, "alerts");
snapshot!(snapshot_complete, "complete");
