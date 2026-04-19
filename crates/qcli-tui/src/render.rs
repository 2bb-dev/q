use crate::app::{App, Pane};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;

pub fn draw(f: &mut Frame, app: &App, cursor_on: bool) {
    let area = f.area();
    let composer_h = composer_height(&app.composer, area.width, area.height);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),          // top spacer
            Constraint::Min(3),             // queue
            Constraint::Length(1),          // spacer
            Constraint::Length(composer_h), // composer
            Constraint::Length(1),          // spacer
            Constraint::Length(1),          // footer
        ])
        .split(area);

    render_queue(f, app, outer[1]);
    render_composer(f, app, cursor_on, outer[3]);
    render_footer(f, app, outer[5]);
}

fn composer_height(composer: &str, term_width: u16, term_height: u16) -> u16 {
    let wrap_width = (term_width as usize).saturating_sub(3).max(1);
    let total: usize = if composer.is_empty() {
        1
    } else {
        composer
            .split('\n')
            .map(|seg| {
                let len = seg.chars().count() + 1;
                len.div_ceil(wrap_width).max(1)
            })
            .sum()
    };
    let max_h = (term_height as usize).saturating_sub(7).max(1);
    total.clamp(1, max_h) as u16
}

fn render_queue(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Pane::Queue;
    let pinned: Vec<_> = app.queue.iter_pinned().collect();
    let unpinned: Vec<_> = app.queue.iter_unpinned().collect();

    let mut lines: Vec<Line> = Vec::new();
    let mut idx = 0usize;
    let mut first = true;

    for p in &pinned {
        if !first {
            lines.push(Line::raw(""));
        }
        lines.push(row_line(idx, app.selected, focused, true, &p.text));
        idx += 1;
        first = false;
    }
    for p in &unpinned {
        if !first {
            lines.push(Line::raw(""));
        }
        lines.push(row_line(idx, app.selected, focused, false, &p.text));
        idx += 1;
        first = false;
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn row_line(
    idx: usize,
    selected: Option<usize>,
    focused: bool,
    pinned: bool,
    text: &str,
) -> Line<'static> {
    let is_selected = Some(idx) == selected;
    let first = text.lines().next().unwrap_or("").to_string();

    let bullet_style = if pinned {
        Style::default().fg(ACCENT)
    } else {
        dim()
    };
    let bullet = Span::styled(" • ", bullet_style);

    let body_span = if is_selected && focused {
        Span::styled(
            format!(" {first} "),
            Style::default().add_modifier(Modifier::REVERSED),
        )
    } else {
        let body_style = if pinned {
            Style::default().fg(ACCENT)
        } else if is_selected {
            Style::default()
        } else {
            dim()
        };
        Span::styled(first, body_style)
    };

    Line::from(vec![bullet, body_span])
}

fn render_composer(f: &mut Frame, app: &App, cursor_on: bool, area: Rect) {
    let focused = app.focus == Pane::Composer;
    let prefix_style = if focused {
        Style::default().fg(ACCENT)
    } else {
        dim()
    };
    let body: Vec<Line> = if app.composer.is_empty() {
        const PLACEHOLDER: &str = "type a prompt… (Tab to focus, Enter to send)";
        let (head, tail) = PLACEHOLDER.split_at(1);
        let head_span = if focused && cursor_on {
            Span::styled(
                head.to_string(),
                Style::default().add_modifier(Modifier::REVERSED),
            )
        } else {
            Span::styled(head.to_string(), dim())
        };
        vec![Line::from(vec![
            Span::styled(" › ", prefix_style),
            head_span,
            Span::styled(tail.to_string(), dim()),
        ])]
    } else {
        let segments: Vec<&str> = app.composer.split('\n').collect();
        let last = segments.len() - 1;
        let mut lines: Vec<Line> = Vec::new();
        for (i, seg) in segments.iter().enumerate() {
            let prefix = if i == 0 {
                Span::styled(" › ", prefix_style)
            } else {
                Span::raw("   ")
            };
            let mut spans = vec![prefix, Span::raw(seg.to_string())];
            if focused && i == last {
                let cursor = if cursor_on {
                    Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED))
                } else {
                    Span::raw(" ")
                };
                spans.push(cursor);
            }
            lines.push(Line::from(spans));
        }
        lines
    };

    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let hints = if app.status.is_empty() {
        "enter send/add  ·  p pin  ·  e edit  ·  tab switch"
    } else {
        app.status.as_str()
    };
    f.render_widget(Paragraph::new(Line::styled(hints, dim())), area);
}

fn dim() -> Style {
    Style::default().fg(MUTED)
}
