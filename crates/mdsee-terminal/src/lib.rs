//! mdsee-terminal（design.md §5）。
//!
//! Terminal capabilityを扱う（§34〜§37）。

use std::env;

use crossterm::tty::IsTty;

/// 色深度（§35）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorLevel {
    #[default]
    None,
    Ansi16,
    Ansi256,
    TrueColor,
}

/// Graphics protocol種別（§36）。判定はSprint 4（S4-1/S4-2）で導入する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsProtocol {
    Kitty,
    Iterm2,
    Sixel,
    Unicode,
}

/// セルのpixel寸法（§34）。Sprint 4（S4-1）で取得を導入する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPixelSize {
    pub width: u16,
    pub height: u16,
}

/// Terminal capability（§34）。
///
/// Sprint 2（S2-5）ではpassive detectionで得られる範囲のみ実装する。
/// `graphics` / `cell_pixels` はSprint 4（S4-1/S4-2）で埋める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub tty: bool,

    pub columns: u16,
    pub rows: u16,

    pub color_level: ColorLevel,

    pub osc8: bool,

    pub graphics: Vec<GraphicsProtocol>,

    pub cell_pixels: Option<CellPixelSize>,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self {
            tty: false,
            columns: 80,
            rows: 24,
            color_level: ColorLevel::None,
            osc8: false,
            graphics: Vec::new(),
            cell_pixels: None,
        }
    }
}

/// Terminal capabilityを検出する（§100 基本pipeline、§37 passive detection）。
///
/// 環境変数とTTY判定のみを使い、端末へのqueryは送らない。
/// active queryはReader / `--detect-terminal`（Sprint 4）でのみ行う。
pub fn detect_terminal() -> TerminalCapabilities {
    let vars = env::vars_os().map(|(k, v)| {
        (
            k.to_string_lossy().into_owned(),
            v.to_string_lossy().into_owned(),
        )
    });
    detect_from(std::io::stdout().is_tty(), vars)
}

/// 検出ロジック本体。環境変数はinjectableにして単体テスト可能にする。
pub fn detect_from<I, K, V>(stdout_is_tty: bool, vars: I) -> TerminalCapabilities
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let vars: Vec<(String, String)> = vars
        .into_iter()
        .map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
        .collect();

    let mut capabilities = TerminalCapabilities {
        tty: stdout_is_tty,
        ..TerminalCapabilities::default()
    };

    if let Ok((columns, rows)) = crossterm::terminal::size() {
        capabilities.columns = columns;
        capabilities.rows = rows;
    }

    capabilities.color_level = detect_color_level(&vars, stdout_is_tty);
    capabilities.osc8 = detect_osc8(&vars, stdout_is_tty);
    capabilities
}

/// 環境変数参照ヘルパー。
fn var<'a>(vars: &'a [(String, String)], name: &str) -> Option<&'a str> {
    vars.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// 色深度の判定（§35）。`NO_COLOR` を最優先する。
fn detect_color_level(vars: &[(String, String)], tty: bool) -> ColorLevel {
    // §72: NO_COLORが存在すれば color = false。空文字以外の値で発動。
    if let Some(value) = var(vars, "NO_COLOR") {
        if !value.is_empty() {
            return ColorLevel::None;
        }
    }

    if !tty {
        return ColorLevel::None;
    }

    let colorterm = var(vars, "COLORTERM").unwrap_or("");
    let term = var(vars, "TERM").unwrap_or("");
    let term_program = var(vars, "TERM_PROGRAM").unwrap_or("");

    if colorterm.eq_ignore_ascii_case("truecolor") || colorterm.eq_ignore_ascii_case("24bit") {
        return ColorLevel::TrueColor;
    }

    if !term_program.is_empty()
        && matches!(term_program, "iTerm.app" | "WezTerm" | "ghostty" | "kitty")
    {
        return ColorLevel::TrueColor;
    }

    if term == "xterm-kitty" || term == "xterm-ghostty" {
        return ColorLevel::TrueColor;
    }

    if term.contains("256color") || term.contains("direct") {
        if term.contains("direct") {
            return ColorLevel::TrueColor;
        }
        return ColorLevel::Ansi256;
    }

    if term.is_empty() || term == "dumb" {
        return ColorLevel::None;
    }

    ColorLevel::Ansi16
}

