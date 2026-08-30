use crate::app::{
    App, EditorOrigin, Pane, PromptHit, SearchHit, TabHit, TabHitTarget, TabMenuAction, TabMenuHit,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const TAB_BAR_BG: Color = Color::Rgb(32, 32, 32);
const CREATE_WIDTH: u16 = 3;
const BULLET_WIDTH: usize = 3;

pub fn draw(frame: &mut Frame, app: &mut App, cursor_on: bool) {
    if app.editor.is_some() {
        render_editor(frame, app, cursor_on);
        return;
    }
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

    let overlay = Rect::new(
        area.x,
        outer[2].y,
        area.width,
        outer[6].bottom().saturating_sub(outer[2].y),
    );
    render_tabs(frame, app, outer[1]);
    render_queue(frame, app, outer[3]);
    render_composer(frame, app, cursor_on, outer[5]);
    render_footer(frame, app, outer[7]);
    render_search(frame, app, cursor_on, overlay);
    render_preview(frame, app, overlay);
    render_tab_menu(frame, app);
    render_tab_dialog(frame, app, cursor_on);
    render_close_tab_dialog(frame, app);
    render_delete_prompt_dialog(frame, app);
}

fn composer_height(lines: &[String], term_width: u16, term_height: u16) -> u16 {
    let wrap_width = (term_width as usize).saturating_sub(3).max(1);
    let total: usize = lines
        .iter()
        .map(|line| {
            let width = UnicodeWidthStr::width(line.as_str()) + 1;
            width.div_ceil(wrap_width).max(1)
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
        .map(|(_, name)| ((UnicodeWidthStr::width(name.as_str()) + 2) as u16).min(available.max(1)))
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
        let clipped = take_display_width(name, inner_width);
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
    let focused = app.focus == Pane::Queue && !app.overlay_open();
    let prompts: Vec<_> = app
        .visible_prompts()
        .into_iter()
        .map(|prompt| (app.source_card_text(prompt.source()), prompt.pinned()))
        .collect();
    let mut lines = Vec::new();
    let mut hits = Vec::new();
    let mut visual_row = 0;

    for (index, (text, pinned)) in prompts.iter().enumerate() {
        if index > 0 {
            lines.push(Line::raw(""));
            visual_row += 1;
        }
        let rows = collapsed_rows(text, area.width);
        let row_height = if area.width == 0 {
            0
        } else {
            u16::try_from(rows.len()).unwrap_or(u16::MAX)
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
        lines.extend(row_lines(rows, index, app.selected, focused, *pinned));
    }
    app.prompt_hits = hits;
    frame.render_widget(Paragraph::new(lines), area);
}

fn collapsed_rows(text: &str, width: u16) -> Vec<String> {
    let inner_width = (width as usize).saturating_sub(BULLET_WIDTH + 2).max(1);
    let first_line = text.lines().next().unwrap_or("").trim_end();
    if UnicodeWidthStr::width(first_line) <= inner_width {
        return vec![first_line.to_string()];
    }

    let mut truncated = take_display_width(first_line, inner_width.saturating_sub(1));
    truncated.push('…');
    vec![truncated]
}

fn row_lines(
    rows: Vec<String>,
    index: usize,
    selected: Option<usize>,
    focused: bool,
    pinned: bool,
) -> Vec<Line<'static>> {
    let is_selected = Some(index) == selected;
    let bullet_style = if pinned {
        Style::default().fg(ACCENT)
    } else {
        dim()
    };
    let body_style = if pinned {
        Style::default().fg(ACCENT)
    } else if is_selected {
        Style::default()
    } else {
        dim()
    };

    rows.into_iter()
        .enumerate()
        .map(|(row, text)| {
            let prefix = if row == 0 {
                Span::styled(" • ", bullet_style)
            } else {
                Span::raw(" ".repeat(BULLET_WIDTH))
            };
            let body = if is_selected && focused {
                Span::styled(
                    format!(" {text} "),
                    Style::default().add_modifier(Modifier::REVERSED),
                )
            } else {
                Span::styled(text, body_style)
            };
            Line::from(vec![prefix, body])
        })
        .collect()
}

fn render_composer(frame: &mut Frame, app: &mut App, cursor_on: bool, area: Rect) {
    app.composer_area = Some(area);
    let focused = app.focus == Pane::Composer && !app.overlay_open();
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
        "p pin  ·  d delete  ·  e edit  ·  f open  ·  ⌘/ history  ·  tab switch"
    } else {
        app.status.as_str()
    };
    // Align with the composer's `›`, which sits one column into the prefix.
    let area = Rect::new(
        area.x.saturating_add(1),
        area.y,
        area.width.saturating_sub(1),
        area.height,
    );
    frame.render_widget(Paragraph::new(Line::styled(hints, dim())), area);
}

fn render_search(frame: &mut Frame, app: &mut App, cursor_on: bool, area: Rect) {
    app.search_hits.clear();
    let Some(query) = app.search.as_ref().map(|search| search.query.clone()) else {
        return;
    };
    app.refresh_search_folds();
    if area.width < 8 || area.height < 5 {
        return;
    }
    let block = Block::default()
        .title(" History ")
        .title_bottom(Line::styled(
            " type to filter · ↑↓ select · Enter open · ^d forget · Esc close ",
            dim(),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("/ ", dim()),
            Span::raw(query),
            Span::styled(
                if cursor_on { "█" } else { " " },
                Style::default().fg(ACCENT),
            ),
        ])),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let list_area = Rect::new(
        inner.x,
        inner.y.saturating_add(1),
        inner.width,
        inner.height.saturating_sub(1),
    );
    if list_area.height == 0 {
        return;
    }

    let results: Vec<String> = app
        .search_results()
        .iter()
        .map(|entry| {
            let text = match entry.external_markdown_path() {
                Some(path) => format!(
                    "{}  {}",
                    path.display(),
                    app.source_card_text(entry.source())
                ),
                None => app.source_card_text(entry.source()),
            };
            condense(&text, list_area.width.saturating_sub(2) as usize)
        })
        .collect();
    if results.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(" no matching prompts", dim())),
            list_area,
        );
        return;
    }

    let selected = app
        .search
        .as_ref()
        .map(|search| search.selected)
        .unwrap_or(0)
        .min(results.len() - 1);
    if let Some(search) = app.search.as_mut() {
        search.selected = selected;
    }
    let height = list_area.height as usize;
    let offset = selected.saturating_sub(height.saturating_sub(1));

    let mut lines = Vec::new();
    for (row, text) in results.iter().skip(offset).take(height).enumerate() {
        let index = offset + row;
        let style = if index == selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::styled(format!(" {text} "), style));
        app.search_hits.push(SearchHit {
            area: Rect::new(list_area.x, list_area.y + row as u16, list_area.width, 1),
            index,
        });
    }
    frame.render_widget(Paragraph::new(lines), list_area);
}

