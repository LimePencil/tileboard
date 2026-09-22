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
    app::{App, Candidate, Modal},
    grid::Placement,
    theme,
    tiles::{accent, plain_text},
};

pub const HELP_LINES: &[&str] = &[
    "DASHBOARD",
    "e edit layout · r reload TOML · q quit",
    "p: choose saved profile or Auto",
    "[ / ]: previous / next saved profile",
    "Profile choices save immediately and survive restarts.",
    "Auto follows terminal size; manual choices stay selected.",
    "",
    "LAYOUT EDITOR",
    "Tab / Shift+Tab: next / previous tile",
    "Arrows: move into free cells or swap occupied tiles",
    "Swaps exchange full slots, including their sizes",
    "Shift+arrows: preview resize",
    "h / l: shrink / grow width",
    "k / j: shrink / grow height",
    "Enter: apply preview · Esc: discard preview",
    "Drag a tile to move; drag ◢ to resize",
    "a add · r replace · d delete · t settings · u undo",
    "p: edit next profile · c: cycle theme",
    "g: profile name, grid dimensions, and resize rules",
    "n: save a copy as a named profile",
    "New copies are manual; enable automatic in g settings.",
    "Refresh interval: milliseconds, 250–86400000",
    "s: save all edits and return to live",
    "Esc without a preview: cancel entire session",
    "",
    "TILE / PROFILE SETTINGS",
    "Tab: next field · Ctrl+u: clear field",
    "Left/Right, Home/End: move text cursor",
    "Backspace/Delete: remove text · paste supported",
    "Enter: stage changes · Esc: cancel settings",
    "Profile rules: leave optional maximum/aspect limits blank.",
    "Automatic: true / false; keep an automatic fallback.",
    "",
    "Tile edits affect this profile; themes apply everywhere.",
    "Saved profiles include their complete tile layout.",
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
        app.config.theme.apply(frame.buffer_mut());
        return;
    }
    let status = app.visible_status(std::time::Instant::now()).to_string();
    let profile = &app.config.profiles[app.profile];
    let profile_name = plain_text(&profile.name);
    let profile_mode = if app.config.active_profile.is_some() {
        "Manual"
    } else {
        "Auto"
    };
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
                format!(
                    "   {}×{} · {} {}",
                    profile.columns,
                    profile.rows,
                    profile.tiles.len(),
                    if profile.tiles.len() == 1 {
                        "tile"
                    } else {
                        "tiles"
                    }
                )
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
            " {profile_mode} · {profile_name} · {}",
            if dirty { "unsaved" } else { "unchanged" }
        )
    } else {
        format!(" {profile_mode} · {profile_name}")
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
                if !rect.is_empty() {
                    frame.render_widget(
                        Paragraph::new("·").style(Style::default().fg(theme::BORDER)),
                        Rect::new(rect.x, rect.y, 1, 1),
                    );
                }
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
    let preview = app.placement_preview().unwrap_or_default();
    for (index, config) in profile.tiles.iter().enumerate() {
        let placement = preview
            .iter()
            .find(|(i, _)| *i == index)
            .map(|(_, p)| *p)
            .unwrap_or(config.placement);
        let affected = preview.iter().any(|(i, _)| *i == index);
        let rect = app.tile_rect(placement);
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
                config.title, placement.column_span, placement.row_span
            )
        } else {
            format!(" {} ", config.title)
        };
        let definition = app.registry.get(&config.kind).unwrap();
        let interval = theme::interval(config.refresh_ms.unwrap_or(definition.default_refresh_ms));
        let key = (profile.name.clone(), config.id.clone(), config.kind.clone());
        let delayed = app
            .tiles
            .get(&key)
            .and_then(|state| state.pending_since)
            .is_some_and(|time| time.elapsed().as_secs() >= 5);
        let annotation = if affected {
            if preview.len() == 2 {
                " swap preview ".into()
            } else {
                " preview ".into()
            }
        } else if delayed {
            " delayed ".into()
        } else {
            format!(" {interval} ")
        };
        let block = Block::bordered()
            .title_bottom(
                Line::from(annotation).style(Style::default().fg(if delayed {
                    theme::RED
                } else {
                    theme::MUTED
                })),
            )
            .border_type(if selected {
                BorderType::Double
            } else {
                BorderType::Rounded
            })
            .title(Span::styled(
                title,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ))
            .style(if affected {
                theme::surface().bg(theme::PREVIEW_OK)
            } else {
                theme::surface()
            })
            .border_style(Style::default().fg(if selected {
                color
            } else if affected {
                theme::GREEN
            } else {
                theme::BORDER
            }));
        let mut inner = block.inner(rect);
        if inner.width >= 16 {
            inner.x += 1;
            inner.width -= 2;
        }
        frame.render_widget(Clear, rect);
        frame.render_widget(block, rect);
        let key = (profile.name.clone(), config.id.clone(), config.kind.clone());
        if let Some(tile) = app.tiles.get(&key) {
            let minimum = tile.tile.minimum_size();
            if rect.width < minimum.0 || rect.height < minimum.1 {
                frame.render_widget(
                    Paragraph::new("Enlarge tile").style(Style::default().fg(theme::MUTED)),
                    inner,
                );
            } else {
                tile.tile.render(frame, inner, config, &tile.metrics);
            }
        }
        if selected && rect.width > 2 && rect.height > 2 {
            frame.render_widget(
                Paragraph::new("◢").style(Style::default().fg(theme::YELLOW)),
                Rect::new(rect.right() - 2, rect.bottom() - 1, 1, 1),
            );
        }
    }
    if let Some(Candidate::Place(candidate)) = app.candidate.filter(|_| !app.candidate_valid()) {
        let valid = app.candidate_valid();
        frame.render_widget(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .title(format!(
                    " {} {}×{} ",
                    if valid { "Preview" } else { "Blocked" },
                    candidate.column_span,
                    candidate.row_span
                ))
                .style(Style::default().bg(if valid {
                    theme::PREVIEW_OK
                } else {
                    theme::PREVIEW_BAD
                }))
                .border_style(Style::default().fg(if valid { theme::GREEN } else { theme::RED })),
            app.tile_rect(candidate),
        );
    }
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
        Some(Modal::Settings { .. } | Modal::ProfileSettings { .. }) => {
            "Tab field  Enter apply\nEsc close  Ctrl+u clear"
        }
        Some(Modal::SaveProfile { .. }) => "Enter create copy\nEsc close  Ctrl+u clear",
        Some(Modal::Profiles { .. }) => "↑/↓ select  Enter switch\nEsc close",
        Some(Modal::Add { replace: true, .. }) => "↑/↓ select  Enter replace\nEsc close",
        Some(Modal::Add { .. }) => "↑/↓ select  Enter add\nEsc close",
        Some(Modal::Help { .. }) => "↑/↓ scroll  Esc close",
        None if app.candidate.is_some() => {
            "Enter apply  Esc discard\nArrows move  h/l width  k/j height"
        }
        None if app.editing && area.width < 75 => {
            "s save  Esc cancel  ? help\ng profile  n copy  p next"
        }
        None if app.editing => {
            "Tab select  arrows move  h/j/k/l resize  a add  r replace  t settings\ns save  Esc cancel  u undo  p next  g profile  n copy  c theme  ? help"
        }
        None if area.width < 50 => "p profiles  [/] switch\ne edit  ? help  q quit",
        _ => "e edit  p profiles  [/] switch  ? help  q quit",
    };
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(if app.editing || app.modal.is_some() {
            theme::TEXT
        } else {
            theme::MUTED
        })),
        Rect::new(area.x + 1, area.bottom() - 2, area.width - 2, 2),
    );
    if let Some(modal) = &app.modal {
        frame
            .buffer_mut()
            .set_style(app.board_area(), Style::default().fg(theme::BORDER));
        draw_modal(frame, app, modal);
    }
    app.config.theme.apply(frame.buffer_mut());
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
        Modal::Add { selected, replace } => {
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
            (
                if *replace {
                    " Replace tile "
                } else {
                    " Add tile "
                },
                lines,
                None,
            )
        }
        Modal::Profiles { selected } => {
            let mut lines = Vec::with_capacity((app.config.profiles.len() + 1) * 2);
            for index in 0..=app.config.profiles.len() {
                let (name, details, active) = if index == 0 {
                    (
                        "Auto".to_string(),
                        "Follow terminal size".to_string(),
                        app.config.active_profile.is_none(),
                    )
                } else {
                    let profile = &app.config.profiles[index - 1];
                    (
                        plain_text(&profile.name),
                        format!(
                            "{}×{} · {}",
                            profile.columns,
                            profile.rows,
                            if profile.automatic {
                                "automatic"
                            } else {
                                "manual only"
                            }
                        ),
                        app.config.active_profile.as_deref() == Some(profile.name.as_str()),
                    )
                };
                lines.push(
                    Line::from(format!(
                        "{} {name}{}",
                        if index == *selected { "›" } else { " " },
                        if active { " ✓" } else { "" }
                    ))
                    .style(Style::default().fg(if index == *selected {
                        theme::YELLOW
                    } else {
                        theme::TEXT
                    })),
                );
                lines.push(
                    Line::from(format!("  {details}")).style(Style::default().fg(theme::MUTED)),
                );
            }
            (" Profiles ", lines, None)
        }
        Modal::Settings {
            fields,
            selected,
            cursor,
        }
        | Modal::ProfileSettings {
            fields,
            selected,
            cursor,
        }
        | Modal::SaveProfile {
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
            (
                match modal {
                    Modal::ProfileSettings { .. } => " Profile settings ",
                    Modal::SaveProfile { .. } => " Save profile copy ",
                    _ => " Tile settings ",
                },
                lines,
                cursor_row,
            )
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
        Modal::Settings { selected, .. }
        | Modal::ProfileSettings { selected, .. }
        | Modal::SaveProfile { selected, .. } => (selected * 3 + 2).saturating_sub(visible),
        Modal::Profiles { selected } => (selected * 2 + 2).saturating_sub(visible),
        Modal::Add { selected, .. } => (selected + 3).saturating_sub(visible),
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

    fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn profile_selection_mode_and_name_remain_visible_on_small_screens() {
        let mut config = Config::default();
        config.profiles.last_mut().unwrap().name = "Desk\nwork".into();
        let mut app = App::new(config, "unused.toml".into(), Registry::builtin());
        let mut terminal = Terminal::new(TestBackend::new(26, 10)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        assert!(rendered_text(&terminal).contains("Auto · Desk work"));
        app.config.active_profile = Some("Desk\nwork".into());
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        assert!(rendered_text(&terminal).contains("Manual · Desk work"));
    }

    #[test]
    fn profile_picker_scrolls_selected_profile_and_details_into_view() {
        let mut config = Config::default();
        let mut saved = config.profiles.last().unwrap().clone();
        saved.name = "Saved\nwork".into();
        saved.automatic = false;
        let selected = config.profiles.len();
        config.profiles.insert(selected - 1, saved);
        let mut app = App::new(config, "unused.toml".into(), Registry::builtin());
        app.modal = Some(Modal::Profiles { selected });
        for (width, height) in [(26, 10), (38, 16), (120, 40), (30, 100)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| draw(frame, &mut app)).unwrap();
            let output = rendered_text(&terminal);
            assert!(output.contains("› Saved work"), "{width}×{height}");
            assert!(output.contains("2×2 · manual only"), "{width}×{height}");
        }
        app.modal = Some(Modal::Profiles { selected: 0 });
        let mut terminal = Terminal::new(TestBackend::new(26, 10)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        assert!(rendered_text(&terminal).contains("› Auto ✓"));
    }

    #[test]
    fn profile_forms_scroll_to_selected_field_and_keep_unicode_caret_visible() {
        let mut app = App::new(Config::default(), "unused.toml".into(), Registry::builtin());
        let value = "a long name with 한글 👨‍💻 final";
        for (width, height) in [(26, 10), (38, 16), (120, 40), (30, 100)] {
            for settings in [true, false] {
                let mut fields = vec![("Earlier field".into(), String::new()); 9];
                fields.push(("Profile name".into(), value.into()));
                app.modal = Some(if settings {
                    Modal::ProfileSettings {
                        selected: fields.len() - 1,
                        fields,
                        cursor: value.len(),
                    }
                } else {
                    Modal::SaveProfile {
                        fields: vec![("Profile name".into(), value.into())],
                        selected: 0,
                        cursor: value.len(),
                    }
                });
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal.draw(|frame| draw(frame, &mut app)).unwrap();
                let output = rendered_text(&terminal);
                assert!(output.contains("Profile name"), "{width}×{height}");
                assert!(output.contains("final"), "{width}×{height}");
                assert!(output.contains(if settings {
                    "Profile settings"
                } else {
                    "Save profile copy"
                }));
                let cursor = terminal.get_cursor_position().unwrap();
                assert!(cursor.x < width - 2 && cursor.y < height - 3);
                assert_eq!(
                    terminal.backend().buffer()[(cursor.x, cursor.y)].symbol(),
                    " "
                );
            }
        }
    }

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
