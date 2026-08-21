//! 文字列wrap（design.md §20, §23）。
//!
//! word boundary（英語）と grapheme boundary（CJK）を両立し、
//! 収まらない長い語（URL等）は grapheme 単位の soft break で折る。
//!
//! 表示幅の計算に `str::len()` は使わない（§20）。

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use mdsee_core::Inline;

use crate::model::{LayoutLine, LayoutSpan, SemanticStyle};

/// paragraph等のinline列を `width` 桁に折り返す。
pub(crate) fn wrap_inlines(
    inlines: &[Inline],
    width: usize,
    base: SemanticStyle,
) -> Vec<LayoutLine> {
    let mut flat = Vec::new();
    flatten(inlines, &base, &mut flat);
    let tokens = tokenize(flat);

    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current: Vec<Segment> = Vec::new();
    let mut current_width = 0usize;
    let mut pending_space = false;

    for token in tokens {
        match token {
            Token::Space => {
                if !current.is_empty() {
                    pending_space = true;
                }
            }
            Token::ForcedBreak => {
                lines.push(finish_line(std::mem::take(&mut current)));
                current_width = 0;
                pending_space = false;
            }
            Token::Word(segments) => {
                for segment in segments {
                    let separator = usize::from(pending_space && !current.is_empty());
                    if current_width + separator + segment.width <= width {
                        if separator == 1 {
                            append_space(&mut current);
                            current_width += 1;
                        }
                        current_width += segment.width;
                        current.push(segment);
                        pending_space = false;
                    } else {
                        if !current.is_empty() {
                            lines.push(finish_line(std::mem::take(&mut current)));
                        }
                        pending_space = false;
                        if segment.width <= width {
                            current_width = segment.width;
                            current.push(segment);
                        } else {
                            // §23: 行に収まらない長い語（URL等）は
                            // grapheme単位で折る（soft break possible）
                            for chunk in split_hard(segment, width) {
                                current.push(chunk);
                                lines.push(finish_line(std::mem::take(&mut current)));
                            }
                            current_width = 0;
                        }
                    }
                }
            }
        }
    }

    if !current.is_empty() {
        lines.push(finish_line(current));
    }
    lines
}

/// flatten中間表現。grapheme単位の表示要素。
enum FlatToken {
    Glyph {
        cluster: String,
        width: usize,
        style: SemanticStyle,
    },
    /// 折り返し可能な空白（通常テキスト内）。
    Space,
    /// `HardBreak`による強制改行。
    ForcedBreak,
}

/// tokenize後のword単位。break可能点で切られたsegmentの列。
enum Token {
    Word(Vec<Segment>),
    Space,
    ForcedBreak,
}

#[derive(Debug, Clone, PartialEq)]
struct Segment {
    text: String,
    width: usize,
    style: SemanticStyle,
}

/// inlineツリーをflat token列へ展開する。
///
/// styleは最も内側の装飾を採用する（strong内のemphasis等）。
fn flatten(inlines: &[Inline], style: &SemanticStyle, out: &mut Vec<FlatToken>) {
    for inline in inlines {
        match inline {
            Inline::Text(run) => push_text(&run.content, style, out),
            Inline::Code(literal) => push_code(literal, out),
            Inline::Emphasis(children) => {
                flatten(children, &SemanticStyle::Emphasis, out);
            }
            Inline::Strong(children) => {
                flatten(children, &SemanticStyle::Strong, out);
            }
            Inline::Strike(children) => {
                flatten(children, &SemanticStyle::Strike, out);
            }
            Inline::Link(link) => {
                flatten(&link.children, &SemanticStyle::Link, out);
            }
            Inline::SoftBreak => out.push(FlatToken::Space),
            Inline::HardBreak => out.push(FlatToken::ForcedBreak),
        }
    }
}

fn push_text(content: &str, style: &SemanticStyle, out: &mut Vec<FlatToken>) {
    for cluster in content.graphemes(true) {
        if cluster == " " {
            out.push(FlatToken::Space);
        } else {
            let cluster = if cluster == "\t" { " " } else { cluster };
            out.push(FlatToken::Glyph {
                cluster: cluster.to_string(),
                width: UnicodeWidthStr::width(cluster),
                style: *style,
            });
        }
    }
}

/// inline code内の空白はcollapseしない（glyphとして扱う）。
fn push_code(literal: &str, out: &mut Vec<FlatToken>) {
    for cluster in literal.graphemes(true) {
        let cluster = if cluster == "\t" { " " } else { cluster };
        out.push(FlatToken::Glyph {
            cluster: cluster.to_string(),
            width: UnicodeWidthStr::width(cluster),
            style: SemanticStyle::InlineCode,
        });
    }
}

