use crate::app::{App, Pane, PromptHit, TabHit, TabHitTarget, TabMenuAction, TabMenuHit};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const TAB_BAR_BG: Color = Color::Rgb(32, 32, 32);
const CREATE_WIDTH: u16 = 3;

pub fn draw(frame: &mut Frame, app: &mut App, cursor_on: bool) {
    let area = frame.area();
    let composer_height = composer_height(app.composer.lines(), area.width, area.height);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(composer_height),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    render_tabs(frame, app, outer[1]);
    render_queue(frame, app, outer[3]);
    render_composer(frame, app, cursor_on, outer[5]);
    render_footer(frame, app, outer[7]);
    render_preview(
        frame,
        app,
        Rect::new(
            area.x,
            outer[2].y,
            area.width,
            outer[6].bottom().saturating_sub(outer[2].y),
        ),
    );
    render_tab_menu(frame, app);
    render_tab_dialog(frame, app, cursor_on);
    render_close_tab_dialog(frame, app);
}

fn composer_height(lines: &[String], term_width: u16, term_height: u16) -> u16 {
    let wrap_width = (term_width as usize).saturating_sub(3).max(1);
    let total: usize = lines
        .iter()
        .map(|line| {
            let len = line.chars().count() + 1;
            len.div_ceil(wrap_width).max(1)
        })
        .sum();
    let max_height = (term_height as usize).saturating_sub(9).max(1);
    total.clamp(1, max_height) as u16
}

fn render_tabs(frame: &mut Frame, app: &mut App, area: Rect) {
    app.tab_hits.clear();
    if area.width == 0 {
        return;
    }

    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(TAB_BAR_BG)),
        area,
    );

    let tabs: Vec<_> = app
        .workspace
        .tabs()
        .iter()
        .map(|tab| (tab.id(), tab.name().to_string()))
        .collect();
    let available = area.width.saturating_sub(CREATE_WIDTH);
    let active_index = tabs
        .iter()
        .position(|(id, _)| *id == app.active_tab_id)
        .unwrap_or(0);

    let widths: Vec<u16> = tabs
        .iter()
        .map(|(_, name)| ((name.chars().count() + 2) as u16).min(available.max(1)))
        .collect();
    let mut start = active_index;
    let mut end = active_index + 1;
    let mut used = widths.get(active_index).copied().unwrap_or(0);

    while end < tabs.len() && used.saturating_add(widths[end]) <= available {
        used += widths[end];
        end += 1;
    }
    while start > 0 && used.saturating_add(widths[start - 1]) <= available {
        start -= 1;
        used += widths[start];
    }

    let mut x = area.x;
    let content_right = area.right().saturating_sub(CREATE_WIDTH);
    for index in start..end {
        if x >= content_right {
            break;
        }
        let width = widths[index].min(content_right.saturating_sub(x));
        if width == 0 {
            continue;
        }
        let tab_area = Rect::new(x, area.y, width, 1);
        let (id, name) = &tabs[index];
        let inner_width = width.saturating_sub(2) as usize;
        let clipped: String = name.chars().take(inner_width).collect();
        let style = if *id == app.active_tab_id {
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray).bg(TAB_BAR_BG)
        };
        frame.render_widget(
            Paragraph::new(format!(" {clipped} ")).style(style),
            tab_area,
        );
        app.tab_hits.push(TabHit {
            area: tab_area,
            target: TabHitTarget::Tab(*id),
        });
        x = x.saturating_add(width);
    }

    let create_area = Rect::new(
        x,
        area.y,
        CREATE_WIDTH.min(area.right().saturating_sub(x)),
        1,
    );
    frame.render_widget(
        Paragraph::new(" + ").style(Style::default().fg(ACCENT).bg(TAB_BAR_BG)),
        create_area,
    );
    app.tab_hits.push(TabHit {
        area: create_area,
        target: TabHitTarget::Create,
    });
}

