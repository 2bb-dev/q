use super::*;
use q_core::{Prompt, Workspace};
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

fn buffer_as_text(buffer: &Buffer) -> String {
    let mut output = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            output.push_str(buffer[(x, y)].symbol());
        }
        output.push('\n');
    }
    output
}

fn render(app: &mut App, cursor_on: bool, width: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
    terminal.draw(|frame| draw(frame, app, cursor_on)).unwrap();
    buffer_as_text(terminal.backend().buffer())
}

#[test]
fn empty_workspace_renders_initial_tab_composer_and_footer() {
    let mut app = App::new(Workspace::new());
    let text = render(&mut app, false, 80);
    assert!(text.contains(" 1 "), "missing initial tab; got:\n{text}");
    assert!(text.contains(" + "), "missing create tab; got:\n{text}");
    assert!(
        !text.contains("type a prompt"),
        "composer placeholder is gone"
    );
    assert!(text.contains("p pin"));
    assert!(!text.contains("[ ] tabs"));
    assert!(!text.contains("^t new"));
    assert!(!text.contains("r rename"));
}

#[test]
fn footer_starts_in_the_same_column_as_the_composer_prefix() {
    let mut app = App::new(Workspace::new());
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal.draw(|frame| draw(frame, &mut app, false)).unwrap();
    let buffer = terminal.backend().buffer();

    let prefix_column = (0..buffer.area.width)
        .find(|x| buffer[(*x, 17)].symbol() == "›")
        .expect("composer prefix");
    let footer_column = (0..buffer.area.width)
        .find(|x| buffer[(*x, 19)].symbol() != " ")
        .expect("footer hints");

    assert_eq!(footer_column, prefix_column);
}

#[test]
fn active_tab_renders_only_its_prompts() {
    let mut workspace = Workspace::new();
    let first = workspace.first_tab_id();
    workspace
        .add_prompt(first, Prompt::new("first prompt").unwrap())
        .unwrap();
    let second = workspace.create_tab("work").unwrap();
    workspace
        .add_prompt(second, Prompt::new("work prompt").unwrap())
        .unwrap();
    let mut app = App::new(workspace);
    app.select_tab(second);
    let text = render(&mut app, true, 80);
    assert!(text.contains("work prompt"));
    assert!(!text.contains("first prompt"));
}

#[test]
fn tab_context_menu_renders_rename_and_close_targets() {
    let mut app = App::new(Workspace::new());
    let id = app.active_tab_id;
    app.tab_menu = Some(crate::app::TabContextMenu {
        tab_id: id,
        column: 2,
        row: 1,
        selected: TabMenuAction::Rename,
    });

    let text = render(&mut app, false, 80);

    assert!(text.contains("Rename"));
    assert!(text.contains("Close"));
    assert_eq!(app.tab_menu_hits.len(), 2);
}

#[test]
fn close_tab_confirmation_warns_about_prompt_deletion() {
    let mut app = App::new(Workspace::new());
    app.close_tab_dialog = Some(crate::app::CloseTabDialog {
        tab_id: app.active_tab_id,
        tab_name: "work".to_string(),
    });

    let text = render(&mut app, false, 80);

    assert!(text.contains("Close \"work\"?"));
    assert!(text.contains("deletes all prompts"));
    assert!(text.contains("Enter close"));
}

#[test]
fn tab_dialog_renders_value_and_error() {
    let mut app = App::new(Workspace::new());
    let mut dialog = crate::app::TabDialog::create();
    dialog.value = "work".to_string();
    dialog.error = "invalid tab".to_string();
    app.tab_dialog = Some(dialog);
    let text = render(&mut app, true, 80);
    assert!(text.contains("New tab"));
    assert!(text.contains("work"));
    assert!(text.contains("invalid tab"));
}

#[test]
fn rendered_prompts_and_composer_have_click_targets() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(tab, Prompt::new("click me").unwrap())
        .unwrap();
    let mut app = App::new(workspace);

    render(&mut app, false, 80);

    let prompt_area = app.prompt_hits[0].area;
    assert_eq!(
        app.content_input_at(prompt_area.x, prompt_area.y),
        Some(crate::app::Input::SelectPrompt(0))
    );
    let composer_area = app.composer_area.unwrap();
    assert_eq!(
        app.content_input_at(composer_area.x, composer_area.y),
        Some(crate::app::Input::FocusComposer)
    );
}

#[test]
fn wrapped_prompt_is_clickable_across_its_rendered_height() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    workspace
        .add_prompt(
            tab,
            Prompt::new("a prompt long enough to wrap across rows").unwrap(),
        )
        .unwrap();
    let mut app = App::new(workspace);

    render(&mut app, false, 12);

    let area = app.prompt_hits[0].area;
    assert!(area.height > 1);
    assert_eq!(
        app.content_input_at(area.x, area.bottom() - 1),
        Some(crate::app::Input::SelectPrompt(0))
    );
}

#[test]
fn long_prompt_collapses_to_three_rows_with_an_ellipsis() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let text = std::iter::repeat_n("word", 200)
        .collect::<Vec<_>>()
        .join(" ");
    workspace
        .add_prompt(tab, Prompt::new(text).unwrap())
        .unwrap();
    let mut app = App::new(workspace);

    let rendered = render(&mut app, false, 40);

    assert_eq!(app.prompt_hits[0].area.height, 3);
    assert!(rendered.contains('…'), "missing ellipsis; got:\n{rendered}");
    let body_rows = rendered
        .lines()
        .filter(|line| line.contains("word"))
        .count();
    assert_eq!(body_rows, 3);
}