fn condense(text: &str, width: usize) -> String {
    let condensed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let width = width.max(1);
    if UnicodeWidthStr::width(condensed.as_str()) <= width {
        return condensed;
    }
    let mut clipped = take_display_width(&condensed, width.saturating_sub(1));
    clipped.push('…');
    clipped
}

fn render_preview(frame: &mut Frame, app: &mut App, preview_area: Rect) {
    let Some(scroll) = app.preview.as_ref().map(|preview| preview.scroll) else {
        return;
    };
    let text = match app.preview_text() {
        Ok(text) => text,
        Err(_) => app
            .preview_source()
            .map(|source| app.source_card_text(source))
            .unwrap_or_else(|| "prompt is no longer available".to_string()),
    };
    if preview_area.width < 8 || preview_area.height < 3 {
        return;
    }
    let block = Block::default()
        .title(format!(" {} ", app.preview_title()))
        .title_bottom(Line::styled(
            " ↑↓ scroll · Enter copy · e edit · Esc close ",
            dim(),
        ))
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
        let mut emitted = false;
        for segment in line.split_inclusive(' ') {
            let segment_width = UnicodeWidthStr::width(segment);
            if current_width > 0 && current_width + segment_width > width {
                wrapped.push(std::mem::take(&mut current));
                current_width = 0;
                emitted = true;
            }
            if segment_width > width {
                for grapheme in UnicodeSegmentation::graphemes(segment, true) {
                    let grapheme_width = UnicodeWidthStr::width(grapheme);
                    if grapheme_width > width {
                        if !current.is_empty() {
                            wrapped.push(std::mem::take(&mut current));
                            current_width = 0;
                        }
                        wrapped.push("…".to_string());
                        emitted = true;
                        continue;
                    }
                    if current_width > 0 && current_width + grapheme_width > width {
                        wrapped.push(std::mem::take(&mut current));
                        current_width = 0;
                        emitted = true;
                    }
                    current.push_str(grapheme);
                    current_width += grapheme_width;
                }
                continue;
            }
            current.push_str(segment);
            current_width += segment_width;
        }
        if !current.is_empty() || !emitted {
            wrapped.push(current);
        }
    }
    wrapped
}

