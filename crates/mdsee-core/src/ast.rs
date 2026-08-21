//! Internal AST（design.md §10〜§13, §75）。
//!
//! Parser固有のASTをRendererへ漏らさない（§10）。

use std::fmt;

/// Blockの一意ID（§75）。Parse時に連番発行し、
/// Reader・search・TOC・source mapで共通利用する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(u64);

impl BlockId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// ソース上の位置（§13）。行・列は1-based。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceSpan {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

/// ドキュメント単位のmetadata（§10）。
/// front matter等は将来のSprintで拡張する。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentMetadata {
    /// 未使用。将来拡張のために確保している。
    pub title: Option<String>,
}

/// Parse済みドキュメント（§10）。
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub blocks: Vec<Block>,
    pub metadata: DocumentMetadata,
}

/// Block種別（§11）。
///
/// Sprint 1では変換対象のBlockのみ定義する。`BlockQuote` / `List` /
/// `Table` / `HorizontalRule` / `Alert` / `Image` / `Math` は
/// それぞれ担当Sprint（S2-1〜S2-3, S3-3, S3-4, S4-3, S7-4）で追加する。
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(Heading),
    Paragraph(Paragraph),
    CodeBlock(CodeBlock),
}

/// 見出し。
#[derive(Debug, Clone, PartialEq)]
pub struct Heading {
    /// 1〜6。
    pub level: u8,
    pub inlines: Vec<Inline>,
    pub span: SourceSpan,
    pub id: BlockId,
}

/// 段落。
#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
    pub inlines: Vec<Inline>,
    pub span: SourceSpan,
    pub id: BlockId,
}

/// コードブロック（§28）。
#[derive(Debug, Clone, PartialEq)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub source: String,
    pub span: SourceSpan,
    pub id: BlockId,
}

/// Inline要素（§12）。
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(TextRun),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Strike(Vec<Inline>),
    Code(String),
    Link(Link),
    SoftBreak,
    HardBreak,
}

/// 平文テキストの連続。
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub content: String,
}

/// Link（§12, §33）。OSC 8での描画はSprint 2（S2-4）。
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub url: String,
    pub title: Option<String>,
    pub children: Vec<Inline>,
}