/// flat token列をword単位へ分解する。
///
/// `Space`はwordの区切り。連続する空白は1つに畳み、行頭の空白は除去する。
fn tokenize(flat: Vec<FlatToken>) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut word: Vec<(String, usize, SemanticStyle)> = Vec::new();

    for token in flat {
        match token {
            FlatToken::Glyph {
                cluster,
                width,
                style,
            } => word.push((cluster, width, style)),
            FlatToken::Space => {
                flush_word(&mut word, &mut tokens);
                if !matches!(tokens.last(), Some(Token::Space) | None) {
                    tokens.push(Token::Space);
                }
            }
            FlatToken::ForcedBreak => {
                flush_word(&mut word, &mut tokens);
                tokens.push(Token::ForcedBreak);
            }
        }
    }
    flush_word(&mut word, &mut tokens);
    tokens
}

/// word内のgrapheme列を、break可能境界で分割されたsegment列へ変換する。
///
/// 境界は (1) styleの切り替わり (2) CJK文字の前後（§23 grapheme boundary）。
fn flush_word(word: &mut Vec<(String, usize, SemanticStyle)>, tokens: &mut Vec<Token>) {
    if word.is_empty() {
        return;
    }

    let mut segments: Vec<Segment> = Vec::new();
    let mut prev_ends_breakable = false;

    for (cluster, width, style) in word.drain(..) {
        let starts_breakable = cluster.chars().next().is_some_and(is_cjk_breakable);
        let need_new_segment = match segments.last() {
            None => true,
            Some(last) => last.style != style || prev_ends_breakable || starts_breakable,
        };
        if need_new_segment {
            segments.push(Segment {
                text: String::new(),
                width: 0,
                style,
            });
        }
        let last = segments.last_mut().expect("segment just pushed");
        last.text.push_str(&cluster);
        last.width += width;
        prev_ends_breakable = cluster.chars().next_back().is_some_and(is_cjk_breakable);
    }

    tokens.push(Token::Word(segments));
}

/// CJK相当の文字。前後でgrapheme breakを許可する（§23）。
fn is_cjk_breakable(c: char) -> bool {
    matches!(c,
        '\u{3000}'..='\u{303F}'   // CJK記号・句読点（全角記号）
        | '\u{3040}'..='\u{309F}' // 平仮名
        | '\u{30A0}'..='\u{30FF}' // 片仮名
        | '\u{3400}'..='\u{4DBF}' // CJK拡張A
        | '\u{4E00}'..='\u{9FFF}' // CJK統合漢字
        | '\u{AC00}'..='\u{D7AF}' // ハングル音節
        | '\u{F900}'..='\u{FAFF}' // CJK互換漢字
        | '\u{FF01}'..='\u{FF60}' // 全角形
    )
}

