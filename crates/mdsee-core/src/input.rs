//! 入力の読み込み（design.md §9, §100）。

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::error::LoadError;

/// 入力源（§9）。
#[derive(Debug, Clone)]
pub enum InputSource {
    File(PathBuf),
    Stdin,
}

/// 読み込み済みドキュメント（§9）。
#[derive(Debug, Clone)]
pub struct SourceDocument {
    pub content: String,
    pub origin: Origin,
}

/// ドキュメントの由来（§9）。
///
/// `base_dir` はMarkdown中の相対link・画像を解決する基準 Directory。
#[derive(Debug, Clone)]
pub enum Origin {
    File { path: PathBuf, base_dir: PathBuf },
    Stdin { cwd: PathBuf },
}

/// 入力を読み込む（§100 基本pipeline）。
pub fn load_source(input: InputSource) -> Result<SourceDocument, LoadError> {
    match input {
        InputSource::File(path) => load_file(path),
        InputSource::Stdin => load_stdin(),
    }
}

fn load_file(path: PathBuf) -> Result<SourceDocument, LoadError> {
    let bytes = fs::read(&path).map_err(|source| LoadError::ReadFile {
        path: path.clone(),
        source,
    })?;
    let content = decode_utf8(bytes)?;
    let base_dir = base_dir_of(&path)?;
    Ok(SourceDocument {
        content,
        origin: Origin::File { path, base_dir },
    })
}

fn load_stdin() -> Result<SourceDocument, LoadError> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .map_err(|source| LoadError::ReadStdin { source })?;
    let content = decode_utf8(bytes)?;
    let cwd = std::env::current_dir().map_err(|source| LoadError::CurrentDir { source })?;
    Ok(SourceDocument {
        content,
        origin: Origin::Stdin { cwd },
    })
}

fn decode_utf8(bytes: Vec<u8>) -> Result<String, LoadError> {
    match String::from_utf8(bytes) {
        Ok(content) => Ok(content),
        Err(err) => Err(LoadError::InvalidUtf8 {
            source: err.utf8_error(),
        }),
    }
}

/// `base_dir` 解決（§9）。
///
/// `foo.md` のようにparentが空の相対Pathの場合はcurrent directoryを採る。
fn base_dir_of(path: &Path) -> Result<PathBuf, LoadError> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => Ok(parent.to_path_buf()),
        _ => std::env::current_dir().map_err(|source| LoadError::CurrentDir { source }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_file_reads_content_and_base_dir() {
        let dir = std::env::temp_dir().join("mdsee-core-tests");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.md");
        fs::write(&path, "# hi").unwrap();

        let doc = load_source(InputSource::File(path.clone())).unwrap();
        assert_eq!(doc.content, "# hi");
        match doc.origin {
            Origin::File { path: p, base_dir } => {
                assert_eq!(p, path);
                assert_eq!(base_dir, dir);
            }
            Origin::Stdin { .. } => panic!("expected file origin"),
        }
    }

    #[test]
    fn load_file_rejects_invalid_utf8() {
        let dir = std::env::temp_dir().join("mdsee-core-tests");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("binary.md");
        fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();

        let err = load_source(InputSource::File(path)).unwrap_err();
        assert!(matches!(err, LoadError::InvalidUtf8 { .. }));
    }

    #[test]
    fn load_file_reports_missing_file() {
        let err = load_source(InputSource::File(PathBuf::from("definitely/missing.md")));
        assert!(matches!(err, Err(LoadError::ReadFile { .. })));
    }
}
