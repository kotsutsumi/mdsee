//! AST block → LayoutBlock 変換（design.md §21〜§24, §28）。

use mdsee_core::{Block, CodeBlock, Heading, Paragraph};

use crate::model::{CodeLayout, LayoutBlock, LayoutLine, LayoutSpan, SemanticStyle, TextBlock};
use crate::wrap::wrap_inlines;
use crate::LayoutContext;

pub(crate) fn layout_block(block: &Block, ctx: &LayoutContext) -> LayoutBlock {
    match block {
        Block::Heading(heading) => LayoutBlock::Text(layout_heading(heading, ctx)),
        Block::Paragraph(paragraph) => LayoutBlock::Text(layout_paragraph(paragraph, ctx)),
        Block::CodeBlock(code) => LayoutBlock::Code(layout_code_block(code)),
    }
}

fn layout_paragraph(paragraph: &Paragraph, ctx: &LayoutContext) -> TextBlock {
    TextBlock {
        lines: wrap_inlines(
            &paragraph.inlines,
            ctx.content_width as usize,
            SemanticStyle::Body,
        ),
    }
}

/// 見出しのlayout（§24）。
///
/// H1 は `━` 下線、H2 は `─` 下線、H3以下は `###` 風のprefixを付けて
/// そのまま表示する。H1〜H6全部を派手にしない。
fn layout_heading(heading: &Heading, ctx: &LayoutContext) -> TextBlock {
    let style = SemanticStyle::heading(heading.level);
    let width = ctx.content_width as usize;

    let mut inlines: Vec<mdsee_core::Inline> = Vec::new();
    if heading.level >= 3 {
        let marker = format!("{} ", "#".repeat(heading.level as usize));
        inlines.push(mdsee_core::Inline::Text(mdsee_core::TextRun {
            content: marker,
        }));
    }
    inlines.extend(heading.inlines.iter().cloned());

    let mut lines = wrap_inlines(&inlines, width, style);
    match heading.level {
        // 罫線のstyleは§19のとおりheadingではなくBorderを使う
        1 => lines.push(rule_line('━', width)),
        2 => lines.push(rule_line('─', width)),
        _ => {}
    }
    TextBlock { lines }
}

fn rule_line(ch: char, width: usize) -> LayoutLine {
    let content: String = std::iter::repeat_n(ch, width).collect();
    LayoutLine {
        spans: vec![LayoutSpan {
            content,
            style: SemanticStyle::Border,
            link: None,
        }],
    }
}

/// コードblock（§28）。
///
/// Sprint 1は枠なしの素朴な表示。`╭─` 枠はSprint 3（S3-1）で導入する。
fn layout_code_block(code: &CodeBlock) -> CodeLayout {
    CodeLayout {
        language: code.language.clone(),
        lines: code.source.lines().map(str::to_string).collect(),
    }
}

#[cfg(test)]
mod tests {
    use mdsee_core::{BlockId, Document, Inline, SourceSpan, TextRun};

    use super::*;
    use crate::LayoutOptions;

    fn heading(level: u8, title: &str) -> Block {
        Block::Heading(Heading {
            level,
            inlines: vec![Inline::Text(TextRun {
                content: title.to_string(),
            })],
            span: SourceSpan::default(),
            id: BlockId::new(0),
        })
    }

    fn ctx() -> LayoutContext {
        LayoutContext::from_options(&LayoutOptions {
            terminal_width: 40,
            max_width: 100,
            margin: 2,
        })
    }

    fn line_texts(block: &LayoutBlock) -> Vec<String> {
        match block {
            LayoutBlock::Text(tb) => tb
                .lines
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.as_str()).collect())
                .collect(),
            LayoutBlock::Code(cl) => cl.lines.clone(),
        }
    }

    #[test]
    fn h1_has_heavy_rule_line() {
        let block = layout_block(&heading(1, "MDSEE"), &ctx());
        let lines = line_texts(&block);
        assert_eq!(lines[0], "MDSEE");
        assert_eq!(lines[1].chars().next(), Some('━'));
        assert_eq!(lines[1].chars().count(), 36); // 40 - margin 2*2
    }

    #[test]
    fn rule_lines_use_border_style() {
        // §19: 罫線はheading styleではなくBorderを使う
        for level in [1, 2] {
            let LayoutBlock::Text(text_block) = layout_block(&heading(level, "T"), &ctx()) else {
                panic!("expected text block");
            };
            let rule = &text_block.lines.last().unwrap().spans[0];
            assert_eq!(rule.style, SemanticStyle::Border);
        }
    }

    #[test]
    fn h2_has_light_rule_line() {
        let block = layout_block(&heading(2, "Installation"), &ctx());
        let lines = line_texts(&block);
        assert_eq!(lines[0], "Installation");
        assert_eq!(lines[1].chars().next(), Some('─'));
    }

    #[test]
    fn h3_and_below_keep_marker_prefix() {
        for level in 3..=6 {
            let block = layout_block(&heading(level, "Config"), &ctx());
            let lines = line_texts(&block);
            assert_eq!(lines[0], format!("{} Config", "#".repeat(level as usize)));
            assert_eq!(lines.len(), 1, "H3以下は罫線なし");
        }
    }

    #[test]
    fn code_block_keeps_lines_verbatim() {
        let document = Document {
            blocks: vec![Block::CodeBlock(CodeBlock {
                language: Some("rust".to_string()),
                source: "fn main() {\n    println!(\"hi\");\n}\n".to_string(),
                span: SourceSpan::default(),
                id: BlockId::new(0),
            })],
            metadata: Default::default(),
        };
        let options = LayoutOptions {
            terminal_width: 40,
            max_width: 100,
            margin: 2,
        };
        let laid = crate::layout(&document, &options).unwrap();
        assert_eq!(laid.blocks.len(), 1);
        let LayoutBlock::Code(code) = &laid.blocks[0] else {
            panic!("expected code layout");
        };
        assert_eq!(code.language.as_deref(), Some("rust"));
        assert_eq!(code.lines, ["fn main() {", "    println!(\"hi\");", "}"]);
    }
}
