//! Table layout engine（design.md §30〜§32）。
//!
//! アルゴリズム（§30）:
//! 1. 各セルのminimum widthを取得
//! 2. preferred widthを取得
//! 3. columnごとのpreferred width決定
//! 4. 全幅がcontent_widthを超える場合縮小
//! 5. 各cellをwrap
//! 6. row height決定
//! 7. border描画
//!
//! 縮小時は `preferred - min` の余裕が大きいcolumnから削る（§31）。

use mdsee_core::{Alignment, Inline, Link, Table, TableCell};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::model::{LayoutBlock, LayoutLine, LayoutSpan, SemanticStyle, TableLayout};
use crate::wrap::{is_cjk_breakable, wrap_inlines};

/// 表layout engine（§30）。独立moduleにする設計に従う。
#[derive(Debug, Clone, Copy, Default)]
pub struct TableLayoutEngine;

impl TableLayoutEngine {
    /// Table ASTを幅 `width` に収まる表へlayoutする。
    pub fn layout(table: &Table, width: usize) -> TableLayout {
        let ncols = table
            .alignments
            .len()
            .max(table.header.len())
            .max(table.rows.iter().map(Vec::len).max().unwrap_or(0));
        if ncols == 0 {
            return TableLayout { lines: Vec::new() };
        }

        // 行を正規化（列数に満たないcellは空で埋める）
        let alignments = normalize_alignments(&table.alignments, ncols);
        let header = normalize_row(&table.header, ncols);
        let rows: Vec<Vec<&TableCell>> = table
            .rows
            .iter()
            .map(|row| normalize_row(row, ncols))
            .collect();

        // (1)(2) 各columnのmin / preferred
        let mut min = vec![0usize; ncols];
        let mut preferred = vec![0usize; ncols];
        let all_rows = std::iter::once(&header).chain(rows.iter());
        for row in all_rows {
            for (col, cell) in row.iter().enumerate() {
                min[col] = min[col].max(cell_min_width(&cell.inlines));
                preferred[col] = preferred[col].max(cell_preferred_width(&cell.inlines));
            }
        }

        // (3)(4) assigned決定。区切りを除いた列幅の合計は avail を超えない
        let overhead = 3 * ncols + 1; // "│ x │ y │" の罫線と空白
        let avail = width.saturating_sub(overhead).max(ncols);
        let mut assigned: Vec<usize> = preferred.iter().map(|p| (*p).min(avail)).collect();
        shrink_to_fit(&mut assigned, &min, avail);

        // (5)(6) 各cellをwrapし、行高を揃えて行を合成
        let header_cells = layout_row_cells(&header, &assigned, SemanticStyle::Strong);
        let header_lines =
            compose_rows(std::slice::from_ref(&header_cells), &alignments, &assigned);
        let body_cells: Vec<Vec<Vec<LayoutLine>>> = rows
            .iter()
            .map(|row| layout_row_cells(row, &assigned, SemanticStyle::Body))
            .collect();

        // (7) border描画
        let mut lines = Vec::new();
        lines.push(separator_line(&assigned, '┌', '┬', '┐'));
        if !table.header.is_empty() {
            lines.extend(header_lines);
            lines.push(separator_line(&assigned, '├', '┼', '┤'));
        }
        lines.extend(compose_rows(&body_cells, &alignments, &assigned));
        lines.push(separator_line(&assigned, '└', '┴', '┘'));

        TableLayout { lines }
    }
}

/// Table blockを `LayoutBlock::Table` へ変換するentry point。
pub(crate) fn layout_table(table: &Table, width: usize) -> LayoutBlock {
    LayoutBlock::Table(TableLayoutEngine::layout(table, width))
}

fn normalize_alignments(alignments: &[Alignment], ncols: usize) -> Vec<Alignment> {
    let mut out: Vec<Alignment> = alignments.to_vec();
    out.resize(ncols, Alignment::None);
    out
}

fn normalize_row(row: &[TableCell], ncols: usize) -> Vec<&TableCell> {
    let mut out: Vec<&TableCell> = row.iter().collect();
    static EMPTY: TableCell = TableCell {
        inlines: Vec::new(),
    };
    while out.len() < ncols {
        out.push(&EMPTY);
    }
    out
}

