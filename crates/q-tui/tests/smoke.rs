use q_core::Queue;
use q_tui::{reduce, App, Effect, Input, QueueMutation};

#[test]
fn end_to_end_reducer_flow() {
    let mut app = App::new(Queue::new());

    // Empty queue starts focused on composer; type "first" and save.
    for c in "first".chars() {
        reduce(&mut app, Input::Char(c));
    }
    let eff = reduce(&mut app, Input::CtrlS);
    assert!(matches!(
        eff,
        Some(Effect::Persist(QueueMutation::Add { prompt, .. })) if prompt.text == "first"
    ));

    // Add a second prompt.
    reduce(&mut app, Input::Tab);
    for c in "second".chars() {
        reduce(&mut app, Input::Char(c));
    }
    reduce(&mut app, Input::CtrlS);
    assert_eq!(app.visible_prompts().len(), 2);

    // Pin "first" (newest-first ordering leaves it below "second").
    reduce(&mut app, Input::Down);
    let first_id = app.selected_prompt().unwrap().id;
    let eff = reduce(&mut app, Input::Char('p'));
    assert_eq!(
        eff,
        Some(Effect::Persist(QueueMutation::SetPinned {
            id: first_id,
            pinned: true,
        }))
    );
    assert!(app.visible_prompts()[0].pinned);

    // Copy+pop the unpinned "second".
    reduce(&mut app, Input::Down);
    let second_id = app.selected_prompt().unwrap().id;
    let eff = reduce(&mut app, Input::Enter);
    assert_eq!(
        eff,
        Some(Effect::CopyAndPersist {
            text: "second".to_string(),
            mutation: QueueMutation::Remove(second_id),
        })
    );
    assert_eq!(app.visible_prompts().len(), 1);

    // Quit.
    assert_eq!(reduce(&mut app, Input::Quit), Some(Effect::Quit));
}
