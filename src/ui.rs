use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{App, Modal},
    grid::Placement,
    theme,
    tiles::accent,
};

pub const HELP_LINES: &[&str] = &[
    "DASHBOARD",
    "e edit layout · r reload TOML · q quit",
    "",
    "LAYOUT EDITOR",
    "Tab / Shift+Tab: next / previous tile",
    "Arrows: preview movement",
    "Shift+arrows: preview resize",
    "h / l: shrink / grow width",
    "k / j: shrink / grow height",
    "Enter: apply preview · Esc: discard preview",
    "Drag a tile to move; drag ◢ to resize",
    "a add · d delete · t settings · u undo",
    "p: edit next responsive profile",
    "s: save all edits and return to live",
    "Esc without a preview: cancel entire session",
    "",
    "SETTINGS",
    "Tab: next field · Ctrl+u: clear field",
    "Left/Right, Home/End: move text cursor",
    "Backspace/Delete: remove text · paste supported",
    "Enter: apply · Esc: cancel settings",
    "",
    "Profiles are independent. Only the shown profile changes.",
    "Edit grid dimensions and resize rules in TOML.",
    "Ctrl+c exits, discarding unsaved edits.",
    "",
    "↑/↓ or PgUp/PgDn scroll · Esc closes",
];

pub fn draw(frame: &mut Frame, app: &mut App) {
    app.resize(frame.area());
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::BACKGROUND).fg(theme::TEXT)),
        area,
    );
    if area.width < 26 || area.height < 10 {
        frame.render_widget(
            Paragraph::new("Tileboard\nEnlarge to 26 × 10\nCtrl+c quit").wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    let profile = &app.config.profiles[app.profile];
    let dirty = app.is_dirty();
    let header = Line::from(vec![
        Span::styled(
            " TILEBOARD ",
            Style::default()
                .fg(theme::CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if app.editing {
                if dirty { " EDIT • " } else { " EDIT " }
            } else {
                " LIVE "
            },
            Style::default().fg(theme::BACKGROUND).bg(if app.editing {
                theme::YELLOW
            } else {
                theme::GREEN
            }),
        ),
        Span::styled(
            if area.width >= 55 {
                format!("   {} · {}×{}", profile.name, profile.columns, profile.rows)
            } else {
                String::new()
            },
            Style::default().fg(theme::MUTED),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(header),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let subtitle = if app.editing {
        format!(
            " {} profile · {} {} · {}",
            profile.name,
            profile.tiles.len(),
            if profile.tiles.len() == 1 {
                "tile"
            } else {
                "tiles"
            },
            if dirty { "unsaved" } else { "unchanged" }
        )
    } else {
        " A little clarity for your terminal".into()
    };
    frame.render_widget(
        Paragraph::new(subtitle).style(Style::default().fg(theme::MUTED)),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );

    let grid = app.grid();
    if app.editing {
        for row in 0..profile.rows {
            for column in 0..profile.columns {
                let rect = grid.rect(Placement {
                    column,
                    row,
                    column_span: 1,
                    row_span: 1,
                });
                frame.render_widget(
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(theme::BORDER)),
                    rect,
                );
            }
        }
    }
    if profile.tiles.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.editing {
                "An empty canvas.\nPress a to add your first tile."
            } else {
                "An empty canvas.\nPress e, then a to add a tile."
            })
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme::MUTED)),
            grid.area,
        );
    }
    for (index, config) in profile.tiles.iter().enumerate() {
        let rect = app.tile_rect(config.placement);
        if rect.is_empty() {
            continue;
        }
        let selected = app.editing && index == app.selected;
        let color = if selected {
            theme::YELLOW
        } else {
            accent(&config.accent).unwrap_or(theme::CYAN)
        };
        let title = if selected {
            format!(
                " {} · {}×{} ",
                config.title, config.placement.column_span, config.placement.row_span
            )
        } else {
            format!(" {} ", config.title)
        };
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                title,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ))
            .style(theme::surface())
            .border_style(Style::default().fg(if selected { color } else { theme::BORDER }));
        let mut inner = block.inner(rect);
        if inner.width >= 16 {
            inner.x += 1;
            inner.width -= 2;
        }
        frame.render_widget(block, rect);
        let key = (profile.name.clone(), config.id.clone(), config.kind.clone());
        if let Some(tile) = app.tiles.get(&key) {
            let minimum = tile.minimum_size();
            if rect.width < minimum.0 || rect.height < minimum.1 {
                frame.render_widget(
                    Paragraph::new("Enlarge tile").style(Style::default().fg(theme::MUTED)),
                    inner,
                );
            } else {
                tile.render(frame, inner, config, &app.metrics);
            }
        }
        if selected && rect.width > 2 && rect.height > 2 {
            frame.render_widget(
                Paragraph::new("◢").style(Style::default().fg(theme::YELLOW)),
                Rect::new(rect.right() - 2, rect.bottom() - 1, 1, 1),
            );
        }
    }
    if let Some(candidate) = app.candidate {
        let valid = app.candidate_valid();
        frame.render_widget(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .title(if valid {
                    " Preview · Enter "
                } else {
                    " Blocked "
                })
                .border_style(Style::default().fg(if valid { theme::GREEN } else { theme::RED })),
            app.tile_rect(candidate),
        );
    }
    let status = if app.metrics.sampled_at.elapsed().as_secs() > 5 {
        format!("Metrics delayed · {}", app.status)
    } else {
        app.status.clone()
    };
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(
            if app.candidate.is_some() && !app.candidate_valid() {
                theme::RED
            } else {
                theme::MUTED
            },
        )),
        Rect::new(area.x + 1, area.bottom() - 3, area.width - 2, 1),
    );
    let footer = match &app.modal {
        Some(Modal::Settings { .. }) => "Tab field  Enter apply\nEsc close  Ctrl+u clear",
        Some(Modal::Add { .. }) => "↑/↓ select  Enter add\nEsc close",
        Some(Modal::Help { .. }) => "↑/↓ scroll  Esc close",
        None if app.candidate.is_some() => {
            "Enter apply  Esc discard\nArrows move  h/l width  k/j height"
        }
        None if app.editing && area.width < 75 => {
            "s save  Esc cancel  ? help\nTab select  a add  t settings"
        }
        None if app.editing => {
            "Tab select  arrows move  h/j/k/l resize  a add  t settings  d delete\ns save  Esc cancel  Enter apply  u undo  p profile  ? help"
        }
        _ => "e edit   ? help   q quit",
    };
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(theme::TEXT)),
        Rect::new(area.x + 1, area.bottom() - 2, area.width - 2, 2),
    );
    if let Some(modal) = &app.modal {
        frame
            .buffer_mut()
            .set_style(app.board_area(), Style::default().fg(theme::BORDER));
        draw_modal(frame, app, modal);
    }
}