/// §31: 余裕（assigned - min）が大きいcolumnから1列ずつ削る。
fn shrink_to_fit(assigned: &mut [usize], min: &[usize], avail: usize) {
    while assigned.iter().sum::<usize>() > avail {
        let target = assigned
            .iter()
            .zip(min)
            .enumerate()
            .filter(|(_, (a, m))| **a > **m)
            .max_by_key(|(_, (a, m))| **a - **m)
            .map(|(index, _)| index);
        match target {
            Some(index) => assigned[index] -= 1,
            None => break,
        }
    }
    // min まで削っても収まらない場合は1列分まで強制する（hard split）
    while assigned.iter().sum::<usize>() > avail {
        let target = assigned
            .iter()
            .enumerate()
            .filter(|(_, a)| **a > 1)
            .max_by_key(|(_, a)| **a)
            .map(|(index, _)| index);
        match target {
            Some(index) => assigned[index] -= 1,
            None => break,
        }
    }
}

/// セルのminimum width（§30 step 1）。
///
/// 空白とCJK境界で区切った最長runの幅。英語のwordは切れない前提の幅で、
/// CJK連続は1文字ずつ切れるため最大grapheme幅になる。
fn cell_min_width(inlines: &[Inline]) -> usize {
    let text = plain_text(inlines);
    let mut max_run = 1usize;
    let mut run = 0usize;
    let mut prev_breakable = false;
    for cluster in text.graphemes(true) {
        if cluster.trim().is_empty() {
            max_run = max_run.max(run);
            run = 0;
            prev_breakable = false;
            continue;
        }
        let starts_breakable = cluster.chars().any(is_cjk_breakable);
        if run > 0 && (prev_breakable || starts_breakable) {
            max_run = max_run.max(run);
            run = 0;
        }
        run += UnicodeWidthStr::width(cluster);
        prev_breakable = starts_breakable;
    }
    max_run.max(run)
}

/// セルのpreferred width（§30 step 2）。1行で表示した場合の幅。
fn cell_preferred_width(inlines: &[Inline]) -> usize {
    let text = plain_text(inlines);
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return 0;
    }
    words
        .iter()
        .map(|w| UnicodeWidthStr::width(*w))
        .sum::<usize>()
        + words.len()
        - 1
}

