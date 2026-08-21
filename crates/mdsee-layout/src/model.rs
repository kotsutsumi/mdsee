//! Layout model（design.md §16〜§19）。

/// Layout後のドキュメント（§16）。
#[derive(Debug, Clone, Default)]
pub struct LayoutDocument {
    pub blocks: Vec<LayoutBlock>,
}

/// Layout block（§17）。
///
/// Sprint 1では `Text` / `Code` のみ定義する。`Table` / `Image` / `Rule` は
/// 担当Sprint（S3-3, S4-9, S2-3）で追加する。
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutBlock {
    Text(TextBlock),
    Code(CodeLayout),
}

/// テキスト領域（§18）。
#[derive(Debug, Clone, PartialEq)]
pub struct TextBlock {
    pub lines: Vec<LayoutLine>,
}

/// 1論理行（§18）。
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutLine {
    pub spans: Vec<LayoutSpan>,
}

/// 1装飾区切り（§18）。linkはSprint 2（S2-4）でOSC 8に使う。
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutSpan {
    pub content: String,
    pub style: SemanticStyle,
    pub link: Option<LinkTarget>,
}

/// Hyperlink target（§18）。Sprint 1では未使用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkTarget {
    pub url: String,
}

/// コード領域（§17, §28）。
///
/// Sprint 1は枠なしの素朴な表示。枠・言語label・highlight付きの
/// 本格layoutはSprint 3（S3-1, S3-2）で行う。
#[derive(Debug, Clone, PartialEq)]
pub struct CodeLayout {
    pub language: Option<String>,
    pub lines: Vec<String>,
}

/// 意味スタイル（§19）。色は持たず、Theme側で実際の色へ変換する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticStyle {
    Body,
    Muted,

    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Heading6,

    Strong,
    Emphasis,
    Strike,

    InlineCode,

    Link,

    Quote,

    Code,

    AlertNote,
    AlertTip,
    AlertImportant,
    AlertWarning,
    AlertCaution,
}

impl SemanticStyle {
    /// 見出しレベルから対応するstyleを返す（§24）。
    pub fn heading(level: u8) -> Self {
        match level {
            1 => Self::Heading1,
            2 => Self::Heading2,
            3 => Self::Heading3,
            4 => Self::Heading4,
            5 => Self::Heading5,
            _ => Self::Heading6,
        }
    }
}
