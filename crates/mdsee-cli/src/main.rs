//! mdsee CLI（design.md §5, §6, §68, §70, §73, §100）。
//!
//! args → config → input → pipeline のみを担当する。

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};

use mdsee_core::{load_source, parse, InputSource};
use mdsee_layout::{layout, LayoutOptions};
use mdsee_render::{render, RenderOptions};
use mdsee_terminal::{terminal_columns, ColorLevel};

/// Markdownをターミナルへ美しく表示する。
#[derive(Parser)]
#[command(name = "mdsee", version, about, disable_help_subcommand = true)]
struct Cli {
    /// 表示するMarkdownファイル。`-` でstdinを読む。
    /// 省略時はstdinがpipeならstdinを読み、stdinもTTYならhelpを表示する（§6）。
    file: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // diagnosticsはstderrのみへ（§70）
            eprintln!("mdsee: {err:#}");
            // §68: runtime error = 1。CLI argument errorの2はclapが返す。
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let input = match cli.file.as_deref() {
        Some("-") => InputSource::Stdin,
        Some(path) => InputSource::File(PathBuf::from(path)),
        None => {
            if io::stdin().is_terminal() {
                // §6: FILE省略かつstdinもTTY → help表示
                let _ = Cli::command().print_help();
                return Ok(());
            }
            InputSource::Stdin
        }
    };

    let source = load_source(input).context("failed to load input")?;
    let document = parse(&source).context("failed to parse markdown")?;

    let options = LayoutOptions {
        terminal_width: terminal_columns().unwrap_or(80),
        ..LayoutOptions::default()
    };
    let layout_document = layout(&document, &options)?;

    let render_options = RenderOptions {
        // stdoutがTTYでない場合はplain rendering（§71 pipe safety）。
        // 環境変数によるColorLevel検出はSprint 2（S2-5）で導入する。
        color_level: if io::stdout().is_terminal() {
            ColorLevel::TrueColor
        } else {
            ColorLevel::None
        },
        margin: options.margin,
    };

    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());
    render(&layout_document, &mut writer, &render_options)?;
    writer.flush().context("failed to write output")?;
    Ok(())
}