/// inline列の平文。表示幅計算のみに使う。
pub(crate) fn plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    fn walk(inlines: &[Inline], out: &mut String) {
        for inline in inlines {
            match inline {
                Inline::Text(run) => out.push_str(&run.content),
                Inline::Code(code) => out.push_str(code),
                Inline::SoftBreak | Inline::HardBreak => out.push(' '),
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

fn layout_row_cells(
    row: &[&TableCell],
    assigned: &[usize],
    base: SemanticStyle,
) -> Vec<Vec<LayoutLine>> {
    row.iter()
        .enumerate()
        .map(|(col, cell)| wrap_inlines(&cell.inlines, assigned[col].max(1), base))
        .collect()
}

/// wrap済みcell列を行へ合成する。行高は各行の最大（§30 step 6）。
fn compose_rows(
    rows: &[Vec<Vec<LayoutLine>>],
    alignments: &[Alignment],
    assigned: &[usize],
) -> Vec<LayoutLine> {
    let mut lines = Vec::new();
    for cells in rows {
        let height = cells.iter().map(Vec::len).max().unwrap_or(0);
        for line_index in 0..height {
            let mut spans = vec![border_span("│")];
            for (col, cell) in cells.iter().enumerate() {
                spans.push(space_span(1));
                match cell.get(line_index) {
                    Some(line) => {
                        spans.extend(pad_cell_line(line.clone(), assigned[col], alignments[col]))
                    }
                    None => spans.push(space_span(assigned[col])),
                }
                spans.push(space_span(1));
                spans.push(border_span("│"));
            }
            lines.push(LayoutLine { spans });
        }
    }
    lines
}

/// セルの1行をalignmentどおりにpaddingする（§32）。
fn pad_cell_line(mut line: LayoutLine, width: usize, alignment: Alignment) -> Vec<LayoutSpan> {
    let current = line
        .spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_str()))
        .sum::<usize>();
    let pad = width.saturating_sub(current);
    let (left, right) = match alignment {
        Alignment::Left | Alignment::None => (0, pad),
        Alignment::Center => (pad / 2, pad - pad / 2),
        Alignment::Right => (pad, 0),
    };
    let mut spans = Vec::with_capacity(line.spans.len() + 2);
    spans.push(space_span(left));
    spans.append(&mut line.spans);
    spans.push(space_span(right));
    spans
}

fn separator_line(assigned: &[usize], left: char, middle: char, right: char) -> LayoutLine {
    let mut content = String::new();
    content.push(left);
    for (index, width) in assigned.iter().enumerate() {
        content.push_str(&"─".repeat(width + 2));
        if index + 1 < assigned.len() {
            content.push(middle);
        }
    }
    content.push(right);
    LayoutLine {
        spans: vec![LayoutSpan {
            content,
            style: SemanticStyle::Border,
            link: None,
        }],
    }
}

fn border_span(content: &str) -> LayoutSpan {
    LayoutSpan {
        content: content.to_string(),
        style: SemanticStyle::Border,
        link: None,
    }
}

fn space_span(width: usize) -> LayoutSpan {
    LayoutSpan {
        content: " ".repeat(width),
        style: SemanticStyle::Body,
        link: None,
    }
}

#[cfg(test)]
mod tests {
    use mdsee_core::SourceDocument;

    use super::*;
    use crate::LayoutOptions;

    fn parse_table(markdown: &str) -> mdsee_core::Table {
        let source = SourceDocument {
            content: markdown.to_string(),
            origin: mdsee_core::Origin::Stdin {
                cwd: std::env::temp_dir(),
            },
        };
        let document = mdsee_core::parse(&source).unwrap();
        match document.blocks.into_iter().next() {
            Some(mdsee_core::Block::Table(table)) => table,
            other => panic!("expected table, got {other:?}"),
        }
    }

    fn line_texts(layout: &TableLayout) -> Vec<String> {
        layout
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_str())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn simple_table_fits_width() {
        let table = parse_table("| a | b |\n| --- | --- |\n| 1 | 2 |\n");
        let layout = TableLayoutEngine::layout(&table, 40);
        let lines = line_texts(&layout);
        assert_eq!(
            lines,
            [
                "┌───┬───┐",
                "│ a │ b │",
                "├───┼───┤",
                "│ 1 │ 2 │",
                "└───┴───┘",
            ]
        );
    }

    #[test]
    fn alignment_is_applied() {
        let table =
            parse_table("| Left | Center | Right |\n| :--- | :----: | ----: |\n| a | b | c |\n");
        let layout = TableLayoutEngine::layout(&table, 40);
        let lines = line_texts(&layout);
        // header幅（Left=4, Center=6, Right=5）でalignmentどおりにpadされる
        assert!(
            lines[3].contains("│ a    │   b    │     c │"),
            "got: {}",
            lines[3]
        );
    }

    #[test]
    fn wide_columns_shrink_by_slack() {
        // 3列で1列だけ極端に長い。長い列が削られる
        let markdown =
            "| s | verylongcolumnheader | s |\n| --- | --- | --- |\n| a | bbbbbbbbbbbbbbbb | c |\n";
        let table = parse_table(markdown);
        let layout = TableLayoutEngine::layout(&table, 30);
        for line in line_texts(&layout) {
            assert!(
                UnicodeWidthStr::width(line.as_str()) <= 30,
                "line too wide: {line}"
            );
        }
    }

    #[test]
    fn cells_wrap_when_narrow() {
        let markdown =
            "| lang | description |\n| --- | --- |\n| rust | a systems programming language that is safe |\n";
        let table = parse_table(markdown);
        let layout = TableLayoutEngine::layout(&table, 34);
        for line in line_texts(&layout) {
            assert!(
                UnicodeWidthStr::width(line.as_str()) <= 34,
                "line too wide: {line}"
            );
        }
        // ┌ + header + ├ + wrapされたdata行2 + └ = 6行
        assert_eq!(layout.lines.len(), 6);
    }

    #[test]
    fn japanese_table_wraps() {
        // §84: 日本語表
        let markdown = "| 名前 | 説明 |\n| --- | --- |\n| 日本語テキスト | これはとても長い日本語の説明文です |\n";
        let table = parse_table(markdown);
        let layout = TableLayoutEngine::layout(&table, 30);
        for line in line_texts(&layout) {
            assert!(
                UnicodeWidthStr::width(line.as_str()) <= 30,
                "line too wide: {line}"
            );
        }
        // 分断しても連結すると元テキストを保つ
        let joined: String = line_texts(&layout).join("");
        assert!(joined.contains("日本語"));
        assert!(joined.contains("説明"));
    }

    #[test]
    fn empty_table_produces_nothing() {
        let table = Table {
            alignments: vec![],
            header: vec![],
            rows: vec![],
            span: Default::default(),
            id: mdsee_core::BlockId::new(0),
        };
        let layout = TableLayoutEngine::layout(&table, 40);
        assert!(layout.lines.is_empty());
    }

    #[test]
    fn layout_dispatch_produces_table_block() {
        let source = SourceDocument {
            content: "| a |\n| --- |\n| b |\n".to_string(),
            origin: mdsee_core::Origin::Stdin {
                cwd: std::env::temp_dir(),
            },
        };
        let document = mdsee_core::parse(&source).unwrap();
        let laid = crate::layout(&document, &LayoutOptions::default()).unwrap();
        assert!(matches!(laid.blocks[0], LayoutBlock::Table(_)));
    }
}
