//! Markdown parser（design.md §14, §15）。
//!
//! Comrak ASTを再帰的にInternal ASTへ変換する。

use comrak::nodes::{AstNode, NodeValue, Sourcepos};
use comrak::{parse_document, Arena, Options};

use crate::ast::{
    Block, BlockId, CodeBlock, Document, DocumentMetadata, Heading, Inline, Link, Paragraph,
    SourceSpan, TextRun,
};
use crate::error::ParseError;
use crate::input::SourceDocument;

/// Markdown parser interface（§14）。
pub trait MarkdownParser {
    fn parse(&self, source: &SourceDocument) -> Result<Document, ParseError>;
}

/// comrak実装（§14）。
#[derive(Debug, Clone, Copy, Default)]
pub struct ComrakParser;

impl ComrakParser {
    pub const fn new() -> Self {
        Self
    }
}

impl MarkdownParser for ComrakParser {
    fn parse(&self, source: &SourceDocument) -> Result<Document, ParseError> {
        let arena = Arena::new();
        let options = parser_options();
        let root = parse_document(&arena, &source.content, &options);

        let mut ids = IdGenerator::default();
        let blocks = convert_children(root, &mut ids);
        Ok(Document {
            blocks,
            metadata: DocumentMetadata::default(),
        })
    }
}

/// 基本pipeline用の簡易入口（§100）。
pub fn parse(source: &SourceDocument) -> Result<Document, ParseError> {
    ComrakParser.parse(source)
}

/// comrak拡張の有効化。
///
/// Sprint 1ではstrikethroughのみ（S1-6の変換対象と対応）。
/// table / tasklist / alerts等は担当Sprintで有効化する。
fn parser_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options
}

/// BlockIdの連番発行器（§75）。
#[derive(Default)]
struct IdGenerator {
    next: u64,
}

impl IdGenerator {
    fn issue(&mut self) -> BlockId {
        let id = BlockId::new(self.next);
        self.next += 1;
        id
    }
}

fn convert_children<'a>(node: &'a AstNode<'a>, ids: &mut IdGenerator) -> Vec<Block> {
    node.children()
        .flat_map(|child| convert_block(child, ids))
        .collect()
}

fn convert_block<'a>(node: &'a AstNode<'a>, ids: &mut IdGenerator) -> Vec<Block> {
    let ast = node.data.borrow();
    let span = to_span(&ast.sourcepos);

    match &ast.value {
        NodeValue::Paragraph => vec![Block::Paragraph(Paragraph {
            inlines: convert_inlines(node),
            span,
            id: ids.issue(),
        })],
        NodeValue::Heading(heading) => vec![Block::Heading(Heading {
            level: heading.level,
            inlines: convert_inlines(node),
            span,
            id: ids.issue(),
        })],
        NodeValue::CodeBlock(code) => vec![Block::CodeBlock(CodeBlock {
            language: language_from_info(&code.info),
            source: code.literal.clone(),
            span,
            id: ids.issue(),
        })],
        NodeValue::HtmlBlock(html) => {
            // §15: block HTMLはtagを除去してtext fallbackする。
            let text = strip_html_tags(&html.literal);
            let text = text.trim();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![Block::Paragraph(Paragraph {
                    inlines: vec![Inline::Text(TextRun {
                        content: text.to_string(),
                    })],
                    span,
                    id: ids.issue(),
                })]
            }
        }
        // Sprint 2/3で対応するblock（list / blockquote / thematic break / table /
        // alert等）は、対応までの間は子を透過的に走査してparagraph等を拾う。
        _ => convert_children(node, ids),
    }
}

fn convert_inlines<'a>(node: &'a AstNode<'a>) -> Vec<Inline> {
    let mut out = Vec::new();
    for child in node.children() {
        let ast = child.data.borrow();
        match &ast.value {
            NodeValue::Text(text) => out.push(Inline::Text(TextRun {
                content: text.to_string(),
            })),
            NodeValue::SoftBreak => out.push(Inline::SoftBreak),
            NodeValue::LineBreak => out.push(Inline::HardBreak),
            NodeValue::Code(code) => out.push(Inline::Code(code.literal.clone())),
            NodeValue::Emph => out.push(Inline::Emphasis(convert_inlines(child))),
            NodeValue::Strong => out.push(Inline::Strong(convert_inlines(child))),
            NodeValue::Strikethrough => out.push(Inline::Strike(convert_inlines(child))),
            NodeValue::Link(link) => out.push(Inline::Link(Link {
                url: link.url.clone(),
                title: optional_title(&link.title),
                children: convert_inlines(child),
            })),
            NodeValue::Image(_image) => {
                // §12: inline imageはBlock昇格が設計方針だが、画像描画はv0.2。
                // それまでの間、alt text（children）を失わないようにテキスト化する。
                out.extend(convert_inlines(child));
            }
            NodeValue::HtmlInline(raw) => {
                // §15: inline HTMLはtagを除去してtextだけを残す。
                let text = strip_html_tags(raw);
                if !text.is_empty() {
                    out.push(Inline::Text(TextRun { content: text }));
                }
            }
            _ => out.extend(convert_inlines(child)),
        }
    }
    out
}

fn to_span(sourcepos: &Sourcepos) -> SourceSpan {
    SourceSpan {
        start_line: sourcepos.start.line,
        start_column: sourcepos.start.column,
        end_line: sourcepos.end.line,
        end_column: sourcepos.end.column,
    }
}

