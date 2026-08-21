//! mdsee-render（design.md §5）。
//!
//! Layout TreeからANSI / plain textを生成する。

mod style;

use std::io;

use thiserror::Error;

use mdsee_layout::{LayoutBlock, LayoutDocument, LayoutLine};
use mdsee_terminal::ColorLevel;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    pub color_level: ColorLevel,
    /// 行頭の左margin（§22）。layoutのmarginと同じ値を渡す。
    pub margin: u16,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            color_level: ColorLevel::None,
            margin: 2,
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
            LayoutBlock::Code(code) => {
                for line in &code.lines {
                    write_code_line(&mut output, line, options);
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
            for span in &line.spans {
                output.push_str(&span.content);
            }
        }
        level => {
            for span in &line.spans {
                let sequence = style::sgr_sequence(&style::style_spec(span.style), level);
                if sequence.is_empty() {
                    output.push_str(&span.content);
                } else {
                    output.push_str(&sequence);
                    output.push_str(&span.content);
                    output.push_str(RESET);
                }
            }
        }
    }
    output.push('\n');
}

fn write_code_line(output: &mut String, line: &str, options: &RenderOptions) {
    push_margin(output, options.margin);
    match options.color_level {
        ColorLevel::None => output.push_str(line),
        level => {
            let sequence =
                style::sgr_sequence(&style::style_spec(mdsee_layout::SemanticStyle::Code), level);
            if sequence.is_empty() {
                output.push_str(line);
            } else {
                output.push_str(&sequence);
                output.push_str(line);
                output.push_str(RESET);
            }
        }
    }
    output.push('\n');
}

fn push_margin(output: &mut String, margin: u16) {
    for _ in 0..margin {
        output.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use mdsee_layout::{LayoutLine, LayoutSpan, SemanticStyle, TextBlock};

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
    fn code_lines_are_rendered_with_code_style() {
        let document = LayoutDocument {
            blocks: vec![LayoutBlock::Code(mdsee_layout::CodeLayout {
                language: Some("rust".to_string()),
                lines: vec!["fn main() {}".to_string()],
            })],
        };
        let options = RenderOptions {
            color_level: ColorLevel::Ansi256,
            margin: 0,
        };
        let output = render_to(&document, &options);
        assert_eq!(
            output,
            format!(
                "\x1b[38;5;{}mfn main() {{}}\x1b[0m\n",
                style::rgb_to_256(style::Rgb(166, 173, 186))
            )
        );
    }
}
