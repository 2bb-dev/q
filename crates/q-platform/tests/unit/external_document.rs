use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;

#[test]
fn absolute_path_retains_noncanonical_components() {
    let relative = Path::new("somewhere").join("..").join("document.md");
    let expected = std::env::current_dir().unwrap().join(&relative);

    let absolute = absolute_path(&relative).unwrap();

    assert!(absolute.is_absolute());
    assert_eq!(absolute, expected);
    assert!(absolute.to_string_lossy().contains(".."));

    let already_absolute = expected.join(".").join("other.md");
    assert_eq!(absolute_path(&already_absolute).unwrap(), already_absolute);
}

#[test]
fn fingerprint_changes_when_document_changes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("document.md");
    fs::write(&path, "one").unwrap();
    let before = fingerprint(&path).unwrap();

    fs::write(&path, "longer contents").unwrap();

    assert_ne!(fingerprint(&path).unwrap(), before);
}

#[test]
fn exact_read_allows_empty_and_preserves_bom_and_line_endings() {
    let dir = TempDir::new().unwrap();
    let empty = dir.path().join("empty.md");
    fs::write(&empty, []).unwrap();
    assert_eq!(read_utf8(&empty).unwrap(), "");

    let exact = dir.path().join("exact.markdown");
    let bytes = b"\xef\xbb\xbfheading\r\n\r\nbody\n";
    fs::write(&exact, bytes).unwrap();
    assert_eq!(read_utf8(&exact).unwrap().as_bytes(), bytes);
}

#[test]
fn read_errors_distinguish_missing_non_regular_and_invalid_utf8() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("missing.md");
    let error = read_utf8(&missing).unwrap_err();
    assert!(matches!(error, ExternalDocumentError::NotFound { .. }));
    assert!(error.to_string().contains("may have been moved or deleted"));

    let error = read_utf8(dir.path()).unwrap_err();
    assert!(matches!(error, ExternalDocumentError::NotRegular { .. }));

    let invalid = dir.path().join("invalid.md");
    fs::write(&invalid, [0xff, 0xfe]).unwrap();
    let error = read_utf8(&invalid).unwrap_err();
    assert!(matches!(error, ExternalDocumentError::InvalidUtf8 { .. }));
    assert!(error.to_string().contains("not valid UTF-8"));
}

#[test]
fn editor_load_normalizes_and_save_restores_bom_crlf_and_final_newline() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("document.md");
    fs::write(&path, b"\xef\xbb\xbfone\r\ntwo\r\n").unwrap();

    let mut document = EditorDocument::load(&path).unwrap();
    assert_eq!(document.text, "one\ntwo");
    assert!(document.metadata.has_utf8_bom);
    assert_eq!(document.metadata.line_ending, LineEnding::CrLf);
    assert!(document.metadata.has_final_newline);

    let caller_buffer = String::from("changed\ntext");
    document.save(&caller_buffer).unwrap();

    assert_eq!(caller_buffer, "changed\ntext");
    assert_eq!(fs::read(&path).unwrap(), b"\xef\xbb\xbfchanged\r\ntext\r\n");
}

#[test]
fn editor_save_preserves_absence_of_final_newline() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("document.md");
    fs::write(&path, b"one\r\ntwo").unwrap();

    let mut document = EditorDocument::load(&path).unwrap();
    assert_eq!(document.text, "one\ntwo");
    assert_eq!(document.metadata.line_ending, LineEnding::CrLf);
    assert!(!document.metadata.has_final_newline);

    document.save("updated\ntext").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"updated\r\ntext");
}

#[test]
fn save_detects_byte_conflicts_without_overwriting_disk_or_caller_buffer() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("document.md");
    fs::write(&path, "original\n").unwrap();
    let mut document = EditorDocument::load(&path).unwrap();
    fs::write(&path, "changed elsewhere\n").unwrap();
    let caller_buffer = String::from("my unsaved edit");

    let error = document.save(&caller_buffer).unwrap_err();

    assert!(matches!(error, ExternalDocumentError::Conflict { .. }));
    assert_eq!(fs::read_to_string(&path).unwrap(), "changed elsewhere\n");
    assert_eq!(caller_buffer, "my unsaved edit");
}

#[cfg(unix)]
#[test]
fn save_preserves_unix_permissions() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("document.md");
    fs::write(&path, "original").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let before = fs::metadata(&path).unwrap();
    let mut document = EditorDocument::load(&path).unwrap();

    document.save("updated").unwrap();

    let after = fs::metadata(&path).unwrap();
    assert_eq!(after.mode() & 0o7777, before.mode() & 0o7777);
}

#[cfg(unix)]
#[test]
fn save_rejects_concurrent_permission_changes() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("document.md");
    fs::write(&path, "original").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let mut document = EditorDocument::load(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    let error = document.save("updated").unwrap_err();

    assert!(matches!(error, ExternalDocumentError::Conflict { .. }));
    assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn save_through_symlink_replaces_target_and_not_symlink() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target.md");
    let link = dir.path().join("link.md");
    fs::write(&target, "original\n").unwrap();
    symlink("target.md", &link).unwrap();
    let mut document = EditorDocument::load(&link).unwrap();

    document.save("updated").unwrap();

    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(&target).unwrap(), "updated\n");
}

#[cfg(unix)]
#[test]
fn document_lock_is_shared_by_symlink_and_target() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target.md");
    let link = dir.path().join("link.md");
    fs::write(&target, "body").unwrap();
    symlink("target.md", &link).unwrap();

    assert_eq!(
        document_lock_path(&target).unwrap(),
        document_lock_path(&link).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn save_rejects_a_retargeted_symlink() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    let first = dir.path().join("first.md");
    let second = dir.path().join("second.md");
    let link = dir.path().join("link.md");
    fs::write(&first, "same bytes").unwrap();
    fs::write(&second, "same bytes").unwrap();
    symlink("first.md", &link).unwrap();
    let mut document = EditorDocument::load(&link).unwrap();

    fs::remove_file(&link).unwrap();
    symlink("second.md", &link).unwrap();
    let error = document.save("updated").unwrap_err();

    assert!(matches!(error, ExternalDocumentError::Conflict { .. }));
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(&first).unwrap(), "same bytes");
    assert_eq!(fs::read_to_string(&second).unwrap(), "same bytes");
}