/// Choose a grapheme boundary that keeps the insertion point inside the field.
fn input_window(value: &str, cursor: usize, width: usize) -> (String, usize) {
    let before = &value[..cursor];
    let mut start = 0;
    for (index, _) in before.grapheme_indices(true) {
        if before[index..].width() < width.max(1) {
            start = index;
            break;
        }
        start = cursor;
    }
    (value[start..].to_string(), value[start..cursor].width())
}

fn draw_modal(frame: &mut Frame, app: &App, modal: &Modal) {
    let area = frame.area();
    let width = area.width.saturating_sub(4).min(68);
    let inner_width = width.saturating_sub(4) as usize;
    let (title, lines, cursor_row): (&str, Vec<Line<'_>>, Option<(usize, usize)>) = match modal {
        Modal::Help { scroll } => (
            " Help ",
            HELP_LINES
                .iter()
                .skip(*scroll)
                .map(|line| Line::from(*line))
                .collect(),
            None,
        ),
        Modal::Add { selected } => {
            let mut lines = vec![Line::from("Choose a tile"), Line::from("")];
            for (i, definition) in app.registry.list().iter().enumerate() {
                lines.push(
                    Line::from(format!(
                        "{} {}",
                        if i == *selected { "›" } else { " " },
                        definition.name
                    ))
                    .style(Style::default().fg(if i == *selected {
                        theme::YELLOW
                    } else {
                        theme::TEXT
                    })),
                );
            }
            (" Add tile ", lines, None)
        }
        Modal::Settings {
            fields,
            selected,
            cursor,
        } => {
            let mut lines = vec![];
            let mut cursor_row = None;
            for (i, (label, value)) in fields.iter().enumerate() {
                lines.push(
                    Line::from(format!(
                        "{} {label}",
                        if i == *selected { "›" } else { " " }
                    ))
                    .style(Style::default().fg(if i == *selected {
                        theme::YELLOW
                    } else {
                        theme::MUTED
                    })),
                );
                let shown = if i == *selected {
                    let (text, column) = input_window(value, *cursor, inner_width);
                    cursor_row = Some((i * 3 + 1, column + 1));
                    text
                } else {
                    value.clone()
                };
                lines.push(Line::from(format!(" {shown}")));
                lines.push(Line::from(""));
            }
            (" Tile settings ", lines, cursor_row)
        }
    };
    let max_height = area.height.saturating_sub(5);
    let height = max_height.min(lines.len() as u16 + 2).max(4);
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + 2 + (max_height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .style(theme::surface())
        .border_style(Style::default().fg(theme::CYAN));
    let visible = height.saturating_sub(2) as usize;
    let scroll = match modal {
        Modal::Settings { selected, .. } => (selected * 3 + 2).saturating_sub(visible),
        Modal::Add { selected } => (selected + 3).saturating_sub(visible),
        _ => 0,
    };
    let mut paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    if matches!(modal, Modal::Help { .. }) {
        paragraph = paragraph.wrap(Wrap { trim: false });
    }
    frame.render_widget(paragraph.block(block), rect);
    if let Some((row, column)) = cursor_row {
        frame.set_cursor_position((
            rect.x + 1 + column as u16,
            rect.y + 1 + (row - scroll) as u16,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, metrics::Metrics, tiles::Registry};
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn render_survives_tiny_wide_tall_and_edit_sizes() {
        let mut app = App::new(Config::default(), "unused.toml".into(), Registry::builtin());
        app.update_metrics(Metrics {
            cpu: Some(42.0),
            cores: vec![42.0; 4],
            ..Metrics::default()
        });
        for (width, height) in [
            (0, 0),
            (1, 1),
            (25, 9),
            (26, 10),
            (40, 20),
            (80, 24),
            (120, 40),
            (200, 12),
            (30, 100),
        ] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| draw(frame, &mut app)).unwrap();
            if width >= 26 && height >= 10 {
                app.handle_key(crossterm::event::KeyCode::Char('e').into());
                terminal.draw(|frame| draw(frame, &mut app)).unwrap();
                app.handle_key(crossterm::event::KeyCode::Char('t').into());
                app.paste("long title 한글 👨‍💻 with lots of text beyond the field width");
                terminal.draw(|frame| draw(frame, &mut app)).unwrap();
                app.handle_key(crossterm::event::KeyCode::Esc.into());
                app.handle_key(crossterm::event::KeyCode::Char('?').into());
                terminal.draw(|frame| draw(frame, &mut app)).unwrap();
                app.handle_key(crossterm::event::KeyCode::Esc.into());
                app.handle_key(crossterm::event::KeyCode::Esc.into());
            }
        }
    }
    #[test]
    fn long_unicode_inputs_keep_the_caret_visible() {
        for value in [
            "a long title beyond the field",
            "한글 타일 이름입니다",
            "👨‍💻👨‍💻👨‍💻name",
        ] {
            for width in 3..20 {
                for cursor in value.char_indices().map(|(i, _)| i).chain([value.len()]) {
                    let (_, column) = input_window(value, cursor, width);
                    assert!(column < width);
                }
            }
        }
    }
}
