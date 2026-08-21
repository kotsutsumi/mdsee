//! Syntax highlight（design.md §29, §92）。
//!
//! Renderer（rendering code path）は `SyntaxHighlighter` traitのみに依存し、
//! syntectのAPIを直接呼ばない。`SyntectHighlighter` はfeature `syntax`
//! 有効時のみ提供される。

use crate::style::Rgb;

/// highlight済みの1行。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HighlightedLine {
    pub spans: Vec<HighlightedSpan>,
}

/// highlight済みの1区切り。色はsyntect theme由来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedSpan {
    pub text: String,
    pub fg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// syntax highlight interface（§29）。
pub trait SyntaxHighlighter {
    fn highlight(&self, code: &str, language: Option<&str>) -> Vec<HighlightedLine>;
}

/// highlightを行わない実装。feature `syntax` 無効時のfallback。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHighlight;

impl SyntaxHighlighter for NoHighlight {
    fn highlight(&self, code: &str, _language: Option<&str>) -> Vec<HighlightedLine> {
        code.lines()
            .map(|_| HighlightedLine { spans: Vec::new() })
            .collect()
    }
}

/// syntect実装（§29）。feature `syntax` で有効化。
#[cfg(feature = "syntax")]
pub mod syntect_backend {
    use super::{HighlightedLine, HighlightedSpan, Rgb, SyntaxHighlighter};
    use std::sync::OnceLock;
    use syntect::easy::HighlightLines;
    use syntect::highlighting::{FontStyle, ThemeSet};
    use syntect::parsing::SyntaxSet;

    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

    /// syntect backendの `SyntectHighlighter`（§29）。
    ///
    /// theme名は `Theme.syntax_theme`（§61）から渡される。
    /// 未登録のtheme名は `base16-ocean.dark` へfallbackする。
    #[derive(Debug, Clone)]
    pub struct SyntectHighlighter {
        theme_name: String,
    }

    impl SyntectHighlighter {
        pub fn new(theme_name: impl Into<String>) -> Self {
            Self {
                theme_name: theme_name.into(),
            }
        }
    }

    impl Default for SyntectHighlighter {
        fn default() -> Self {
            Self::new("base16-ocean.dark")
        }
    }

    impl SyntaxHighlighter for SyntectHighlighter {
        fn highlight(&self, code: &str, language: Option<&str>) -> Vec<HighlightedLine> {
            let syntax_set = SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines);
            let theme_set = THEME_SET.get_or_init(ThemeSet::load_defaults);
            let fallback = theme_set.themes.get("base16-ocean.dark");
            let theme = theme_set
                .themes
                .get(&self.theme_name)
                .or(fallback)
                .expect("built-in theme set always contains base16-ocean.dark");

            let syntax = language
                .and_then(|token| syntax_set.find_syntax_by_token(token))
                .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

            let mut highlighter = HighlightLines::new(syntax, theme);
            let mut lines = Vec::new();
            for line in code.split_inclusive('\n') {
                let mut spans: Vec<HighlightedSpan> = Vec::new();
                let ranges = highlighter.highlight_line(line, syntax_set);
                match ranges {
                    Ok(ranges) => {
                        for (style, text) in ranges {
                            let text = text.strip_suffix('\n').unwrap_or(text);
                            let text = text.strip_suffix('\r').unwrap_or(text);
                            if text.is_empty() {
                                continue;
                            }
                            spans.push(HighlightedSpan {
                                text: text.to_string(),
                                fg: Rgb(style.foreground.r, style.foreground.g, style.foreground.b),
                                bold: style.font_style.contains(FontStyle::BOLD),
                                italic: style.font_style.contains(FontStyle::ITALIC),
                                underline: style.font_style.contains(FontStyle::UNDERLINE),
                            });
                        }
                    }
                    Err(_) => spans.push(HighlightedSpan {
                        text: line.trim_end_matches(['\n', '\r']).to_string(),
                        fg: Rgb(166, 173, 186),
                        bold: false,
                        italic: false,
                        underline: false,
                    }),
                }
                lines.push(HighlightedLine { spans });
            }
            // 末尾の空行分（code.lines() と長期を合わせるための調整は不要。
            // split_inclusive('\n') は code.lines() と同一の行数を返す）
            lines
        }
    }
}

#[cfg(all(test, feature = "syntax"))]
mod tests {
    use super::*;

    #[test]
    fn rust_code_gets_multiple_colored_spans() {
        let highlighter = syntect_backend::SyntectHighlighter::default();
        let lines = highlighter.highlight("fn main() {}\n", Some("rust"));
        assert_eq!(lines.len(), 1);
        let joined: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, "fn main() {}");
        // keyword等で複数色に分かれる
        assert!(lines[0].spans.len() >= 2, "spans: {:?}", lines[0].spans);
        let colors: std::collections::HashSet<_> = lines[0].spans.iter().map(|s| s.fg).collect();
        assert!(colors.len() >= 2);
    }

    #[test]
    fn unknown_language_falls_back_to_plain() {
        let highlighter = syntect_backend::SyntectHighlighter::default();
        let lines = highlighter.highlight("just text\n", Some("no-such-lang"));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].text, "just text");
    }

    #[test]
    fn multi_line_code_preserves_line_count() {
        let highlighter = syntect_backend::SyntectHighlighter::default();
        let lines = highlighter.highlight("a\nb\nc\n", None);
        assert_eq!(lines.len(), 3);
        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect();
        assert_eq!(texts, ["a", "b", "c"]);
    }

    #[test]
    fn unknown_theme_falls_back_to_default() {
        let highlighter = syntect_backend::SyntectHighlighter::new("no-such-theme");
        let lines = highlighter.highlight("x\n", None);
        assert_eq!(lines.len(), 1);
    }
}
