use qcli_core::Queue;
use qcli_tui::{reduce, App, Effect, Input};

#[test]
fn end_to_end_reducer_flow() {
    let mut app = App::new(Queue::new());

    // Focus composer, type "first", save.
    reduce(&mut app, Input::Tab);
    for c in "first".chars() {
        reduce(&mut app, Input::Char(c));
    }
    let eff = reduce(&mut app, Input::CtrlS);
    assert_eq!(eff, Some(Effect::Persist));

    // Add a second prompt.
    reduce(&mut app, Input::Tab);
    for c in "second".chars() {
        reduce(&mut app, Input::Char(c));
    }
    reduce(&mut app, Input::CtrlS);
    assert_eq!(app.visible_prompts().len(), 2);

    // Pin "first".
    reduce(&mut app, Input::Up);
    let eff = reduce(&mut app, Input::Char('p'));
    assert_eq!(eff, Some(Effect::Persist));
    assert!(app.visible_prompts()[0].pinned);

    // Copy+pop the unpinned "second".
    reduce(&mut app, Input::Down);
    let eff = reduce(&mut app, Input::Enter);
    assert_eq!(eff, Some(Effect::CopyToClipboard("second".to_string())));
    assert_eq!(app.visible_prompts().len(), 1);

    // Quit.
    assert_eq!(reduce(&mut app, Input::Quit), Some(Effect::Quit));
}
