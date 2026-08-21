//! AST block → LayoutBlock 変換（design.md §21〜§26, §28）。

use mdsee_core::{Block, BlockQuote, CodeBlock, Heading, List, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::model::{
    CodeLayout, LayoutBlock, LayoutLine, LayoutSpan, RuleLayout, SemanticStyle, TextBlock,
};
use crate::wrap::wrap_inlines;
use crate::LayoutContext;

pub(crate) fn layout_block(block: &Block, ctx: &LayoutContext) -> LayoutBlock {
    layout_block_at(block, ctx, 0)
}

/// `indent` はlist等の入れ子で消費済みの桁数。
fn layout_block_at(block: &Block, ctx: &LayoutContext, indent: usize) -> LayoutBlock {
    let width = available_width(ctx, indent);
    match block {
        Block::Heading(heading) => LayoutBlock::Text(layout_heading(heading, width)),
        Block::Paragraph(paragraph) => LayoutBlock::Text(layout_paragraph(paragraph, width)),
        Block::CodeBlock(code) => LayoutBlock::Code(layout_code_block(code)),
        Block::BlockQuote(quote) => LayoutBlock::Text(layout_blockquote(quote, ctx, indent)),
        Block::List(list) => LayoutBlock::Text(layout_list(list, ctx, indent)),
        Block::HorizontalRule => LayoutBlock::Rule(RuleLayout { width }),
    }
}

/// 入れ子込みで利用可能な本文幅。
fn available_width(ctx: &LayoutContext, indent: usize) -> usize {
    (ctx.content_width as usize).saturating_sub(indent).max(1)
}

fn layout_paragraph(paragraph: &Paragraph, width: usize) -> TextBlock {
    TextBlock {
        lines: wrap_inlines(&paragraph.inlines, width, SemanticStyle::Body),
    }
}

/// 見出しのlayout（§24）。
///
/// H1 は `━` 下線、H2 は `─` 下線、H3以下は `###` 風のprefixを付けて
/// そのまま表示する。H1〜H6全部を派手にしない。
fn layout_heading(heading: &Heading, width: usize) -> TextBlock {
    let style = SemanticStyle::heading(heading.level);

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

/// Blockquote（§26）。
///
/// 子blockを幅 `content_width - 2` でlayoutし、全行に `│ ` を付ける。
/// nested quoteは再帰的に `│ │ ` となる。
fn layout_blockquote(quote: &BlockQuote, ctx: &LayoutContext, indent: usize) -> TextBlock {
    let inner_indent = indent + 2;
    let mut lines = Vec::new();
    // §26: quote内の子block間に空行を挟まず、`│` を連続させる
    for child in &quote.children {
        let inner = layout_block_at(child, ctx, inner_indent);
        match inner {
            LayoutBlock::Text(text_block) => lines.extend(text_block.lines),
            LayoutBlock::Code(code) => {
                for source_line in code.lines {
                    lines.push(plain_line(source_line, SemanticStyle::Code));
                }
            }
            LayoutBlock::Rule(rule) => lines.push(rule_line('─', rule.width)),
        }
    }
    TextBlock {
        lines: lines.into_iter().map(prefix_quote_bar).collect(),
    }
}

/// 行頭に `│ `（Quote style）を付ける。空行には付けない。
fn prefix_quote_bar(line: LayoutLine) -> LayoutLine {
    if line.spans.iter().all(|span| span.content.is_empty()) {
        return line;
    }
    let mut spans = vec![LayoutSpan {
        content: "│ ".to_string(),
        style: SemanticStyle::Quote,
        link: None,
    }];
    spans.extend(line.spans);
    LayoutLine { spans }
}

/// List（§25）。
///
/// bullet `•` / ordered `N.` / task `☐` `☑`。
/// ネストはmarker幅（2桁）ずつ字下げし、§25の
/// ```text
/// • item
///   • child
/// ```
/// に合わせる。tight listでは項目間・項目内に空行を挟まない。
fn layout_list(list: &List, ctx: &LayoutContext, indent: usize) -> TextBlock {
    let mut lines = Vec::new();
    for (index, item) in list.items.iter().enumerate() {
        if index > 0 && !list.tight {
            lines.push(LayoutLine { spans: vec![] });
        }
        layout_list_item(item, list, index, ctx, indent, &mut lines);
    }
    TextBlock { lines }
}

fn layout_list_item(
    item: &mdsee_core::ListItem,
    list: &List,
    index: usize,
    ctx: &LayoutContext,
    indent: usize,
    lines: &mut Vec<LayoutLine>,
) {
    let marker = list_marker(list, item, index);
    let marker_width = UnicodeWidthStr::width(marker.as_str());
    let content_indent = indent + marker_width;

    for (child_index, child) in item.children.iter().enumerate() {
        if child_index > 0 && !list.tight {
            lines.push(LayoutLine { spans: vec![] });
        }
        match child {
            Block::List(nested) => {
                // ネストしたlistはmarkerの下に揃える（§25）
                lines.extend(layout_list(nested, ctx, content_indent).lines);
            }
            other => {
                let inner = layout_block_at(other, ctx, content_indent);
                match inner {
                    LayoutBlock::Text(text_block) => {
                        for (line_index, mut line) in text_block.lines.into_iter().enumerate() {
                            if line.spans.iter().all(|span| span.content.is_empty()) {
                                lines.push(line);
                                continue;
                            }
                            if line_index == 0 && child_index == 0 {
                                let mut prefix = String::new();
                                if indent > 0 {
                                    prefix.push_str(&" ".repeat(indent));
                                }
                                prefix.push_str(&marker);
                                let marker_span = LayoutSpan {
                                    content: prefix,
                                    style: SemanticStyle::Body,
                                    link: None,
                                };
                                line.spans.insert(0, marker_span);
                            } else {
                                line = indent_line(line, indent + marker_width);
                            }
                            lines.push(line);
                        }
                    }
                    LayoutBlock::Code(code) => {
                        for (line_index, source_line) in code.lines.iter().enumerate() {
                            let prefix = if line_index == 0 && child_index == 0 {
                                marker.clone()
                            } else {
                                " ".repeat(content_indent)
                            };
                            let mut line = plain_line(source_line, SemanticStyle::Code);
                            if !prefix.trim().is_empty() {
                                let span = LayoutSpan {
                                    content: prefix,
                                    style: SemanticStyle::Body,
                                    link: None,
                                };
                                line.spans.insert(0, span);
                            }
                            lines.push(line);
                        }
                    }
                    LayoutBlock::Rule(rule) => {
                        let _ = rule;
                        lines.push(indent_line(rule_line('─', 8), content_indent));
                    }
                }
            }
        }
    }
}

/// list marker（§25）。task listは `☐` / `☑`。
fn list_marker(list: &List, item: &mdsee_core::ListItem, index: usize) -> String {
    if let Some(checked) = item.task {
        if checked {
            "☑ ".to_string()
        } else {
            "☐ ".to_string()
        }
    } else if list.ordered {
        format!("{}. ", list.start + index as u64)
    } else {
        "• ".to_string()
    }
}

/// 行頭に `width` 桁の空白を付ける。
fn indent_line(line: LayoutLine, width: usize) -> LayoutLine {
    let mut spans = vec![LayoutSpan {
        content: " ".repeat(width),
        style: SemanticStyle::Body,
        link: None,
    }];
    spans.extend(line.spans);
    LayoutLine { spans }
}

fn plain_line(content: impl Into<String>, style: SemanticStyle) -> LayoutLine {
    LayoutLine {
        spans: vec![LayoutSpan {
            content: content.into(),
            style,
            link: None,
        }],
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
            LayoutBlock::Rule(rule) => vec!["─".repeat(rule.width)],
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

    // ---- Sprint 2（S2-1〜S2-3） ----

    fn parse_and_layout(markdown: &str) -> Vec<String> {
        let source = mdsee_core::SourceDocument {
            content: markdown.to_string(),
            origin: mdsee_core::Origin::Stdin {
                cwd: std::env::temp_dir(),
            },
        };
        let document = mdsee_core::parse(&source).unwrap();
        let options = LayoutOptions {
            terminal_width: 40,
            max_width: 100,
            margin: 2,
        };
        let laid = crate::layout(&document, &options).unwrap();
        let mut out = Vec::new();
        for (i, block) in laid.blocks.iter().enumerate() {
            if i > 0 {
                out.push(String::new());
            }
            out.extend(line_texts(block));
        }
        out
    }

    #[test]
    fn bullet_list_renders_with_markers() {
        let lines = parse_and_layout("- one\n- two\n");
        assert_eq!(lines, vec!["• one", "• two"]);
    }

    #[test]
    fn nested_list_aligns_under_marker() {
        let lines = parse_and_layout("- one\n  - child\n");
        assert_eq!(lines, vec!["• one", "  • child"]);
    }

    #[test]
    fn deeply_nested_list() {
        let lines = parse_and_layout("- a\n  - b\n    - c\n");
        assert_eq!(lines, vec!["• a", "  • b", "    • c"]);
    }

    #[test]
    fn loose_list_inserts_blank_lines() {
        let lines = parse_and_layout("- a\n\n- b\n");
        assert_eq!(lines, vec!["• a", "", "• b"]);
    }

    #[test]
    fn ordered_list_numbers_from_start() {
        let lines = parse_and_layout("3. three\n4. four\n");
        assert_eq!(lines, vec!["3. three", "4. four"]);
    }

    #[test]
    fn task_list_uses_checkboxes() {
        let lines = parse_and_layout("- [ ] todo\n- [x] done\n");
        assert_eq!(lines, vec!["☐ todo", "☑ done"]);
    }

    #[test]
    fn long_list_item_wraps_aligned() {
        // content_width 36 から marker 2桁を引いた34桁でwrapする
        let markdown = "- aaa bbb ccc ddd eee fff ggg hhh iii\n";
        let lines = parse_and_layout(markdown);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "• aaa bbb ccc ddd eee fff ggg hhh");
        assert_eq!(lines[1], "  iii");
    }

    #[test]
    fn blockquote_lines_have_bar_prefix() {
        // soft breakはspaceとしてwrapされる（CommonMarkのtext render標準）
        let lines = parse_and_layout("> quoted text\n> continues here\n");
        assert_eq!(lines, vec!["│ quoted text continues here"]);
    }

    #[test]
    fn blockquote_wraps_within_reduced_width() {
        let markdown = format!("> {}\n", "word ".repeat(10).trim());
        let lines = parse_and_layout(&markdown);
        // content_width 36 - quote bar 2 = 34桁で折れる
        assert!(lines.len() >= 2);
        assert!(lines.iter().all(|l| l.starts_with("│ ")));
    }

    #[test]
    fn nested_blockquote_doubles_bar() {
        let lines = parse_and_layout("> outer\n> > inner\n");
        assert_eq!(lines[0], "│ outer");
        assert!(lines[1].starts_with("│ │ "), "got: {}", lines[1]);
    }

    #[test]
    fn horizontal_rule_is_full_width_line() {
        // blocks: [a, rule, b] → out = ["a", "", rule, "", "b"]
        let lines = parse_and_layout("a\n\n---\n\nb\n");
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[2].chars().count(), 36);
        assert!(lines[2].chars().all(|c| c == '─'));
    }

    #[test]
    fn quote_bar_uses_quote_style() {
        let lines_markdown = "> hi\n";
        let source = mdsee_core::SourceDocument {
            content: lines_markdown.to_string(),
            origin: mdsee_core::Origin::Stdin {
                cwd: std::env::temp_dir(),
            },
        };
        let document = mdsee_core::parse(&source).unwrap();
        let laid = crate::layout(
            &document,
            &LayoutOptions {
                terminal_width: 40,
                max_width: 100,
                margin: 2,
            },
        )
        .unwrap();
        let LayoutBlock::Text(text_block) = &laid.blocks[0] else {
            panic!("expected text block");
        };
        let first_span = &text_block.lines[0].spans[0];
        assert_eq!(first_span.content, "│ ");
        assert_eq!(first_span.style, SemanticStyle::Quote);
    }
}
