//! mdsee CLI（design.md §5, §6, §8, §68〜§72, §100）。
//!
//! args → config → input → pipeline のみを担当する。

mod config;

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};

use mdsee_core::{load_source, parse, InputSource};
use mdsee_layout::{layout, LayoutOptions};
use mdsee_render::{render, RenderOptions, Theme};
use mdsee_terminal::{detect_terminal, ColorLevel, TerminalCapabilities};

/// 出力mode（§8）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Print,
    /// Sprint 6（S6-1）で実装する。Sprint 2では判定のみ定義。
    #[allow(dead_code)]
    Reader,
    Plain,
}

/// Markdownをターミナルへ美しく表示する。
#[derive(Parser)]
#[command(name = "mdsee", version, about, disable_help_subcommand = true)]
struct Cli {
    /// 表示するMarkdownファイル。`-` でstdinを読む。
    /// 省略時はstdinがpipeならstdinを読み、stdinもTTYならhelpを表示する（§6）。
    file: Option<String>,

    /// 装飾なしのplain textへrenderする（§8）。
    #[arg(short, long)]
    plain: bool,

    /// 端末幅をN桁とみなす（§7）。
    #[arg(long)]
    width: Option<u16>,

    /// 本文の最大幅（§21）。デフォルトはconfig / 100。
    #[arg(long)]
    max_width: Option<u16>,

    /// theme名（§62）。
    #[arg(long, value_parser = ["auto", "dark", "light"])]
    theme: Option<String>,

    /// 色を強制する。`NO_COLOR` を上書きする唯一の手段（§72）。
    #[arg(long)]
    force_color: bool,

    /// 色を無効化する。
    #[arg(long)]
    no_color: bool,

    /// passive detectionの結果をstderrへ出力して終了する（§37）。
    #[arg(long)]
    detect_terminal: bool,

    /// debug logをstderrへ出力する（§69）。
    #[arg(long)]
    debug: bool,

    /// parse後のASTをstderrへ出力して終了する。
    #[arg(long)]
    dump_ast: bool,

    /// layout後の木をstderrへ出力して終了する。
    #[arg(long)]
    dump_layout: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.debug);
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

/// `--debug` のときのみstderrへloggingを出す（§69）。通常は一切出さない。
fn init_logging(debug: bool) {
    if debug {
        let _ = tracing_subscriber::fmt()
            .with_writer(io::stderr)
            .with_max_level(tracing::Level::DEBUG)
            .with_target(false)
            .try_init();
    }
}

fn run(cli: Cli) -> Result<()> {
    // §65: defaults < config file < CLI
    let config = load_config();

    let capabilities = detect_terminal();
    tracing::debug!(?capabilities, "terminal capabilities");

    if cli.detect_terminal {
        print_capabilities(&capabilities);
        return Ok(());
    }

    let mode = output_mode(&cli, &capabilities);
    tracing::debug!(?mode, "output mode");

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

    if cli.dump_ast {
        // §70: dumpはdiagnostics。stdoutはdocument出力専用。
        eprintln!("{document:#?}");
        return Ok(());
    }

    let layout_options = LayoutOptions {
        terminal_width: cli.width.unwrap_or_else(|| capabilities.columns.max(1)),
        max_width: cli.max_width.unwrap_or(config.layout.max_width),
        margin: config.layout.margin,
    };
    tracing::debug!(?layout_options, "layout options");

    let layout_document = layout(&document, &layout_options)?;

    if cli.dump_layout {
        eprintln!("{layout_document:#?}");
        return Ok(());
    }

    let render_options = RenderOptions {
        color_level: resolve_color_level(&cli, &capabilities, mode),
        margin: layout_options.margin,
        osc8: mode == OutputMode::Print && capabilities.osc8,
        theme: resolve_theme(&cli, &config),
    };
    tracing::debug!(?render_options, "render options");

    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());
    render(&layout_document, &mut writer, &render_options)?;
    writer.flush().context("failed to write output")?;
    Ok(())
}

/// 自動mode判定（§8）。Sprint 2では Print / Plain のみ。
fn output_mode(cli: &Cli, capabilities: &TerminalCapabilities) -> OutputMode {
    if !capabilities.tty {
        // §71: stdoutがTTYでなければplain
        return OutputMode::Plain;
    }
    if cli.plain {
        return OutputMode::Plain;
    }
    // --reader / --watch はSprint 6/7で追加する
    OutputMode::Print
}

/// 色の解決（§35, §71, §72）。
fn resolve_color_level(
    cli: &Cli,
    capabilities: &TerminalCapabilities,
    mode: OutputMode,
) -> ColorLevel {
    if cli.no_color {
        return ColorLevel::None;
    }
    if mode == OutputMode::Plain {
        // §71: pipeではANSI OFF
        return ColorLevel::None;
    }
    if cli.force_color {
        // §72: NO_COLORを除いた環境変数で再判定する
        let vars = std::env::vars_os().map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        });
        let vars: Vec<(String, String)> = vars.filter(|(k, _)| k != "NO_COLOR").collect();
        let mut forced = mdsee_terminal::detect_from(capabilities.tty, vars);
        if forced.color_level == ColorLevel::None {
            forced.color_level = ColorLevel::TrueColor;
        }
        return forced.color_level;
    }
    capabilities.color_level
}

/// themeの解決（§62）。CLI > config file > defaults（§65）。
fn resolve_theme(cli: &Cli, config: &config::ConfigFile) -> Theme {
    let name = cli.theme.as_deref().unwrap_or(&config.theme);
    match name {
        "auto" => {
            let colorfgbg = std::env::var("COLORFGBG").ok();
            mdsee_render::select_auto_theme(colorfgbg.as_deref())
        }
        "light" => Theme::light(),
        _ => Theme::dark(),
    }
}

/// config.tomlの読み込み（§63, §65）。不正な場合は警告してdefaultsを使う（§67）。
fn load_config() -> config::ConfigFile {
    match config::config_path() {
        Some(path) if path.exists() => match config::load_config_file(&path) {
            Ok(config) => config,
            Err(message) => {
                eprintln!("mdsee: ignoring invalid config: {message}");
                config::ConfigFile::default()
            }
        },
        _ => config::ConfigFile::default(),
    }
}

/// `--detect-terminal` の出力（§34）。§70によりstderrへ出す。
fn print_capabilities(capabilities: &TerminalCapabilities) {
    eprintln!("tty: {}", capabilities.tty);
    eprintln!("columns: {}", capabilities.columns);
    eprintln!("rows: {}", capabilities.rows);
    eprintln!("color_level: {:?}", capabilities.color_level);
    eprintln!("osc8: {}", capabilities.osc8);
    eprintln!("graphics: {:?}", capabilities.graphics);
    eprintln!("cell_pixels: {:?}", capabilities.cell_pixels);
}
