use crate::app::{App, Pane};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(5)])
        .split(cols[1]);

    render_queue(f, app, cols[0]);
    render_composer(f, app, right[0]);
    render_details(f, app, right[1]);
    render_footer(f, app, outer[1]);
}

fn render_queue(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Pane::Queue;
    let pinned: Vec<_> = app.queue.iter_pinned().collect();
    let unpinned: Vec<_> = app.queue.iter_unpinned().collect();

    let mut lines: Vec<Line> = Vec::new();
    let mut idx = 0usize;
    for p in &pinned {
        lines.push(row_line(
            idx,
            app.selected,
            "*",
            &short_id(&p.id.to_string()),
            &p.text,
        ));
        idx += 1;
    }
    if !pinned.is_empty() && !unpinned.is_empty() {
        lines.push(Line::raw("  ──────"));
    }
    for p in &unpinned {
        lines.push(row_line(
            idx,
            app.selected,
            " ",
            &short_id(&p.id.to_string()),
            &p.text,
        ));
        idx += 1;
    }
    if lines.is_empty() {
        lines.push(Line::raw("(empty — press Tab, type, Ctrl+S to add)"));
    }

    let block = Block::default()
        .title("queue")
        .borders(Borders::ALL)
        .border_style(border_style(focused));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn row_line(
    idx: usize,
    selected: Option<usize>,
    mark: &str,
    id: &str,
    text: &str,
) -> Line<'static> {
    let cursor = if Some(idx) == selected { ">" } else { " " };
    let spans = vec![
        Span::raw(format!("{cursor}{mark} ")),
        Span::styled(
            format!("{id:<6}"),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::raw(" "),
        Span::raw(text.lines().next().unwrap_or("").to_string()),
    ];
    Line::from(spans)
}

fn short_id(full: &str) -> String {
    full.chars().take(6).collect()
}

fn render_composer(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Pane::Composer;
    let block = Block::default()
        .title("composer")
        .borders(Borders::ALL)
        .border_style(border_style(focused));
    let body = if app.composer.is_empty() && !focused {
        "(Tab to focus, type, Ctrl+S to add)".to_string()
    } else {
        app.composer.clone()
    };
    f.render_widget(Paragraph::new(body).block(block), area);
}

fn render_details(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title("details").borders(Borders::ALL);
    let body = match app.selected_prompt() {
        Some(p) => format!(
            "id: {}\ncreated: {}\n{} chars · {}",
            short_id(&p.id.to_string()),
            p.created_at.format("%Y-%m-%d %H:%M"),
            p.text.chars().count(),
            if p.pinned { "pinned" } else { "unpinned" },
        ),
        None => "(no selection)".to_string(),
    };
    f.render_widget(Paragraph::new(body).block(block), area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let hints = if app.status.is_empty() {
        "[Enter] copy+pop  [y] copy  [p] pin  [e] edit  [J/K] reorder  [Ctrl+S] save  [Ctrl+U] upgrade  [q] quit"
    } else {
        app.status.as_str()
    };
    f.render_widget(Paragraph::new(hints), area);
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}
