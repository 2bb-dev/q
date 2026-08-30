use super::*;

#[test]
fn new_rejects_empty_text() {
    assert!(Prompt::new("").is_err());
    assert!(Prompt::new("   \n\t").is_err());
}

#[test]
fn new_accepts_non_empty_inline_text() {
    let prompt = Prompt::new("hello world").expect("should succeed");
    assert_eq!(prompt.inline_text(), Some("hello world"));
    assert_eq!(prompt.external_markdown_path(), None);
    assert!(!prompt.pinned);
}

#[test]
fn external_markdown_requires_an_absolute_path_without_touching_it() {
    assert!(Prompt::from_external_markdown("relative.md").is_err());

    let missing = std::env::temp_dir()
        .join("missing")
        .join("..")
        .join("prompt.md");
    let prompt = Prompt::from_external_markdown(&missing).unwrap();

    assert_eq!(prompt.inline_text(), None);
    assert_eq!(prompt.external_markdown_path(), Some(missing.as_path()));
    assert_eq!(
        prompt.source(),
        &PromptSource::ExternalMarkdown {
            path: missing.clone()
        }
    );
    assert_eq!(prompt.external_markdown_path(), Some(missing.as_path()));
}

#[cfg(unix)]
#[test]
fn external_markdown_rejects_non_unicode_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let mut path = std::env::temp_dir();
    path.push(OsString::from_vec(vec![0xff]));
    assert!(Prompt::from_external_markdown(path).is_err());
}

#[test]
fn prompt_source_serialization_is_explicitly_tagged() {
    let source = PromptSource::inline("hello").unwrap();
    assert_eq!(
        serde_json::to_value(source).unwrap(),
        serde_json::json!({"type": "inline", "text": "hello"})
    );
}

#[test]
fn preview_uses_first_line_and_truncates_at_80() {
    let prompt = Prompt::new("first line\nsecond line").unwrap();
    assert_eq!(prompt.preview(), "first line");

    let long = "a".repeat(100);
    let prompt = Prompt::new(&long).unwrap();
    let preview = prompt.preview();
    assert_eq!(preview.chars().count(), 80);
    assert!(preview.ends_with("..."));
}

#[test]
fn prompt_id_display_is_8_chars() {
    let id = PromptId::new();
    let s = id.to_string();
    assert_eq!(s.chars().count(), 8);
}

#[test]
fn parse_input_rejects_short_ids() {
    assert!(PromptId::parse_input("abc").is_err());
    assert!(PromptId::parse_input("abcd").is_ok());
}