fn take_display_width(text: &str, max_width: usize) -> String {
    let mut width = 0;
    UnicodeSegmentation::graphemes(text, true)
        .take_while(|grapheme| {
            let grapheme_width = UnicodeWidthStr::width(*grapheme);
            if width + grapheme_width > max_width {
                return false;
            }
            width += grapheme_width;
            true
        })
        .collect()
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

fn render_delete_prompt_dialog(frame: &mut Frame, app: &App) {
    let Some(dialog) = &app.delete_prompt_dialog else {
        return;
    };
    let area = frame.area();
    if area.width < 8 || area.height < 6 {
        return;
    }
    let width = area.width.saturating_sub(4).clamp(8, 58);
    let dialog_area = centered_rect(width, 6, area);
    let block = Block::default()
        .title(" Remove queue record ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(dialog_area);
    frame.render_widget(Clear, dialog_area);
    frame.render_widget(block, dialog_area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::raw(format!(
                "Remove prompt {} from the queue?",
                dialog.prompt_id
            )),
            Line::styled("History and any source file are preserved.", dim()),
            Line::styled("Enter remove · Esc cancel", dim()),
        ]),
        inner,
    );
}

fn render_editor(frame: &mut Frame, app: &mut App, cursor_on: bool) {
    let area = frame.area();
    let Some(editor) = app.editor.as_mut() else {
        return;
    };
    let (row, column) = editor.buffer.cursor();
    let dirty = if editor.is_dirty() {
        " · modified"
    } else {
        ""
    };
    let source_kind = match editor.origin {
        EditorOrigin::Inline { .. } => "inline",
        EditorOrigin::External { .. } => "external",
    };
    let block = Block::default()
        .title(" Edit ")
        .title_bottom(Line::styled(
            format!(
                " {source_kind} · Ln {}, Col {}{dirty} · Ctrl-S save · Esc close ",
                row + 1,
                column + 1
            ),
            dim(),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let error_height = u16::from(!editor.error.is_empty() && inner.height > 1);
    let edit_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(error_height),
    );
    editor.buffer.set_cursor_visible(cursor_on);
    frame.render_widget(editor.buffer.widget(), edit_area);
    if error_height > 0 {
        frame.render_widget(
            Paragraph::new(Line::styled(
                editor.error.clone(),
                Style::default().fg(Color::Red),
            )),
            Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
        );
    }

    if editor.discard_confirmation && area.width >= 8 && area.height >= 6 {
        let dialog_area = centered_rect(area.width.saturating_sub(4).clamp(8, 52), 6, area);
        let block = Block::default()
            .title(" Discard changes? ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));
        let inner = block.inner(dialog_area);
        frame.render_widget(Clear, dialog_area);
        frame.render_widget(block, dialog_area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw("The complete unsaved buffer will be discarded."),
                Line::styled("Enter discard · Esc keep editing", dim()),
            ]),
            inner,
        );
    }
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
#[path = "../tests/unit/render.rs"]
mod tests;
