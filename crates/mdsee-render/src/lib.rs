//! mdsee-render（design.md §5）。
//!
//! Layout TreeからANSI / plain textを生成する。

mod highlight;
mod style;
mod theme;

use std::io;

use thiserror::Error;

use mdsee_layout::{LayoutBlock, LayoutDocument, LayoutLine, LayoutSpan, SemanticStyle};
use mdsee_terminal::ColorLevel;

#[cfg(feature = "syntax")]
pub use highlight::syntect_backend::SyntectHighlighter;
pub use highlight::{HighlightedLine, HighlightedSpan, NoHighlight, SyntaxHighlighter};
pub use theme::{select_auto_theme, AlertTheme, TextStyle, Theme};

/// ANSI reset sequence。
const RESET: &str = "\x1b[0m";

/// Render error（§66）。
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("failed to write output")]
    Write(#[from] io::Error),
}

/// Render options。
///
/// `color_level` が `None` の場合はplain text renderingになる（§71）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOptions {
    pub color_level: ColorLevel,
    /// 行頭の左margin（§22）。layoutのmarginと同じ値を渡す。
    pub margin: u16,
    /// OSC 8 hyperlinkを出すか（§33）。`false` なら `text <URL>` へfallbackする。
    pub osc8: bool,
    /// Theme（§61, S2-9）。
    pub theme: Theme,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            color_level: ColorLevel::None,
            margin: 2,
            osc8: false,
            theme: Theme::dark(),
        }
    }
}

/// LayoutDocumentをtargetへ書き出す（§100 基本pipeline）。
///
/// blockの間に空行を1つ挟む。空行には余白を出さない。
pub fn render(
    document: &LayoutDocument,
    target: &mut dyn io::Write,
    options: &RenderOptions,
) -> Result<(), RenderError> {
    let mut output = String::new();
    for (index, block) in document.blocks.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        match block {
            LayoutBlock::Text(text_block) => {
                for line in &text_block.lines {
                    write_text_line(&mut output, line, options);
                }
            }
            LayoutBlock::Code(code) => write_code_block(&mut output, code, options),
            LayoutBlock::Rule(rule) => write_rule_line(&mut output, rule.width, options),
            LayoutBlock::Table(table) => {
                for line in &table.lines {
                    write_text_line(&mut output, line, options);
                }
            }
        }
    }
    target.write_all(output.as_bytes())?;
    Ok(())
}

fn write_text_line(output: &mut String, line: &LayoutLine, options: &RenderOptions) {
    if line.spans.iter().all(|span| span.content.is_empty()) {
        output.push('\n');
        return;
    }
    push_margin(output, options.margin);
    match options.color_level {
        ColorLevel::None => {
            for (position, span) in line.spans.iter().enumerate() {
                push_span_plain(output, span, fallback_url(span, line, position));
            }
        }
        level => {
            for (position, span) in line.spans.iter().enumerate() {
                let sequence = style::sgr_sequence(&options.theme.spec(span.style), level);
                if !sequence.is_empty() {
                    output.push_str(&sequence);
                }
                match (&span.link, options.osc8) {
                    // §33: OSC 8 hyperlink
                    (Some(link), true) => {
                        output.push_str(&osc8_open(&link.url));
                        output.push_str(&span.content);
                        output.push_str(OSC8_CLOSE);
                    }
                    _ => {
                        push_span_plain(output, span, fallback_url(span, line, position));
                    }
                }
                if !sequence.is_empty() {
                    output.push_str(RESET);
                }
            }
        }
    }
    output.push('\n');
}

/// span本文を書き出し、fallback URLがあれば末尾空白の直前に置く。
///
/// `docs ` なら `docs <URL> ` とすることで、
/// `docs  <URL>`（空白の二重化）を防ぐ。
fn push_span_plain(output: &mut String, span: &LayoutSpan, fallback_url: Option<&str>) {
    let trimmed = span.content.trim_end_matches(' ');
    let trailing = &span.content[trimmed.len()..];
    output.push_str(trimmed);
    if let Some(url) = fallback_url {
        output.push_str(&format!(" <{url}>"));
    }
    output.push_str(trailing);
}

