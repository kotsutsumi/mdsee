//! mdsee-terminal（design.md §5）。
//!
//! Terminal capabilityを扱う。Sprint 1ではwrap幅の取得とColorLevel型のみを
//! 提供する。TerminalCapabilities体系（§34〜§37）はSprint 2（S2-5）で導入する。

/// 色深度（§35）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorLevel {
    #[default]
    None,
    Ansi16,
    Ansi256,
    TrueColor,
}

/// 端末の列数を返す。取得できない場合（非TTY等）は `None`。
pub fn terminal_columns() -> Option<u16> {
    crossterm::terminal::size().ok().map(|(columns, _)| columns)
}
