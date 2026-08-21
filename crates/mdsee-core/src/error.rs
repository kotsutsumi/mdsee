//! Error定義（design.md §66）。library crateは `thiserror` を使う。

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// 入力読み込みエラー。
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to read {path}")]
    ReadFile { path: PathBuf, source: io::Error },

    #[error("failed to read stdin")]
    ReadStdin { source: io::Error },

    #[error("input is not valid UTF-8")]
    InvalidUtf8 { source: std::str::Utf8Error },

    #[error("failed to resolve base directory")]
    CurrentDir { source: io::Error },
}

/// Markdown parseエラー（§66）。
///
/// comrakのparseは現状failしないため、Sprint 1では発生しない。
/// pipeline署名（§100）のために定義しておく。
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid markdown source")]
    InvalidSource,
}