/// plain fallback用のURL（§33）。
///
/// 同一URLのspanが行内に複数ある場合は、最後の出現箇所にのみ付ける。
fn fallback_url<'a>(
    span: &'a LayoutSpan,
    line: &'a LayoutLine,
    position: usize,
) -> Option<&'a str> {
    let url = span.link.as_ref().map(|l| l.url.as_str())?;
    let is_last_occurrence = !line
        .spans
        .iter()
        .skip(position + 1)
        .any(|other| other.link.as_ref().map(|l| l.url.as_str()) == Some(url));
    is_last_occurrence.then_some(url)
}

/// OSC 8 開始sequence（§33）: `ESC ] 8 ; ; URL ST`
fn osc8_open(url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\")
}

/// OSC 8 終了sequence（§33）: `ESC ] 8 ; ; ST`
const OSC8_CLOSE: &str = "\x1b]8;;\x1b\\";

/// 水平罫線（§11 HorizontalRule）。styleは§19の `Border`。
fn write_rule_line(output: &mut String, width: usize, options: &RenderOptions) {
    push_margin(output, options.margin);
    let content: String = std::iter::repeat_n('─', width).collect();
    match options.color_level {
        ColorLevel::None => output.push_str(&content),
        level => {
            let sequence = style::sgr_sequence(&options.theme.spec(SemanticStyle::Border), level);
            if sequence.is_empty() {
                output.push_str(&content);
            } else {
                output.push_str(&sequence);
                output.push_str(&content);
                output.push_str(RESET);
            }
        }
    }
    output.push('\n');
}

