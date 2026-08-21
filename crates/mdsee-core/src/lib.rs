//! mdsee-core（design.md §5）。
//!
//! MarkdownをDocument ASTへ変換する。Terminalには依存しない（§101）。

mod ast;
mod error;
mod input;
mod parser;

pub use ast::{
    Block, BlockId, CodeBlock, Document, DocumentMetadata, Heading, Inline, Link, Paragraph,
    SourceSpan, TextRun,
};
pub use error::{LoadError, ParseError};
pub use input::{load_source, InputSource, Origin, SourceDocument};
pub use parser::{parse, ComrakParser, MarkdownParser};
