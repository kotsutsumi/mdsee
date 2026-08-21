//! mdsee-layout（design.md §5）。
//!
//! Document ASTを表示可能なレイアウトへ変換する。
//! Terminal escape sequenceは生成しない。

mod block;
mod model;
mod wrap;

pub use model::{
    CodeLayout, LayoutBlock, LayoutDocument, LayoutLine, LayoutSpan, LinkTarget, RuleLayout,
    SemanticStyle, TextBlock,
};

use thiserror::Error;

use mdsee_core::Document;

/// layout options（§21, §22）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutOptions {
    pub terminal_width: u16,
    /// 本文の最大幅。デフォルト100（§21）。
    pub max_width: u16,
    /// 左右margin。デフォルト2（§22）。
    pub margin: u16,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            terminal_width: 80,
            max_width: 100,
            margin: 2,
        }
    }
}

/// Layout context（§21）。
///
/// Sprint 1では幅情報のみを保持する。§21の `theme` / `capabilities` は
/// §101の依存方向（layout → terminal 禁止）と両立しないため、
/// 必要になった時点で設計を改訂したうえで導入する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutContext {
    pub terminal_width: u16,
    pub content_width: u16,
}

impl LayoutContext {
    /// `content_width = min(terminal_width - margin * 2, max_width)`（§21）。
    pub fn from_options(options: &LayoutOptions) -> Self {
        let inner = options
            .terminal_width
            .saturating_sub(options.margin.saturating_mul(2));
        let content_width = inner.min(options.max_width).max(1);
        Self {
            terminal_width: options.terminal_width,
            content_width,
        }
    }
}

/// layout error（§66）。
///
/// Sprint 1では発生しないが、pipeline署名（§100）のために定義する。
#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("layout failed")]
    LayoutFailed,
}

/// Document ASTをLayoutDocumentへ変換する（§100 基本pipeline）。
pub fn layout(document: &Document, options: &LayoutOptions) -> Result<LayoutDocument, LayoutError> {
    let ctx = LayoutContext::from_options(options);
    let blocks = document
        .blocks
        .iter()
        .map(|block| block::layout_block(block, &ctx))
        .collect();
    Ok(LayoutDocument { blocks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_width_is_min_of_inner_width_and_max_width() {
        // terminal 80 / margin 2 / max 100 → 76
        let ctx = LayoutContext::from_options(&LayoutOptions {
            terminal_width: 80,
            max_width: 100,
            margin: 2,
        });
        assert_eq!(ctx.content_width, 76);

        // terminal 120 → max_width 100 で頭打ち（§21）
        let ctx = LayoutContext::from_options(&LayoutOptions {
            terminal_width: 120,
            max_width: 100,
            margin: 2,
        });
        assert_eq!(ctx.content_width, 100);
    }

    #[test]
    fn content_width_never_drops_below_one() {
        let ctx = LayoutContext::from_options(&LayoutOptions {
            terminal_width: 0,
            max_width: 100,
            margin: 2,
        });
        assert_eq!(ctx.content_width, 1);
    }
}
