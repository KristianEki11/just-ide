//! File loading, encoding detection, atomic save, and fault injection.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::document::Document;
use crate::error::CoreError;
use crate::history::History;
use crate::types::{DocId, Eol, LanguageId, Selection};

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Fault injection modes for atomic file saving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveFault {
    /// Normal save without injected faults.
    #[default]
    None,
    /// Simulates a crash or power loss immediately after the temp file is written and synced,
    /// ensuring the temp file is cleaned up and the original target file remains untouched.
    FailAfterTempWrite,
}

impl Document {
    /// Loads a document from raw bytes in memory with automatic BOM, encoding, and EOL detection.
    pub fn from_bytes(id: DocId, path: Option<PathBuf>, bytes: &[u8]) -> Result<Self, CoreError> {
        let (encoding, has_bom, payload, is_utf16) = detect_bom_or_encoding(bytes);

        // Binary check: if not UTF-16 and first 8 KB contains 0x00
        let sample_len = payload.len().min(8192);
        let is_binary = !is_utf16 && payload[..sample_len].contains(&0x00);

        let (cow, malformed) = encoding.decode_without_bom_handling(payload);
        if malformed && !is_binary {
            return Err(CoreError::Encoding(
                "Malformed encoded byte stream".to_string(),
            ));
        }

        let decoded_str = &cow;
        let eol = detect_majority_eol(decoded_str);

        // Normalize \r\n and \r to \n into ropey::Rope
        let rope = if decoded_str.contains('\r') {
            let mut normalized = String::with_capacity(decoded_str.len());
            let mut chars = decoded_str.chars().peekable();
            while let Some(ch) = chars.next() {
                if ch == '\r' {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    normalized.push('\n');
                } else {
                    normalized.push(ch);
                }
            }
            ropey::Rope::from_str(&normalized)
        } else {
            // Avoid extra full-string allocations when input is already pure \n!
            ropey::Rope::from_str(decoded_str)
        };

        let large_file = bytes.len() > 50 * 1024 * 1024 || rope.len_lines() > 1_000_000;

        Ok(Self {
            id,
            rope,
            path,
            encoding,
            has_bom,
            eol,
            language: LanguageId::default(),
            selection: Selection::point(0),
            history: History::new(),
            version: 0,
            saved_version: 0,
            large_file,
            is_binary,
            read_only: is_binary,
            tab_size: 4,
            insert_spaces: true,
            preferred_col: None,
        })
    }