fn render_queue(frame: &mut Frame, app: &mut App, area: Rect) {
    app.prompt_hits.clear();
    let focused = app.focus == Pane::Queue && !app.dialog_open() && app.tab_menu.is_none();
    let prompts = app.visible_prompts();
    let mut lines = Vec::new();
    let mut hits = Vec::new();
    let mut visual_row = 0;

    for (index, prompt) in prompts.iter().enumerate() {
        if index > 0 {
            lines.push(Line::raw(""));
            visual_row += 1;
        }
        let line = row_line(index, app.selected, focused, prompt.pinned, &prompt.text);
        let row_height = if area.width == 0 {
            0
        } else {
            (line.width() as u16).div_ceil(area.width).max(1)
        };
        if visual_row < area.height && row_height > 0 {
            hits.push(PromptHit {
                area: Rect::new(
                    area.x,
                    area.y + visual_row,
                    area.width,
                    row_height.min(area.height - visual_row),
                ),
                index,
            });
        }
        visual_row = visual_row.saturating_add(row_height);
        lines.push(line);
    }
    app.prompt_hits = hits;
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn row_line(
    index: usize,
    selected: Option<usize>,
    focused: bool,
    pinned: bool,
    text: &str,
) -> Line<'static> {
    let is_selected = Some(index) == selected;
    let first = text.lines().next().unwrap_or("").to_string();
    let bullet_style = if pinned {
        Style::default().fg(ACCENT)
    } else {
        dim()
    };
    let bullet = Span::styled(" • ", bullet_style);

    let body = if is_selected && focused {
        Span::styled(
            format!(" {first} "),
            Style::default().add_modifier(Modifier::REVERSED),
        )
    } else {
        let style = if pinned {
            Style::default().fg(ACCENT)
        } else if is_selected {
            Style::default()
        } else {
            dim()
        };
        Span::styled(first, style)
    };
    Line::from(vec![bullet, body])
}

fn render_composer(frame: &mut Frame, app: &mut App, cursor_on: bool, area: Rect) {
    app.composer_area = Some(area);
    let focused = app.focus == Pane::Composer && !app.dialog_open() && app.tab_menu.is_none();
    let prefix_style = if focused {
        Style::default().fg(ACCENT)
    } else {
        dim()
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(" › ", prefix_style))),
        Rect::new(area.x, area.y, area.width.min(3), area.height),
    );

    app.composer.set_cursor_visible(focused && cursor_on);
    let editor_area = Rect::new(
        area.x.saturating_add(3),
        area.y,
        area.width.saturating_sub(3),
        area.height,
    );
    frame.render_widget(app.composer.widget(), editor_area);
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let hints = if app.status.is_empty() {
        "p pin  ·  e edit  ·  f open  ·  tab switch"
    } else {
        app.status.as_str()
    };
    frame.render_widget(Paragraph::new(Line::styled(hints, dim())), area);
}

fn render_preview(frame: &mut Frame, app: &mut App, preview_area: Rect) {
    let Some((text, scroll)) = app.preview.as_ref().and_then(|preview| {
        app.workspace
            .get_prompt(preview.id)
            .map(|prompt| (prompt.text.clone(), preview.scroll))
    }) else {
        return;
    };
    if preview_area.width < 8 || preview_area.height < 3 {
        return;
    }
    let block = Block::default()
        .title(" Prompt ")
        .title_bottom(Line::styled(" ↑↓ scroll · Enter copy · Esc close ", dim()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));
    let inner = block.inner(preview_area);
    frame.render_widget(Clear, preview_area);
    frame.render_widget(block, preview_area);

    let lines = wrap_lines(&text, inner.width as usize);
    let total = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    app.preview_page = inner.height.max(1);
    app.preview_max_scroll = total.saturating_sub(inner.height);
    let scroll = scroll.min(app.preview_max_scroll);
    if let Some(preview) = app.preview.as_mut() {
        preview.scroll = scroll;
    }

    frame.render_widget(
        Paragraph::new(lines.into_iter().map(Line::raw).collect::<Vec<_>>()).scroll((scroll, 0)),
        inner,
    );
}

fn wrap_lines(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut wrapped = Vec::new();
    for line in text.split('\n') {
        let mut current = String::new();
        let mut current_width = 0;
        for segment in line.split_inclusive(' ') {
            let word_width = segment.trim_end_matches(' ').chars().count();
            if current_width > 0 && current_width + word_width > width {
                wrapped.push(std::mem::take(&mut current));
                current_width = 0;
            }
            if word_width > width {
                for c in segment.chars() {
                    if current_width == width {
                        wrapped.push(std::mem::take(&mut current));
                        current_width = 0;
                    }
                    current.push(c);
                    current_width += 1;
                }
                continue;
            }
            current.push_str(segment);
            current_width += segment.chars().count();
        }
        wrapped.push(current);
    }
    wrapped
}