/// コードblock（§28）。`╭─ 言語 ─` 枠で囲み、中身はhighlightして出す。
///
/// 本文は折り返さない（S3-1）。色が無効な場合はhighlightも行わない。
fn write_code_block(output: &mut String, code: &mdsee_layout::CodeLayout, options: &RenderOptions) {
    use unicode_width::UnicodeWidthStr;

    // 上枠: ╭─ rust ─────（言語がなければ ╭────）
    push_margin(output, options.margin);
    match options.color_level {
        ColorLevel::None => match &code.language {
            Some(language) => {
                let prefix = format!("╭─ {language} ");
                let fill = code.width.saturating_sub(prefix.width() + 1);
                output.push_str(&prefix);
                output.push_str(&"─".repeat(fill));
                output.push('─');
            }
            None => {
                output.push('╭');
                output.push_str(&"─".repeat(code.width.saturating_sub(1)));
            }
        },
        level => {
            let border = style::sgr_sequence(&options.theme.spec(SemanticStyle::Border), level);
            output.push_str(&border);
            match &code.language {
                Some(language) => {
                    let label_width = UnicodeWidthStr::width(language.as_str());
                    // "╭─ " + label + " " + "─" * (width - 3 - label - 1)
                    let fill = code.width.saturating_sub(3 + label_width + 1).max(1);
                    output.push_str("╭─ ");
                    output.push_str(RESET);
                    let label =
                        style::sgr_sequence(&options.theme.spec(SemanticStyle::Muted), level);
                    output.push_str(&label);
                    output.push_str(language);
                    output.push_str(RESET);
                    output.push_str(&border);
                    output.push(' ');
                    output.push_str(&"─".repeat(fill));
                    output.push_str(RESET);
                }
                None => {
                    output.push('╭');
                    output.push_str(&"─".repeat(code.width.saturating_sub(1)));
                    output.push_str(RESET);
                }
            }
        }
    }
    output.push('\n');

    // 本文行: │ + highlight済みspan
    let highlighted = highlight_code(code, options);
    for (index, line) in code.lines.iter().enumerate() {
        push_margin(output, options.margin);
        match options.color_level {
            ColorLevel::None => {
                if line.is_empty() {
                    output.push('│');
                } else {
                    output.push_str("│ ");
                    output.push_str(line);
                }
            }
            level => {
                let border = style::sgr_sequence(&options.theme.spec(SemanticStyle::Border), level);
                output.push_str(&border);
                output.push('│');
                output.push_str(RESET);
                if line.is_empty() {
                    output.push('\n');
                    continue;
                }
                output.push(' ');
                match highlighted.as_ref().and_then(|lines| lines.get(index)) {
                    Some(hl) if !hl.spans.is_empty() => {
                        for span in &hl.spans {
                            let sequence = style::sgr_sequence(
                                &style::StyleSpec {
                                    fg: Some(span.fg),
                                    bold: span.bold,
                                    italic: span.italic,
                                    underline: span.underline,
                                    strike: false,
                                },
                                level,
                            );
                            if sequence.is_empty() {
                                output.push_str(&span.text);
                            } else {
                                output.push_str(&sequence);
                                output.push_str(&span.text);
                                output.push_str(RESET);
                            }
                        }
                    }
                    _ => {
                        let sequence =
                            style::sgr_sequence(&options.theme.spec(SemanticStyle::Code), level);
                        if sequence.is_empty() {
                            output.push_str(line);
                        } else {
                            output.push_str(&sequence);
                            output.push_str(line);
                            output.push_str(RESET);
                        }
                    }
                }
            }
        }
        output.push('\n');
    }

    // 下枠: ╰────
    push_margin(output, options.margin);
    match options.color_level {
        ColorLevel::None => {
            output.push('╰');
            output.push_str(&"─".repeat(code.width.saturating_sub(1)));
        }
        level => {
            let border = style::sgr_sequence(&options.theme.spec(SemanticStyle::Border), level);
            output.push_str(&border);
            output.push('╰');
            output.push_str(&"─".repeat(code.width.saturating_sub(1)));
            output.push_str(RESET);
        }
    }
    output.push('\n');
}

/// code blockのhighlight。色無効またはfeature構成によりhighlightできない
/// 場合は `None`（§29: rendererはtrait越しにのみ使う）。
fn highlight_code(
    code: &mdsee_layout::CodeLayout,
    options: &RenderOptions,
) -> Option<Vec<HighlightedLine>> {
    if options.color_level == ColorLevel::None {
        return None;
    }
    #[cfg(feature = "syntax")]
    {
        let highlighter: &dyn SyntaxHighlighter =
            &SyntectHighlighter::new(options.theme.syntax_theme.clone());
        let mut source = code.lines.join("\n");
        if !source.is_empty() {
            source.push('\n');
        }
        let lines = highlighter.highlight(&source, code.language.as_deref());
        Some(lines)
    }
    #[cfg(not(feature = "syntax"))]
    {
        let highlighter: &dyn SyntaxHighlighter = &NoHighlight;
        let lines = highlighter.highlight(&code.lines.join("\n"), code.language.as_deref());
        Some(lines)
    }
}