/// info string（` ```rust ignore ` の `rust ignore`）から言語名を取り出す。
fn language_from_info(info: &str) -> Option<String> {
    info.split_whitespace().next().map(str::to_string)
}

fn optional_title(title: &str) -> Option<String> {
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

/// HTML tagの簡易除去（§15）。
///
/// `<` から `>` までを読み飛ばす。閉じのない `<` は以降をそのまま残す。
fn strip_html_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' if !in_tag => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => output.push(c),
            _ => {}
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Origin;

    fn parse_md(markdown: &str) -> Document {
        let source = SourceDocument {
            content: markdown.to_string(),
            origin: Origin::Stdin {
                cwd: std::env::temp_dir(),
            },
        };
        parse(&source).unwrap()
    }

    fn plain_text(inlines: &[Inline]) -> String {
        let mut out = String::new();
        fn walk(inlines: &[Inline], out: &mut String) {
            for inline in inlines {
                match inline {
                    Inline::Text(run) => out.push_str(&run.content),
                    Inline::Code(code) => out.push_str(code),
                    Inline::SoftBreak | Inline::HardBreak => out.push('\n'),
                    Inline::Emphasis(children)
                    | Inline::Strong(children)
                    | Inline::Strike(children)
                    | Inline::Link(Link { children, .. }) => walk(children, out),
                }
            }
        }
        walk(inlines, &mut out);
        out
    }

    #[test]
    fn parses_heading_with_level_and_span() {
        let doc = parse_md("# Title\n\n## Sub\n");
        assert_eq!(doc.blocks.len(), 2);
        match &doc.blocks[0] {
            Block::Heading(h) => {
                assert_eq!(h.level, 1);
                assert_eq!(plain_text(&h.inlines), "Title");
                assert_eq!(h.span.start_line, 1);
                assert_eq!(h.id, BlockId::new(0));
            }
            other => panic!("expected heading, got {other:?}"),
        }
        match &doc.blocks[1] {
            Block::Heading(h) => {
                assert_eq!(h.level, 2);
                assert_eq!(h.id, BlockId::new(1));
            }
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn parses_paragraph_with_inline_styles() {
        let doc = parse_md("plain **bold** *italic* ~~strike~~ `code`\n");
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph(p) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(plain_text(&p.inlines), "plain bold italic strike code");
        assert!(p.inlines.iter().any(|i| matches!(i, Inline::Strong(_))));
        assert!(p.inlines.iter().any(|i| matches!(i, Inline::Emphasis(_))));
        assert!(p.inlines.iter().any(|i| matches!(i, Inline::Strike(_))));
        assert!(p.inlines.iter().any(|i| matches!(i, Inline::Code(_))));
    }

    #[test]
    fn parses_softbreak_and_hardbreak() {
        let doc = parse_md("first\nsecond  \nthird\n");
        let Block::Paragraph(p) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(p.inlines.contains(&Inline::SoftBreak));
        assert!(p.inlines.contains(&Inline::HardBreak));
    }

    #[test]
    fn parses_link_url_and_title() {
        let doc = parse_md("[site](https://example.com \"Example\")\n");
        let Block::Paragraph(p) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        let Inline::Link(link) = &p.inlines[0] else {
            panic!("expected link");
        };
        assert_eq!(link.url, "https://example.com");
        assert_eq!(link.title.as_deref(), Some("Example"));
        assert_eq!(plain_text(&link.children), "site");
    }

    #[test]
    fn parses_code_block_language_and_source() {
        let doc = parse_md("```rust ignore\nfn main() {}\n```\n");
        let Block::CodeBlock(cb) = &doc.blocks[0] else {
            panic!("expected code block");
        };
        assert_eq!(cb.language.as_deref(), Some("rust"));
        assert_eq!(cb.source, "fn main() {}\n");
    }

    #[test]
    fn block_html_falls_back_to_stripped_text() {
        let doc = parse_md("<details>\n<summary>Hint</summary>\nbody text\n</details>\n");
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph(p) = &doc.blocks[0] else {
            panic!("expected paragraph fallback");
        };
        assert_eq!(plain_text(&p.inlines), "Hint\nbody text");
    }

    #[test]
    fn inline_html_is_stripped_to_text() {
        let doc = parse_md("a <b>bold</b> c\n");
        let Block::Paragraph(p) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(plain_text(&p.inlines), "a bold c");
    }

    #[test]
    fn image_falls_back_to_alt_text() {
        let doc = parse_md("![logo](images/logo.png)\n");
        let Block::Paragraph(p) = &doc.blocks[0] else {
            panic!("expected paragraph");
        };
        assert_eq!(plain_text(&p.inlines), "logo");
    }

    #[test]
    fn block_ids_are_sequential() {
        let doc = parse_md("# a\n\ntext\n\n```sh\nls\n```\n");
        let ids: Vec<BlockId> = doc
            .blocks
            .iter()
            .map(|b| match b {
                Block::Heading(h) => h.id,
                Block::Paragraph(p) => p.id,
                Block::CodeBlock(c) => c.id,
            })
            .collect();
        assert_eq!(ids, vec![BlockId::new(0), BlockId::new(1), BlockId::new(2)]);
    }

    #[test]
    fn unsupported_blocks_are_transparent() {
        // listはSprint 2（S2-1）で対応。それまでの間は子paragraphが透過される。
        let doc = parse_md("- one\n- two\n");
        assert_eq!(doc.blocks.len(), 2);
        let Block::Paragraph(p) = &doc.blocks[0] else {
            panic!("expected transparent paragraph");
        };
        assert_eq!(plain_text(&p.inlines), "one");
    }
}
