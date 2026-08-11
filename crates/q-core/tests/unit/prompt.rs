use super::*;

#[test]
fn new_rejects_empty_text() {
    assert!(Prompt::new("").is_err());
    assert!(Prompt::new("   \n\t").is_err());
}

#[test]
fn new_accepts_non_empty_text() {
    let p = Prompt::new("hello world").expect("should succeed");
    assert_eq!(p.text, "hello world");
    assert!(!p.pinned);
}

#[test]
fn preview_uses_first_line_and_truncates_at_80() {
    let p = Prompt::new("first line\nsecond line").unwrap();
    assert_eq!(p.preview(), "first line");

    let long = "a".repeat(100);
    let p = Prompt::new(&long).unwrap();
    let preview = p.preview();
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
