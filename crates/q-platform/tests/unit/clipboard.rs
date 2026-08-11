use super::*;

#[test]
fn fake_clipboard_records_last_set() {
    let mut cb = FakeClipboard::new();
    cb.set_text("hello").unwrap();
    assert_eq!(cb.last.as_deref(), Some("hello"));
}

#[test]
fn fake_clipboard_overwrites_on_subsequent_set() {
    let mut cb = FakeClipboard::new();
    cb.set_text("first").unwrap();
    cb.set_text("second").unwrap();
    assert_eq!(cb.last.as_deref(), Some("second"));
}

#[test]
fn fake_clipboard_default_is_empty() {
    let cb = FakeClipboard::default();
    assert!(cb.last.is_none());
}
