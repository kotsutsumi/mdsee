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
/// Sprint 3で `Table` / `Alert` を追加。
/// `Image` / `Math` は担当Sprint（S4-3, S7-4）で追加する。
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(Heading),
    Paragraph(Paragraph),
    CodeBlock(CodeBlock),
    BlockQuote(BlockQuote),
    List(List),
    Table(Table),
    HorizontalRule,
    Alert(Alert),
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

/// 引用ブロック（§26）。
#[derive(Debug, Clone, PartialEq)]
pub struct BlockQuote {
    pub children: Vec<Block>,
    pub span: SourceSpan,
    pub id: BlockId,
}

/// リスト（§25）。
#[derive(Debug, Clone, PartialEq)]
pub struct List {
    /// `true` = ordered list。`false` = bullet list。
    pub ordered: bool,
    /// ordered listの開始番号（§25の `1. foo`）。bullet listでは無視する。
    pub start: u64,
    /// tight list（項目間・項目内に空行を挟まない）。§25の表示参照。
    pub tight: bool,
    pub items: Vec<ListItem>,
    pub span: SourceSpan,
    pub id: BlockId,
}

/// リスト項目（§25）。
#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    /// task listのチェック状態（§25）。task listでない場合は `None`。
    pub task: Option<bool>,
    pub children: Vec<Block>,
    pub span: SourceSpan,
    pub id: BlockId,
}

/// 表（§30〜§32）。
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// 各columnのalignment。行方向の長さはcolumn数と一致する。
    pub alignments: Vec<Alignment>,
    /// 先頭行（GFM delimiterより前の行）。
    pub header: Vec<TableCell>,
    /// 本文行。
    pub rows: Vec<Vec<TableCell>>,
    pub span: SourceSpan,
    pub id: BlockId,
}

/// 表のセル（§30）。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableCell {
    pub inlines: Vec<Inline>,
}

/// GFMのcolumn alignment（§32）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// GFM Alert（§27）。
#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub kind: AlertKind,
    /// `> [!NOTE] タイトル` 形式の上書きタイトル。通常は `None`。
    pub title: Option<String>,
    pub children: Vec<Block>,
    pub span: SourceSpan,
    pub id: BlockId,
}

/// Alert種別（§27）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl AlertKind {
    /// 枠に表示する既定タイトル（§27）。
    pub fn default_title(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }
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