/// OSC 8対応判定（§33, §34）。
///
/// passive detectionでは、OSC 8非対応が既知の端末を除外し、
/// truecolor/256色の端末は対応とみなす。
fn detect_osc8(vars: &[(String, String)], tty: bool) -> bool {
    if !tty {
        return false;
    }
    let term = var(vars, "TERM").unwrap_or("");
    if term.is_empty() || term == "dumb" || term == "linux" {
        return false;
    }
    let term_program = var(vars, "TERM_PROGRAM").unwrap_or("");
    if term_program == "Apple_Terminal" {
        return false;
    }
    true
}

/// 端末の列数を返す。取得できない場合（非TTY等）は `None`。
///
/// S2-5で `detect_terminal()` へ統合済み。新規コードは
/// `detect_terminal().columns` を使うこと。
pub fn terminal_columns() -> Option<u16> {
    crossterm::terminal::size().ok().map(|(columns, _)| columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn no_color_wins_over_everything() {
        // §72: NO_COLORはtruecolor環境より優先
        let caps = detect_from(
            true,
            vars(&[
                ("NO_COLOR", "1"),
                ("COLORTERM", "truecolor"),
                ("TERM", "xterm-256color"),
            ]),
        );
        assert_eq!(caps.color_level, ColorLevel::None);
    }

    #[test]
    fn empty_no_color_is_ignored() {
        // NO_COLOR規約では空文字は発動しない
        let caps = detect_from(true, vars(&[("NO_COLOR", ""), ("COLORTERM", "truecolor")]));
        assert_eq!(caps.color_level, ColorLevel::TrueColor);
    }

    #[test]
    fn non_tty_disables_color() {
        let caps = detect_from(false, vars(&[("COLORTERM", "truecolor")]));
        assert_eq!(caps.color_level, ColorLevel::None);
        assert!(!caps.tty);
    }

    #[test]
    fn colorterm_truecolor() {
        let caps = detect_from(true, vars(&[("COLORTERM", "truecolor"), ("TERM", "xterm")]));
        assert_eq!(caps.color_level, ColorLevel::TrueColor);
    }

    #[test]
    fn term_256color() {
        let caps = detect_from(true, vars(&[("TERM", "xterm-256color")]));
        assert_eq!(caps.color_level, ColorLevel::Ansi256);
    }

    #[test]
    fn term_direct_is_truecolor() {
        let caps = detect_from(true, vars(&[("TERM", "xterm-direct")]));
        assert_eq!(caps.color_level, ColorLevel::TrueColor);
    }

    #[test]
    fn plain_xterm_is_ansi16() {
        let caps = detect_from(true, vars(&[("TERM", "xterm")]));
        assert_eq!(caps.color_level, ColorLevel::Ansi16);
    }

    #[test]
    fn dumb_term_has_no_color() {
        let caps = detect_from(true, vars(&[("TERM", "dumb")]));
        assert_eq!(caps.color_level, ColorLevel::None);
        assert!(!caps.osc8);
    }

    #[test]
    fn kitty_term_is_truecolor_with_osc8() {
        let caps = detect_from(true, vars(&[("TERM", "xterm-kitty")]));
        assert_eq!(caps.color_level, ColorLevel::TrueColor);
        assert!(caps.osc8);
    }

    #[test]
    fn term_program_iterm_is_truecolor() {
        let caps = detect_from(
            true,
            vars(&[("TERM_PROGRAM", "iTerm.app"), ("TERM", "xterm-256color")]),
        );
        assert_eq!(caps.color_level, ColorLevel::TrueColor);
        assert!(caps.osc8);
    }

    #[test]
    fn apple_terminal_lacks_osc8() {
        let caps = detect_from(
            true,
            vars(&[
                ("TERM_PROGRAM", "Apple_Terminal"),
                ("TERM", "xterm-256color"),
            ]),
        );
        assert!(!caps.osc8);
    }

    #[test]
    fn no_env_vars_at_all() {
        let caps = detect_from(true, Vec::<(String, String)>::new());
        assert_eq!(caps.color_level, ColorLevel::None);
        assert!(!caps.osc8);
    }
}
