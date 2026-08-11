use super::*;

fn p(text: &str) -> Prompt {
    Prompt::new(text).unwrap()
}

#[test]
fn add_orders_unpinned_newest_first() {
    let mut q = Queue::new();
    q.add(p("a"));
    q.add(p("b"));
    assert_eq!(
        q.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
        vec!["b", "a"]
    );
}

#[test]
fn pinned_prompts_sort_before_unpinned() {
    let mut q = Queue::new();
    q.add(p("one"));
    q.add(p("two"));
    let mut pinned = p("zero");
    pinned.pinned = true;
    q.add(pinned);
    let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, vec!["zero", "two", "one"]);
}

#[test]
fn remove_returns_the_prompt() {
    let mut q = Queue::new();
    let id = q.add(p("foo"));
    let removed = q.remove(id).unwrap();
    assert_eq!(removed.text, "foo");
    assert_eq!(q.len(), 0);
}

#[test]
fn edit_replaces_text() {
    let mut q = Queue::new();
    let id = q.add(p("old"));
    q.edit(id, "new").unwrap();
    assert_eq!(q.get(id).unwrap().text, "new");
}

#[test]
fn edit_rejects_empty() {
    let mut q = Queue::new();
    let id = q.add(p("old"));
    assert!(q.edit(id, "").is_err());
}

#[test]
fn set_pinned_true_moves_to_pinned_section() {
    let mut q = Queue::new();
    q.add(p("a"));
    let id = q.add(p("b"));
    q.add(p("c"));
    q.set_pinned(id, true).unwrap();
    let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, vec!["b", "c", "a"]);
}

#[test]
fn pop_next_unpinned_skips_pinned_head() {
    let mut q = Queue::new();
    let mut pinned = p("stay");
    pinned.pinned = true;
    q.add(pinned);
    q.add(p("go"));
    let popped = q.pop_next_unpinned().unwrap();
    assert_eq!(popped.text, "go");
    assert_eq!(q.len(), 1);
}

#[test]
fn pop_next_unpinned_returns_none_when_only_pinned() {
    let mut q = Queue::new();
    let mut pinned = p("only");
    pinned.pinned = true;
    q.add(pinned);
    assert!(q.pop_next_unpinned().is_none());
}

#[test]
fn resolve_by_full_id_succeeds() {
    let mut q = Queue::new();
    let id = q.add(p("hello"));
    let full = id.0.as_hyphenated().to_string();
    assert_eq!(q.resolve(&full).unwrap(), id);
}

#[test]
fn resolve_by_short_prefix_succeeds() {
    let mut q = Queue::new();
    let id = q.add(p("hello"));
    let short = id.to_string();
    assert_eq!(q.resolve(&short).unwrap(), id);
}

#[test]
fn resolve_reports_not_found() {
    let q = Queue::new();
    assert!(matches!(q.resolve("abcd"), Err(CoreError::NotFound(_))));
}

#[test]
fn clear_empties_queue() {
    let mut q = Queue::new();
    q.add(p("a"));
    q.add(p("b"));
    q.clear();
    assert!(q.is_empty());
}

#[test]
fn iter_pinned_only_yields_pinned() {
    let mut q = Queue::new();
    q.add(p("unpinned-a"));
    let mut pin1 = p("pinned-1");
    pin1.pinned = true;
    let mut pin2 = p("pinned-2");
    pin2.pinned = true;
    q.add(pin1);
    q.add(pin2);
    q.add(p("unpinned-b"));
    let pinned: Vec<_> = q.iter_pinned().map(|p| p.text.as_str()).collect();
    assert_eq!(pinned.len(), 2);
    assert!(pinned.contains(&"pinned-1"));
    assert!(pinned.contains(&"pinned-2"));
}

#[test]
fn iter_unpinned_only_yields_unpinned() {
    let mut q = Queue::new();
    q.add(p("unpinned-a"));
    q.add(p("unpinned-b"));
    let mut pin = p("pinned-1");
    pin.pinned = true;
    q.add(pin);
    let unpinned: Vec<_> = q.iter_unpinned().map(|p| p.text.as_str()).collect();
    assert_eq!(unpinned.len(), 2);
    assert!(unpinned.contains(&"unpinned-a"));
    assert!(unpinned.contains(&"unpinned-b"));
}

#[test]
fn add_text_builds_prompt_and_returns_id() {
    let mut q = Queue::new();
    let id = q.add_text("hello").unwrap();
    assert_eq!(q.len(), 1);
    let prompt = q.get(id).unwrap();
    assert_eq!(prompt.text, "hello");
    assert_eq!(prompt.id, id);
}

#[test]
fn add_text_rejects_empty() {
    let mut q = Queue::new();
    assert!(q.add_text("").is_err());
    assert!(q.add_text("   \n").is_err());
}

#[test]
fn move_within_group_swaps_within_unpinned() {
    let mut q = Queue::new();
    q.add_text("first").unwrap();
    q.add_text("second").unwrap();
    let newest = q.add_text("third").unwrap();
    let result = q.move_within_group(newest, 1).unwrap();
    assert!(result);
    let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, vec!["second", "third", "first"]);
}

#[test]
fn move_within_group_clamps_at_boundary() {
    let mut q = Queue::new();
    let oldest_id = q.add_text("oldest").unwrap();
    q.add_text("mid").unwrap();
    let newest_id = q.add_text("newest").unwrap();
    assert!(!q.move_within_group(newest_id, -1).unwrap());
    assert!(!q.move_within_group(oldest_id, 1).unwrap());
    let texts: Vec<_> = q.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, vec!["newest", "mid", "oldest"]);
}

#[test]
fn move_within_group_cannot_cross_into_other_group() {
    let mut q = Queue::new();
    let mut pin = p("pinned");
    pin.pinned = true;
    let pin_id = q.add(pin);
    q.add_text("unpin-first").unwrap();
    let unpinned_head_id = q.add_text("unpin-second").unwrap();
    // Moving the only pinned prompt down by 5 — single-item group, clamps to itself
    assert!(!q.move_within_group(pin_id, 5).unwrap());
    // Moving first unpinned up by 5 — clamps to unpinned-group head, no change
    assert!(!q.move_within_group(unpinned_head_id, -5).unwrap());
    // Invariant: pinned still first
    assert!(q.prompts[0].pinned);
    assert!(!q.prompts[1].pinned);
}

#[test]
fn move_within_group_unknown_id_is_error() {
    let mut q = Queue::new();
    let unknown = PromptId::new();
    assert!(matches!(
        q.move_within_group(unknown, 1),
        Err(CoreError::NotFound(_))
    ));
}

#[test]
fn move_within_group_zero_delta_is_noop() {
    let mut q = Queue::new();
    let id = q.add_text("only").unwrap();
    assert!(!q.move_within_group(id, 0).unwrap());
    assert_eq!(q.len(), 1);
}
