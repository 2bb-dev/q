//! Filesystem support for external Markdown documents.
//!
//! Extension validation belongs to the CLI. This module only handles paths,
//! exact UTF-8 reads, editor-oriented normalization, and conflict-safe saves.

use std::fs::{self, Metadata, Permissions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::Utf8Error;
use std::time::SystemTime;

use thiserror::Error;

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";

/// Filesystem failures specific to an external document.
#[derive(Debug, Error)]
pub enum ExternalDocumentError {
    #[error("document '{path}' was not found; it may have been moved or deleted")]
    NotFound { path: PathBuf },

    #[error("document '{path}' is not a regular file")]
    NotRegular { path: PathBuf },

    #[error("document '{path}' is not valid UTF-8")]
    InvalidUtf8 {
        path: PathBuf,
        #[source]
        source: Utf8Error,
    },

    #[error("document '{path}' changed on disk; reload it before saving")]
    Conflict { path: PathBuf },

    #[error("could not access document '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// The line-ending convention used when serializing edited text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

/// File-format and filesystem metadata retained across an editor save.
#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    pub has_utf8_bom: bool,
    pub line_ending: LineEnding,
    pub has_final_newline: bool,
    pub permissions: Permissions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentFingerprint {
    modified: SystemTime,
    len: u64,
}

/// A loaded editor document and the byte snapshot used for conflict detection.
#[derive(Debug)]
pub struct EditorDocument {
    /// Absolute, non-canonicalized path supplied for this document.
    pub path: PathBuf,
    /// LF-normalized text, without a UTF-8 BOM or the final line terminator.
    pub text: String,
    pub metadata: DocumentMetadata,
    original_bytes: Vec<u8>,
    original_target: PathBuf,
    original_file_state: FileState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileState {
    readonly: bool,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

/// Make `path` absolute without canonicalizing it or resolving symlinks.
///
/// Relative components such as `.` and `..` are intentionally retained.
pub fn absolute_path(path: &Path) -> Result<PathBuf, ExternalDocumentError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let physical = std::env::current_dir().map_err(|source| ExternalDocumentError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let current = std::env::var_os("PWD")
        .map(PathBuf::from)
        .filter(|pwd| pwd.is_absolute())
        .filter(|pwd| same_directory(pwd, &physical))
        .unwrap_or(physical);
    Ok(current.join(path))
}

fn same_directory(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// Return metadata used to avoid rereading an unchanged external document.
pub fn document_lock_path(path: &Path) -> Result<PathBuf, ExternalDocumentError> {
    let path = absolute_path(path)?;
    let target = current_write_target(&path)?;
    let mut hasher = FnvHasher::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let metadata = fs::metadata(&target).map_err(|source| classify_io(&target, source))?;
        metadata.dev().hash(&mut hasher);
        metadata.ino().hash(&mut hasher);
    }
    #[cfg(not(unix))]
    target.hash(&mut hasher);
    Ok(std::env::temp_dir()
        .join("q-document-locks")
        .join(format!("{:016x}.lock", hasher.finish())))
}

pub fn fingerprint(path: &Path) -> Result<DocumentFingerprint, ExternalDocumentError> {
    let path = absolute_path(path)?;
    let metadata = fs::metadata(&path).map_err(|source| classify_io(&path, source))?;
    if !metadata.is_file() {
        return Err(ExternalDocumentError::NotRegular { path });
    }
    let modified = metadata
        .modified()
        .map_err(|source| io_error(&path, source))?;
    Ok(DocumentFingerprint {
        modified,
        len: metadata.len(),
    })
}

/// Read a regular UTF-8 file exactly.
///
/// The returned string retains all line endings and a leading UTF-8 BOM. Empty
/// files are valid. This is suitable for copy and preview operations.
pub fn read_utf8(path: &Path) -> Result<String, ExternalDocumentError> {
    let path = absolute_path(path)?;
    let (bytes, _) = read_regular_bytes(&path)?;
    String::from_utf8(bytes).map_err(|error| ExternalDocumentError::InvalidUtf8 {
        path,
        source: error.utf8_error(),
    })
}

impl EditorDocument {
    /// Load a regular UTF-8 file for editing.
    pub fn load(path: &Path) -> Result<Self, ExternalDocumentError> {
        let path = absolute_path(path)?;
        let original_target = current_write_target(&path)?;
        let (original_bytes, file_metadata) = read_regular_bytes(&original_target)?;
        if current_write_target(&path)? != original_target {
            return Err(ExternalDocumentError::Conflict { path });
        }
        let decoded = std::str::from_utf8(&original_bytes).map_err(|source| {
            ExternalDocumentError::InvalidUtf8 {
                path: path.clone(),
                source,
            }
        })?;

        let has_utf8_bom = original_bytes.starts_with(UTF8_BOM);
        let content = if has_utf8_bom {
            match decoded.strip_prefix('\u{feff}') {
                Some(content) => content,
                None => decoded,
            }
        } else {
            decoded
        };
        let line_ending = detect_line_ending(content);
        let mut text = content.replace("\r\n", "\n");
        let has_final_newline = text.ends_with('\n');
        if has_final_newline {
            text.truncate(text.len() - 1);
        }

        Ok(Self {
            path,
            text,
            metadata: DocumentMetadata {
                has_utf8_bom,
                line_ending,
                has_final_newline,
                permissions: file_metadata.permissions(),
            },
            original_bytes,
            original_target,
            original_file_state: file_state(&file_metadata),
        })
    }

    /// Save edited, LF-normalized text if the file still matches the loaded bytes.
    ///
    /// `edited_text` is borrowed and remains the caller's responsibility on
    /// success, conflict, or any I/O failure. A final symlink is resolved before
    /// creating and atomically persisting a sibling temporary file, so the
    /// symlink itself is never replaced.
    ///
    /// Callers must serialize cooperative writers. There is no portable
    /// filesystem compare-and-replace primitive that can eliminate the final
    /// check-to-rename race with a non-cooperating external process.
    pub fn save(&mut self, edited_text: &str) -> Result<(), ExternalDocumentError> {
        let target = current_write_target(&self.path)?;
        if target != self.original_target {
            return Err(self.conflict());
        }
        let rendered = self.render(edited_text);
        let parent = target.parent().ok_or_else(|| ExternalDocumentError::Io {
            path: target.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "document has no parent directory",
            ),
        })?;

        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|source| io_error(&target, source))?;
        temporary
            .write_all(&rendered)
            .map_err(|source| io_error(&target, source))?;
        temporary
            .as_file()
            .set_permissions(self.metadata.permissions.clone())
            .map_err(|source| io_error(&target, source))?;
        temporary
            .as_file_mut()
            .sync_all()
            .map_err(|source| io_error(&target, source))?;

        // Compare as late as practical, after preparing the replacement. Also
        // reject a symlink retarget that happened while the temp file was built.
        if current_write_target(&self.path)? != target {
            return Err(self.conflict());
        }
        let (current_bytes, current_metadata) = read_regular_bytes(&target)?;
        if current_bytes != self.original_bytes
            || file_state(&current_metadata) != self.original_file_state
        {
            return Err(self.conflict());
        }

        temporary
            .persist(&target)
            .map_err(|error| io_error(&target, error.error))?;
        let saved_metadata =
            fs::metadata(&target).map_err(|source| classify_io(&target, source))?;
        self.original_bytes = rendered;
        self.original_file_state = file_state(&saved_metadata);
        Ok(())
    }

    fn conflict(&self) -> ExternalDocumentError {
        ExternalDocumentError::Conflict {
            path: self.path.clone(),
        }
    }

    fn render(&self, edited_text: &str) -> Vec<u8> {
        let normalized = edited_text.replace("\r\n", "\n");
        let line_ending = self.metadata.line_ending.as_str();
        let converted = if self.metadata.line_ending == LineEnding::CrLf {
            normalized.replace('\n', line_ending)
        } else {
            normalized
        };

        let capacity = converted.len()
            + usize::from(self.metadata.has_utf8_bom) * UTF8_BOM.len()
            + usize::from(self.metadata.has_final_newline) * line_ending.len();
        let mut bytes = Vec::with_capacity(capacity);
        if self.metadata.has_utf8_bom {
            bytes.extend_from_slice(UTF8_BOM);
        }
        bytes.extend_from_slice(converted.as_bytes());
        if self.metadata.has_final_newline {
            bytes.extend_from_slice(line_ending.as_bytes());
        }
        bytes
    }
}

fn detect_line_ending(content: &str) -> LineEnding {
    let bytes = content.as_bytes();
    match bytes.iter().position(|byte| *byte == b'\n') {
        Some(index) if index > 0 && bytes[index - 1] == b'\r' => LineEnding::CrLf,
        _ => LineEnding::Lf,
    }
}

fn read_regular_bytes(path: &Path) -> Result<(Vec<u8>, Metadata), ExternalDocumentError> {
    let metadata = fs::metadata(path).map_err(|source| classify_io(path, source))?;
    if !metadata.is_file() {
        return Err(ExternalDocumentError::NotRegular {
            path: path.to_path_buf(),
        });
    }
    let bytes = fs::read(path).map_err(|source| classify_io(path, source))?;
    Ok((bytes, metadata))
}

fn file_state(metadata: &Metadata) -> FileState {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        FileState {
            readonly: metadata.permissions().readonly(),
            mode: metadata.permissions().mode(),
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(not(unix))]
    {
        FileState {
            readonly: metadata.permissions().readonly(),
        }
    }
}

fn current_write_target(path: &Path) -> Result<PathBuf, ExternalDocumentError> {
    fs::canonicalize(path).map_err(|source| classify_io(path, source))
}

fn classify_io(path: &Path, source: std::io::Error) -> ExternalDocumentError {
    if source.kind() == std::io::ErrorKind::NotFound {
        ExternalDocumentError::NotFound {
            path: path.to_path_buf(),
        }
    } else {
        io_error(path, source)
    }
}

fn io_error(path: &Path, source: std::io::Error) -> ExternalDocumentError {
    ExternalDocumentError::Io {
        path: path.to_path_buf(),
        source,
    }
}

struct FnvHasher(u64);

impl FnvHasher {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/external_document.rs"]
mod tests;