fn render_tab_menu(frame: &mut Frame, app: &mut App) {
    app.tab_menu_hits.clear();
    let Some(menu) = &app.tab_menu else {
        return;
    };
    let frame_area = frame.area();
    let width = 12.min(frame_area.width);
    let height = 4.min(frame_area.height);
    let x = menu.column.min(frame_area.right().saturating_sub(width));
    let y = menu
        .row
        .saturating_add(1)
        .min(frame_area.bottom().saturating_sub(height));
    let area = Rect::new(x, y, width, height);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MUTED));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let actions = [
        (TabMenuAction::Rename, " Rename"),
        (TabMenuAction::Close, " Close"),
    ];
    let lines: Vec<_> = actions
        .iter()
        .map(|(action, label)| {
            let style = if menu.selected == *action {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Line::styled(*label, style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);

    for (index, (action, _)) in actions.iter().enumerate() {
        if index as u16 >= inner.height {
            break;
        }
        app.tab_menu_hits.push(TabMenuHit {
            area: Rect::new(inner.x, inner.y + index as u16, inner.width, 1),
            action: *action,
        });
    }
}

fn render_tab_dialog(frame: &mut Frame, app: &App, cursor_on: bool) {
    let Some(dialog) = &app.tab_dialog else {
        return;
    };
    let area = frame.area();
    if area.width < 8 || area.height < 5 {
        return;
    }
    let width = area.width.saturating_sub(4).clamp(8, 52);
    let height = if dialog.error.is_empty() { 5 } else { 7 }.min(area.height);
    let dialog_area = centered_rect(width, height, area);
    let title = match dialog.mode {
        crate::app::TabDialogMode::Create => " New tab ",
        crate::app::TabDialogMode::Rename(_) => " Rename tab ",
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));
    let inner = block.inner(dialog_area);
    frame.render_widget(Clear, dialog_area);
    frame.render_widget(block, dialog_area);

    let mut lines = vec![Line::from(vec![
        Span::styled("Name: ", dim()),
        Span::raw(dialog.value.clone()),
        Span::styled(
            if cursor_on { "█" } else { " " },
            Style::default().fg(ACCENT),
        ),
    ])];
    if !dialog.error.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            dialog.error.clone(),
            Style::default().fg(Color::Red),
        ));
    }
    lines.push(Line::styled("Enter save · Esc cancel", dim()));
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), inner);
}

fn render_close_tab_dialog(frame: &mut Frame, app: &App) {
    let Some(dialog) = &app.close_tab_dialog else {
        return;
    };
    let area = frame.area();
    if area.width < 8 || area.height < 7 {
        return;
    }
    let width = area.width.saturating_sub(4).clamp(8, 52);
    let dialog_area = centered_rect(width, 7, area);
    let block = Block::default()
        .title(" Close tab ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(dialog_area);
    frame.render_widget(Clear, dialog_area);
    frame.render_widget(block, dialog_area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::raw(format!("Close \"{}\"?", dialog.tab_name)),
            Line::raw(""),
            Line::styled("This deletes all prompts in this tab.", dim()),
            Line::styled("Enter close · Esc cancel", dim()),
        ]),
        inner,
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width.min(area.width),
        height.min(area.height),
    )
}

fn dim() -> Style {
    Style::default().fg(MUTED)
}

#[cfg(test)]
mod tests {
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
        assert!(text.contains("type a prompt"));
        assert!(text.contains("p pin"));
        assert!(!text.contains("[ ] tabs"));
        assert!(!text.contains("^t new"));
        assert!(!text.contains("r rename"));
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
        app.preview = Some(crate::app::PromptPreview { id, scroll: 0 });

        let top = render(&mut app, false, 60);
        assert!(top.contains("Prompt"));
        assert!(top.contains("Esc close"));
        assert!(top.contains("line-0"));
        assert!(!top.contains("line-39"));
        assert!(app.preview_max_scroll > 0);

        app.preview.as_mut().unwrap().scroll = u16::MAX;
        let bottom = render(&mut app, false, 60);
        assert!(bottom.contains("line-39"));
        assert_eq!(app.preview.unwrap().scroll, app.preview_max_scroll);
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
}
