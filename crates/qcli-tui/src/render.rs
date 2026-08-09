use crate::app::{App, Pane, TabHit, TabHitTarget};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
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
    render_tab_dialog(frame, app, cursor_on);
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
    for index in start..end {
        if x >= area.right().saturating_sub(CREATE_WIDTH) {
            break;
        }
        let width = widths[index].min(area.right().saturating_sub(CREATE_WIDTH + x));
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
            dim()
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

    let create_x = area.right().saturating_sub(CREATE_WIDTH);
    let create_area = Rect::new(create_x, area.y, CREATE_WIDTH.min(area.width), 1);
    frame.render_widget(
        Paragraph::new(" + ").style(Style::default().fg(ACCENT)),
        create_area,
    );
    app.tab_hits.push(TabHit {
        area: create_area,
        target: TabHitTarget::Create,
    });
}

fn render_queue(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Pane::Queue && app.tab_dialog.is_none();
    let prompts = app.visible_prompts();
    let mut lines = Vec::new();

    for (index, prompt) in prompts.iter().enumerate() {
        if index > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(row_line(
            index,
            app.selected,
            focused,
            prompt.pinned,
            &prompt.text,
        ));
    }
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
    let focused = app.focus == Pane::Composer && app.tab_dialog.is_none();
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
        "[ ] tabs  ·  ^t new  ·  r rename  ·  p pin  ·  e edit  ·  tab switch"
    } else {
        app.status.as_str()
    };
    frame.render_widget(Paragraph::new(Line::styled(hints, dim())), area);
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
    use qcli_core::{Prompt, Workspace};
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
        assert!(text.contains("^t new"));
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
    fn rendered_tabs_and_create_button_have_click_targets() {
        let mut app = App::new(Workspace::new());
        render(&mut app, false, 80);
        assert_eq!(app.tab_hits.len(), 2);
        for hit in &app.tab_hits {
            assert!(app.tab_input_at(hit.area.x, hit.area.y).is_some());
        }
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