/// 行頭でも収まらないsegmentをgrapheme単位で分割する（§23）。
fn split_hard(segment: Segment, width: usize) -> Vec<Segment> {
    let mut chunks = Vec::new();
    let mut current = Segment {
        text: String::new(),
        width: 0,
        style: segment.style,
    };
    for cluster in segment.text.graphemes(true) {
        let width_of = UnicodeWidthStr::width(cluster);
        if current.width + width_of > width && current.width > 0 {
            chunks.push(std::mem::replace(
                &mut current,
                Segment {
                    text: String::new(),
                    width: 0,
                    style: segment.style,
                },
            ));
        }
        current.text.push_str(cluster);
        current.width += width_of;
    }
    if current.width > 0 || chunks.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// 直前のsegmentに空白を1つ付加する。
fn append_space(current: &mut [Segment]) {
    if let Some(last) = current.last_mut() {
        last.text.push(' ');
        last.width += 1;
    }
}

/// segment列をstyleごとにmergeして1行へする。
fn finish_line(segments: Vec<Segment>) -> LayoutLine {
    let mut spans: Vec<LayoutSpan> = Vec::with_capacity(segments.len());
    for segment in segments {
        match spans.last_mut() {
            Some(last) if last.style == segment.style => {
                last.content.push_str(&segment.text);
            }
            _ => spans.push(LayoutSpan {
                content: segment.text,
                style: segment.style,
                link: None,
            }),
        }
    }
    LayoutLine { spans }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdsee_core::TextRun;

    fn text(content: &str) -> Vec<Inline> {
        vec![Inline::Text(TextRun {
            content: content.to_string(),
        })]
    }

    fn line_texts(lines: &[LayoutLine]) -> Vec<String> {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_str())
                    .collect::<String>()
            })
            .collect()
    }

    fn line_widths(lines: &[LayoutLine]) -> Vec<usize> {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| UnicodeWidthStr::width(s.content.as_str()))
                    .sum()
            })
            .collect()
    }

    #[test]
    fn wraps_english_on_word_boundary() {
        let lines = wrap_inlines(&text("aaa bbb ccc"), 7, SemanticStyle::Body);
        assert_eq!(line_texts(&lines), ["aaa bbb", "ccc"]);
    }

    #[test]
    fn long_word_moves_to_next_line_when_it_fits() {
        // 幅22なら「aa」の次行へ送れば置ける
        let lines = wrap_inlines(&text("aa supercalifragilistic"), 22, SemanticStyle::Body);
        assert_eq!(line_texts(&lines), ["aa", "supercalifragilistic"]);
    }

    #[test]
    fn oversized_word_is_hard_split_at_line_start() {
        // 行頭でも収まらない語はgrapheme単位で折る（§23 soft break）
        let lines = wrap_inlines(&text("aa supercalifragilistic"), 10, SemanticStyle::Body);
        assert_eq!(line_texts(&lines), ["aa", "supercalif", "ragilistic"]);
    }

    #[test]
    fn wraps_japanese_on_grapheme_boundary() {
        let lines = wrap_inlines(
            &text("日本語のテキストはどこでも折り返せる"),
            10,
            SemanticStyle::Body,
        );
        let widths = line_widths(&lines);
        for w in &widths[..widths.len() - 1] {
            assert_eq!(*w, 10, "途中行は幅いっぱい");
        }
        assert!(lines.len() >= 4);
        // 分断結果を連結すると元に戻る
        let joined = line_texts(&lines).join("");
        assert_eq!(joined, "日本語のテキストはどこでも折り返せる");
    }

    #[test]
    fn wraps_mixed_halfwidth_and_fullwidth() {
        let lines = wrap_inlines(&text("abc日本語def"), 5, SemanticStyle::Body);
        let widths = line_widths(&lines);
        assert!(widths.iter().all(|w| *w <= 5));
        assert_eq!(line_texts(&lines).join(""), "abc日本語def");
    }

    #[test]
    fn emoji_counts_as_two_columns() {
        let lines = wrap_inlines(&text("🙂🙂🙂🙂"), 4, SemanticStyle::Body);
        assert_eq!(line_texts(&lines), ["🙂🙂", "🙂🙂"]);
    }

    #[test]
    fn zwj_family_is_one_cluster() {
        // 👨‍💻 (ZWJ sequence) は1 graphemeとして分断されない
        let lines = wrap_inlines(&text("👨‍💻abc"), 3, SemanticStyle::Body);
        let texts = line_texts(&lines);
        assert!(!texts.iter().any(|t| t.contains('a') && !t.contains("👨‍💻")));
        assert_eq!(texts.join(""), "👨‍💻abc");
    }

    #[test]
    fn combining_character_is_one_cluster() {
        // e + U+0301 (combining acute) は1 grapheme・幅1として扱う
        let lines = wrap_inlines(
            &text("e\u{301}e\u{301}e\u{301}e\u{301}"),
            4,
            SemanticStyle::Body,
        );
        assert_eq!(line_texts(&lines), ["e\u{301}e\u{301}e\u{301}e\u{301}"]);
    }

    #[test]
    fn long_url_is_soft_broken() {
        let url = "https://example.com/very/long/path/that/never/fits";
        let lines = wrap_inlines(&text(url), 10, SemanticStyle::Body);
        let widths = line_widths(&lines);
        assert!(widths.iter().all(|w| *w <= 10));
        assert_eq!(line_texts(&lines).join(""), url);
    }

    #[test]
    fn hard_break_splits_line() {
        let inlines = vec![
            Inline::Text(TextRun {
                content: "first".to_string(),
            }),
            Inline::HardBreak,
            Inline::Text(TextRun {
                content: "second".to_string(),
            }),
        ];
        let lines = wrap_inlines(&inlines, 40, SemanticStyle::Body);
        assert_eq!(line_texts(&lines), ["first", "second"]);
    }

    #[test]
    fn leading_and_trailing_spaces_are_dropped() {
        let lines = wrap_inlines(&text("  hello   world  "), 40, SemanticStyle::Body);
        assert_eq!(line_texts(&lines), ["hello world"]);
    }

    #[test]
    fn inline_code_keeps_inner_spaces() {
        let inlines = vec![
            Inline::Text(TextRun {
                content: "x ".to_string(),
            }),
            Inline::Code("a  b".to_string()),
        ];
        let lines = wrap_inlines(&inlines, 40, SemanticStyle::Body);
        assert_eq!(line_texts(&lines), ["x a  b"]);
    }

    #[test]
    fn spans_carry_semantic_styles() {
        let inlines = vec![
            Inline::Text(TextRun {
                content: "plain ".to_string(),
            }),
            Inline::Strong(vec![Inline::Text(TextRun {
                content: "bold".to_string(),
            })]),
            Inline::Text(TextRun {
                content: " plain".to_string(),
            }),
        ];
        let lines = wrap_inlines(&inlines, 40, SemanticStyle::Body);
        assert_eq!(lines.len(), 1);
        let spans = &lines[0].spans;
        // 単語間の空白は直前segmentのstyleに従う
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].style, SemanticStyle::Body);
        assert_eq!(spans[0].content, "plain ");
        assert_eq!(spans[1].style, SemanticStyle::Strong);
        assert_eq!(spans[1].content, "bold ");
        assert_eq!(spans[2].style, SemanticStyle::Body);
        assert_eq!(spans[2].content, "plain");
    }
}