    /// Opens and loads a document from a filesystem path.
    pub fn open(id: DocId, path: &Path) -> Result<Self, CoreError> {
        let bytes = std::fs::read(path)?;
        let mut doc = Self::from_bytes(id, Some(path.to_path_buf()), &bytes)?;
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.permissions().readonly() {
                doc.read_only = true;
            }
        }
        Ok(doc)
    }

    /// Encodes the document's rope buffer into bytes according to its encoding, BOM, and EOL.
    pub fn encode_bytes(&self) -> Result<Vec<u8>, CoreError> {
        let mut text_with_eol = String::new();
        for line in self.rope.lines() {
            let line_str = line.to_string();
            if self.eol == Eol::Lf {
                text_with_eol.push_str(&line_str);
            } else if line_str.ends_with('\n') {
                text_with_eol.push_str(&line_str[..line_str.len() - 1]);
                text_with_eol.push_str(self.eol.as_str());
            } else {
                text_with_eol.push_str(&line_str);
            }
        }

        if self.encoding == encoding_rs::UTF_16LE {
            let mut bytes = Vec::with_capacity(text_with_eol.len() * 2 + 2);
            if self.has_bom {
                bytes.extend_from_slice(&[0xFF, 0xFE]);
            }
            for u in text_with_eol.encode_utf16() {
                bytes.extend_from_slice(&u.to_le_bytes());
            }
            Ok(bytes)
        } else if self.encoding == encoding_rs::UTF_16BE {
            let mut bytes = Vec::with_capacity(text_with_eol.len() * 2 + 2);
            if self.has_bom {
                bytes.extend_from_slice(&[0xFE, 0xFF]);
            }
            for u in text_with_eol.encode_utf16() {
                bytes.extend_from_slice(&u.to_be_bytes());
            }
            Ok(bytes)
        } else if self.encoding == encoding_rs::UTF_8 {
            let mut bytes = Vec::with_capacity(text_with_eol.len() + 3);
            if self.has_bom {
                bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            bytes.extend_from_slice(text_with_eol.as_bytes());
            Ok(bytes)
        } else {
            let mut output = Vec::new();
            let mut encoder = self.encoding.new_encoder();
            let mut src = &text_with_eol[..];
            let mut buffer = [0u8; 8192];
            loop {
                let (result, read, written, has_unmappable) =
                    encoder.encode_from_utf8(src, &mut buffer, true);
                if has_unmappable {
                    return Err(CoreError::Encoding(
                        "Text cannot be represented in this encoding".into(),
                    ));
                }
                output.extend_from_slice(&buffer[..written]);
                src = &src[read..];
                match result {
                    encoding_rs::CoderResult::InputEmpty => break,
                    encoding_rs::CoderResult::OutputFull => {}
                }
            }
            Ok(output)
        }
    }

    /// Saves the document to its existing path.
    pub fn save(&mut self) -> Result<(), CoreError> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No document path"))?;
        self.save_to(&path, SaveFault::None)
    }

    /// Atomically saves the document to `target_path` via a temporary file with fsync and rename.
    pub fn save_to(&mut self, target_path: &Path, fault: SaveFault) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::ReadOnly);
        }

        let bytes = self.encode_bytes()?;

        let parent = target_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let filename = target_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document");

        let pid = std::process::id();
        let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp_path = parent.join(format!(".{filename}.jide-tmp-{pid}-{seq}"));

        let mut tmp_file = File::options()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;
        let result = (|| -> Result<(), CoreError> {
            tmp_file.write_all(&bytes)?;
            tmp_file.flush()?;
            tmp_file.sync_all()?;
            if fault == SaveFault::FailAfterTempWrite {
                return Err(CoreError::SimulatedFault);
            }
            match std::fs::metadata(target_path) {
                Ok(meta) => {
                    if meta.permissions().readonly() {
                        return Err(CoreError::ReadOnly);
                    }
                    std::fs::set_permissions(&tmp_path, meta.permissions())?;
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
            Ok(())
        })();
        drop(tmp_file);
        let result =
            result.and_then(|()| std::fs::rename(&tmp_path, target_path).map_err(CoreError::from));
        if let Err(err) = result {
            std::fs::remove_file(&tmp_path)?;
            return Err(err);
        }

        self.path = Some(target_path.to_path_buf());
        self.saved_version = self.version;
        Ok(())
    }
}

/// Detects BOM or guesses encoding using `chardetng`.
fn detect_bom_or_encoding(bytes: &[u8]) -> (&'static encoding_rs::Encoding, bool, &[u8], bool) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        (encoding_rs::UTF_8, true, &bytes[3..], false)
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        (encoding_rs::UTF_16LE, true, &bytes[2..], true)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        (encoding_rs::UTF_16BE, true, &bytes[2..], true)
    } else {
        let sample_len = bytes.len().min(64 * 1024);
        let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
        detector.feed(&bytes[..sample_len], sample_len == bytes.len());
        let encoding = detector.guess(None, chardetng::Utf8Detection::Allow);
        (encoding, false, bytes, false)
    }
}

/// Detects majority EOL delimiter in the first 1 MB of text.
fn detect_majority_eol(text: &str) -> Eol {
    let mut end = text.len().min(1024 * 1024);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let sample = &text[..end];

    let mut crlf_count = 0usize;
    let mut lf_count = 0usize;
    let mut cr_count = 0usize;

    let mut chars = sample.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                crlf_count += 1;
                chars.next();
            } else {
                cr_count += 1;
            }
        } else if ch == '\n' {
            lf_count += 1;
        }
    }

    if crlf_count > lf_count && crlf_count > cr_count {
        Eol::CrLf
    } else if cr_count > lf_count && cr_count > crlf_count {
        Eol::Cr
    } else {
        Eol::Lf
    }
}
