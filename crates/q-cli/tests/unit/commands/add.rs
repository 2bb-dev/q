use super::*;

#[test]
fn detects_supported_markdown_extensions_case_insensitively() {
    assert!(is_markdown_path("prompt.md"));
    assert!(is_markdown_path("prompt.MARKDOWN"));
    assert!(!is_markdown_path("prompt.txt"));
}