#[test]
fn collapsed_rows_condense_whitespace_and_keep_short_prompts_intact() {
    assert_eq!(
        collapsed_rows("title\n\n    indented body", 40),
        vec!["title indented body".to_string()]
    );
    assert_eq!(collapsed_rows("short", 40), vec!["short".to_string()]);
    let rows = collapsed_rows(&"x".repeat(500), 20);
    assert_eq!(rows.len(), 3);
    assert!(rows[2].ends_with('…'));
    assert!(rows.iter().all(|row| row.chars().count() <= 16));
}

#[test]
fn preview_renders_full_prompt_and_scrolls_to_the_end() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    let text = (0..40)
        .map(|index| format!("line-{index}"))
        .collect::<Vec<_>>()
        .join("\n");
    workspace
        .add_prompt(tab, Prompt::new(text).unwrap())
        .unwrap();
    let mut app = App::new(workspace);
    let id = app.visible_prompts()[0].id;
    app.preview = Some(crate::app::PromptPreview {
        source: crate::app::PreviewSource::Prompt(id),
        scroll: 0,
    });

    let top = render(&mut app, false, 60);
    assert!(top.contains("Prompt"));
    assert!(top.contains("Esc close"));
    assert!(top.contains("line-0"));
    assert!(!top.contains("line-39"));
    assert!(app.preview_max_scroll > 0);

    app.preview.as_mut().unwrap().scroll = u16::MAX;
    let bottom = render(&mut app, false, 60);
    assert!(bottom.contains("line-39"));
    assert_eq!(app.preview.as_ref().unwrap().scroll, app.preview_max_scroll);
}

#[test]
fn search_renders_filtered_history_with_click_targets() {
    let mut workspace = Workspace::new();
    let tab = workspace.first_tab_id();
    for text in ["deploy the api", "write the docs"] {
        workspace
            .add_prompt(tab, Prompt::new(text).unwrap())
            .unwrap();
    }
    let mut app = App::new(workspace);
    app.search = Some(crate::app::SearchDialog::default());

    let all = render(&mut app, false, 60);
    assert!(all.contains("History"));
    assert!(all.contains("deploy the api"));
    assert!(all.contains("write the docs"));
    assert_eq!(app.search_hits.len(), 2);
    let hit = app.search_hits[0].area;
    assert_eq!(
        app.search_input_at(hit.x, hit.y),
        Some(crate::app::Input::SelectHistory(0))
    );

    app.search.as_mut().unwrap().query = "docs".to_string();
    let filtered = render(&mut app, false, 60);
    assert!(filtered.contains("write the docs"));
    assert!(!filtered.contains("deploy the api"));
    assert_eq!(app.search_hits.len(), 1);
}

#[test]
fn search_without_matches_says_so() {
    let mut app = App::new(Workspace::new());
    app.search = Some(crate::app::SearchDialog {
        query: "nothing".to_string(),
        selected: 0,
    });

    let text = render(&mut app, false, 60);

    assert!(text.contains("no matching prompts"));
    assert!(app.search_hits.is_empty());
}

#[test]
fn history_preview_renders_text_that_is_no_longer_queued() {
    let mut app = App::new(Workspace::new());
    app.preview = Some(crate::app::PromptPreview {
        source: crate::app::PreviewSource::History("long gone prompt".to_string()),
        scroll: 0,
    });

    let text = render(&mut app, false, 60);

    assert!(text.contains("long gone prompt"));
}

#[test]
fn preview_wraps_long_lines_and_preserves_indentation() {
    assert_eq!(
        wrap_lines("alpha beta gamma", 11),
        vec!["alpha beta ".to_string(), "gamma".to_string()]
    );
    assert_eq!(
        wrap_lines("supercalifragilistic", 10),
        vec!["supercalif".to_string(), "ragilistic".to_string()]
    );
    assert_eq!(
        wrap_lines("    indented", 20),
        vec!["    indented".to_string()]
    );
    assert_eq!(wrap_lines("a\n\nb", 5), vec!["a", "", "b"]);
}

#[test]
fn rendered_tabs_and_create_button_have_click_targets() {
    let mut app = App::new(Workspace::new());
    render(&mut app, false, 80);
    assert_eq!(app.tab_hits.len(), 2);
    for hit in &app.tab_hits {
        assert!(app.tab_input_at(hit.area.x, hit.area.y).is_some());
    }
    assert_eq!(app.tab_hits[0].area.right(), app.tab_hits[1].area.x);
}

#[test]
fn tab_bar_has_a_full_width_background() {
    let mut app = App::new(Workspace::new());
    let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
    terminal.draw(|frame| draw(frame, &mut app, false)).unwrap();

    assert_eq!(terminal.backend().buffer()[(39, 1)].bg, TAB_BAR_BG);
}

#[test]
fn narrow_tab_bar_keeps_active_tab_and_create_visible() {
    let mut workspace = Workspace::new();
    for name in ["backend", "website", "documentation"] {
        workspace.create_tab(name).unwrap();
    }
    let active = workspace.resolve_tab("1").unwrap();
    let mut app = App::new(workspace);
    app.select_tab(active);
    let text = render(&mut app, false, 18);
    assert!(text.contains(" 1 "));
    assert!(text.contains(" + "));
}