fn push_margin(output: &mut String, margin: u16) {
    for _ in 0..margin {
        output.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use mdsee_layout::{LayoutLine, LayoutSpan, LinkTarget, RuleLayout, SemanticStyle, TextBlock};

    use super::*;

    fn span(content: &str, style: SemanticStyle) -> LayoutSpan {
        LayoutSpan {
            content: content.to_string(),
            style,
            link: None,
        }
    }

    fn document_with(lines: Vec<LayoutLine>) -> LayoutDocument {
        LayoutDocument {
            blocks: vec![LayoutBlock::Text(TextBlock { lines })],
        }
    }

    fn render_to(document: &LayoutDocument, options: &RenderOptions) -> String {
        let mut buffer: Vec<u8> = Vec::new();
        render(document, &mut buffer, options).unwrap();
        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn plain_rendering_has_no_escape_sequences() {
        let document = document_with(vec![LayoutLine {
            spans: vec![
                span("hello ", SemanticStyle::Body),
                span("world", SemanticStyle::Strong),
            ],
        }]);
        let output = render_to(&document, &RenderOptions::default());
        assert_eq!(output, "  hello world\n");
        assert!(!output.contains('\x1b'));
    }

    #[test]
    fn truecolor_rendering_emits_ansi() {
        let document = document_with(vec![LayoutLine {
            spans: vec![span("hi", SemanticStyle::InlineCode)],
        }]);
        let options = RenderOptions {
            color_level: ColorLevel::TrueColor,
            margin: 0,
            osc8: false,
            theme: Theme::dark(),
        };
        let output = render_to(&document, &options);
        assert_eq!(output, "\x1b[38;2;79;193;233mhi\x1b[0m\n");
    }

    #[test]
    fn blocks_are_separated_by_blank_line() {
        let document = LayoutDocument {
            blocks: vec![
                LayoutBlock::Text(TextBlock {
                    lines: vec![LayoutLine {
                        spans: vec![span("one", SemanticStyle::Body)],
                    }],
                }),
                LayoutBlock::Text(TextBlock {
                    lines: vec![LayoutLine {
                        spans: vec![span("two", SemanticStyle::Body)],
                    }],
                }),
            ],
        };
        let output = render_to(&document, &RenderOptions::default());
        assert_eq!(output, "  one\n\n  two\n");
    }

    #[test]
    fn blank_line_has_no_trailing_margin() {
        let document = document_with(vec![
            LayoutLine {
                spans: vec![span("a", SemanticStyle::Body)],
            },
            LayoutLine { spans: vec![] },
            LayoutLine {
                spans: vec![span("b", SemanticStyle::Body)],
            },
        ]);
        let output = render_to(&document, &RenderOptions::default());
        assert_eq!(output, "  a\n\n  b\n");
    }

    #[test]
    fn code_block_renders_frame_and_highlighted_lines() {
        let document = LayoutDocument {
            blocks: vec![LayoutBlock::Code(mdsee_layout::CodeLayout {
                language: Some("rust".to_string()),
                lines: vec!["fn main() {}".to_string()],
                width: 20,
            })],
        };
        let options = RenderOptions {
            color_level: ColorLevel::Ansi256,
            margin: 0,
            osc8: false,
            theme: Theme::dark(),
        };
        let output = render_to(&document, &options);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3);
        // 上枠は ╭─ rust ─…。言語label付き
        assert!(lines[0].starts_with("\x1b[38;5;"));
        assert!(lines[0].contains('╭'));
        assert!(lines[0].contains("rust"));
        // 本文行は │ prefixを持ち、highlight色（syntect由来）で分割される。
        // escapeを除去すると元のコード行に戻る
        let stripped = strip_sgr(lines[1]);
        assert_eq!(stripped, "│ fn main() {}");
        // 下枠
        assert!(lines[2].contains('╰'));
        assert_eq!(
            lines[2],
            format!(
                "\x1b[38;5;{}m╰{}─\x1b[0m",
                style::rgb_to_256(style::Rgb(48, 54, 61)),
                "─".repeat(18)
            )
        );
    }

    /// SGR sequence（\x1b[...m）を除去する。test専用の簡易版。
    fn strip_sgr(input: &str) -> String {
        let mut out = String::new();
        let mut chars = input.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' && chars.peek() == Some(&'[') {
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn plain_code_block_renders_frame_without_ansi() {
        let document = LayoutDocument {
            blocks: vec![LayoutBlock::Code(mdsee_layout::CodeLayout {
                language: Some("sh".to_string()),
                lines: vec!["ls -la".to_string(), String::new()],
                width: 12,
            })],
        };
        let output = render_to(&document, &RenderOptions::default());
        assert_eq!(output, "  ╭─ sh ──────\n  │ ls -la\n  │\n  ╰───────────\n");
        assert!(!output.contains('\x1b'));
    }

    #[test]
    fn no_language_code_block_has_plain_frame() {
        let document = LayoutDocument {
            blocks: vec![LayoutBlock::Code(mdsee_layout::CodeLayout {
                language: None,
                lines: vec!["x".to_string()],
                width: 6,
            })],
        };
        let output = render_to(&document, &RenderOptions::default());
        assert_eq!(output, "  ╭─────\n  │ x\n  ╰─────\n");
    }

    // ---- Sprint 2（S2-4, S2-3） ----

    fn link_span(content: &str, url: &str) -> LayoutSpan {
        LayoutSpan {
            content: content.to_string(),
            style: SemanticStyle::Link,
            link: Some(LinkTarget {
                url: url.to_string(),
            }),
        }
    }

    #[test]
    fn osc8_wraps_link_text() {
        // §33: ESC ] 8 ; ; URL ST text ESC ] 8 ; ; ST
        let document = document_with(vec![LayoutLine {
            spans: vec![link_span("site", "https://example.com")],
        }]);
        let options = RenderOptions {
            color_level: ColorLevel::TrueColor,
            margin: 0,
            osc8: true,
            theme: Theme::dark(),
        };
        let output = render_to(&document, &options);
        assert_eq!(
            output,
            "\x1b[4;38;2;88;166;255m\x1b]8;;https://example.com\x1b\\site\x1b]8;;\x1b\\\x1b[0m\n"
        );
    }

    #[test]
    fn plain_fallback_appends_url_in_angle_brackets() {
        // §33: plain fallbackは `text <URL>`
        let document = document_with(vec![LayoutLine {
            spans: vec![
                span("see ", SemanticStyle::Body),
                link_span("docs", "https://example.com/docs"),
            ],
        }]);
        let output = render_to(&document, &RenderOptions::default());
        assert_eq!(output, "  see docs <https://example.com/docs>\n");
    }

    #[test]
    fn plain_fallback_keeps_trailing_space_after_url() {
        // link spanの末尾空白はURLの後に置く（空白の二重化を防ぐ）
        let document = document_with(vec![LayoutLine {
            spans: vec![
                span("see ", SemanticStyle::Body),
                link_span("docs ", "https://example.com"),
                span("here", SemanticStyle::Body),
            ],
        }]);
        let output = render_to(&document, &RenderOptions::default());
        assert_eq!(output, "  see docs <https://example.com> here\n");
    }

    #[test]
    fn same_url_repeated_in_line_prints_fallback_once() {
        let document = document_with(vec![LayoutLine {
            spans: vec![
                link_span("a", "https://x"),
                span(" ", SemanticStyle::Body),
                link_span("b", "https://x"),
            ],
        }]);
        let output = render_to(&document, &RenderOptions::default());
        assert_eq!(output, "  a b <https://x>\n");
    }

    #[test]
    fn different_urls_each_get_fallback() {
        let document = document_with(vec![LayoutLine {
            spans: vec![
                link_span("a", "https://x"),
                span(" ", SemanticStyle::Body),
                link_span("b", "https://y"),
            ],
        }]);
        let output = render_to(&document, &RenderOptions::default());
        assert_eq!(output, "  a <https://x> b <https://y>\n");
    }

    #[test]
    fn rule_block_renders_full_width_line_with_border_style() {
        let document = LayoutDocument {
            blocks: vec![LayoutBlock::Rule(RuleLayout { width: 5 })],
        };
        let options = RenderOptions {
            color_level: ColorLevel::TrueColor,
            margin: 0,
            osc8: false,
            theme: Theme::dark(),
        };
        let output = render_to(&document, &options);
        assert_eq!(output, "\x1b[38;2;48;54;61m─────\x1b[0m\n");
    }
}
