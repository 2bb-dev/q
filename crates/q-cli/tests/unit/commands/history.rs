use super::*;

#[test]
fn condense_collapses_whitespace_and_truncates() {
    assert_eq!(condense("first\n\n  second"), "first second");
    let long = condense(&"x".repeat(100));
    assert_eq!(long.chars().count(), 80);
    assert!(long.ends_with("..."));
}

#[test]
fn prompt_count_is_pluralized() {
    assert_eq!(prompt_count(0), "0 prompts");
    assert_eq!(prompt_count(1), "1 prompt");
    assert_eq!(prompt_count(2), "2 prompts");
}
