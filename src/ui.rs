use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    app::{App, Modal},
    grid::Placement,
    tiles::accent,
};

const MUTED: Color = Color::DarkGray;

pub const HELP_LINES: &[&str] = &[
    "DASHBOARD",
    "e  Edit layout       r  Reload TOML       q  Quit",
    "",
    "LAYOUT EDITOR",
    "Tab / Shift+Tab     Select next / previous tile",
    "Arrow keys         Preview movement",
    "Shift+arrows       Preview resize",
    "h / l, k / j       Shrink / grow width, height",
    "Enter              Apply valid preview",
    "Mouse              Drag tile; drag ◢ corner to resize",
    "a / d / t          Add / delete / configure tile",
    "u                  Undo last applied edit",
    "p                  Preview/edit next responsive profile",
    "s                  Save all profiles and return to live",
    "Esc                Discard preview, then cancel session",
    "",
    "Profiles are independent. Edits affect the shown profile.",
    "Resize rules and grid dimensions are edited in TOML.",
    "Settings: Tab selects a field, Ctrl+u clears it.",
    "Ctrl+c exits immediately, discarding unsaved edits.",
    "",
    "↑/↓ or PgUp/PgDn scroll · any other key closes",
];

pub fn draw(frame: &mut Frame, app: &mut App) {
    app.resize(frame.area());
    let area = frame.area();
    if area.width < 26 || area.height < 10 {
        frame.render_widget(
            Paragraph::new("Tileboard\nEnlarge to at least 26 × 10\nq quit · Esc cancels edits")
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    let profile = &app.config.profiles[app.profile];
    let mode = if app.editing { " EDIT " } else { " LIVE " };
    let header = Line::from(vec![
        Span::styled(
            " TILEBOARD ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            mode,
            Style::default().fg(Color::Black).bg(if app.editing {
                Color::Yellow
            } else {
                Color::Green
            }),
        ),
        Span::raw(format!(
            "  {} · {}×{} grid",
            profile.name, profile.columns, profile.rows
        )),
        Span::styled(
            format!("  {}×{} terminal", area.width, area.height),
            Style::default().fg(MUTED),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(header),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let subtitle = if app.editing {
        " Arrange your space · selected tile highlighted · drag corner to resize"
    } else {
        " Your terminal, at a glance"
    };
    frame.render_widget(
        Paragraph::new(subtitle).style(Style::default().fg(MUTED)),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );

    let grid = app.grid();
    if app.editing {
        for row in 0..profile.rows {
            for column in 0..profile.columns {
                frame.render_widget(
                    Block::bordered().border_style(Style::default().fg(MUTED)),
                    grid.rect(Placement {
                        column,
                        row,
                        column_span: 1,
                        row_span: 1,
                    }),
                );
            }
        }
    }
    if profile.tiles.is_empty() {
        frame.render_widget(
            Paragraph::new("No tiles in this profile.\nPress e, then a to add a tile.")
                .alignment(Alignment::Center),
            grid.area,
        );
    }
    for (index, config) in profile.tiles.iter().enumerate() {
        let rect = grid.rect(config.placement);
        if rect.is_empty() {
            continue;
        }
        let selected = app.editing && index == app.selected;
        let color = if selected {
            Color::Yellow
        } else {
            accent(&config.accent).unwrap_or(Color::Cyan)
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
            .title(title)
            .border_style(Style::default().fg(color));
        let inner = block.inner(rect);
        frame.render_widget(Clear, rect);
        frame.render_widget(block, rect);
        let key = (profile.name.clone(), config.id.clone(), config.kind.clone());
        if let Some(tile) = app.tiles.get(&key) {
            let minimum = tile.minimum_size();
            if rect.width < minimum.0 || rect.height < minimum.1 {
                frame.render_widget(
                    Paragraph::new("Too small").style(Style::default().fg(MUTED)),
                    inner,
                );
            } else {
                tile.render(frame, inner, config, &app.metrics);
            }
        }
        if selected && rect.width > 2 && rect.height > 2 {
            frame.render_widget(
                Paragraph::new("◢").style(Style::default().fg(Color::Yellow)),
                Rect::new(rect.right() - 2, rect.bottom() - 1, 1, 1),
            );
        }
    }
    if let Some(candidate) = app.candidate {
        let valid = app.candidate_valid();
        let color = if valid { Color::Green } else { Color::Red };
        let rect = grid.rect(candidate);
        frame.render_widget(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .title(if valid {
                    " Preview · Enter "
                } else {
                    " Blocked "
                })
                .border_style(Style::default().fg(color)),
            rect,
        );
    }

    let status = if !app.editing && app.metrics.sampled_at.elapsed().as_secs() > 5 {
        format!("Metrics delayed · {}", app.status)
    } else {
        app.status.clone()
    };
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(
            if app.candidate.is_some() && !app.candidate_valid() {
                Color::Red
            } else {
                Color::Yellow
            },
        )),
        Rect::new(area.x + 1, area.bottom() - 3, area.width - 2, 1),
    );
    let footer = if app.editing {
        "Tab select  arrows move  Shift+arrows resize  Enter apply  a add  d delete  t settings  u undo  p profile  s save  Esc cancel  ? help"
    } else {
        "e edit layout   r reload config   ? help   q quit"
    };
    frame.render_widget(
        Paragraph::new(footer)
            .style(Style::default().fg(MUTED))
            .wrap(Wrap { trim: true }),
        Rect::new(area.x + 1, area.bottom() - 2, area.width - 2, 2),
    );

    if let Some(modal) = &app.modal {
        draw_modal(frame, app, modal);
    }
}

fn draw_modal(frame: &mut Frame, app: &App, modal: &Modal) {
    let (title, lines): (&str, Vec<Line<'_>>) = match modal {
        Modal::Help { scroll } => (
            " Help · ↑/↓ scroll · Esc close ",
            HELP_LINES
                .iter()
                .skip(*scroll)
                .map(|line| Line::from(*line))
                .collect(),
        ),
        Modal::Add { selected } => {
            let mut lines = vec![Line::from("Choose a tile for this profile"), Line::from("")];
            for (i, definition) in app.registry.list().iter().enumerate() {
                lines.push(
                    Line::from(format!(
                        "{} {}",
                        if i == *selected { "›" } else { " " },
                        definition.name
                    ))
                    .style(Style::default().fg(if i == *selected {
                        Color::Yellow
                    } else {
                        Color::White
                    })),
                );
            }
            lines.push(Line::from(""));
            lines.push(Line::from("↑/↓ select · Enter add · Esc close"));
            (" Add tile ", lines)
        }
        Modal::Settings { fields, selected } => {
            let mut lines = vec![];
            for (i, (label, value)) in fields.iter().enumerate() {
                lines.push(
                    Line::from(format!(
                        "{} {label}",
                        if i == *selected { "›" } else { " " }
                    ))
                    .style(Style::default().fg(if i == *selected {
                        Color::Yellow
                    } else {
                        MUTED
                    })),
                );
                lines.push(Line::from(format!(
                    "  {value}{}",
                    if i == *selected { "▏" } else { "" }
                )));
                lines.push(Line::from(""));
            }
            lines.push(Line::from(
                "Accent: cyan magenta green yellow blue red white",
            ));
            lines.push(Line::from(
                "Tab field · Ctrl+u clear · Enter apply · Esc close",
            ));
            (" Tile settings ", lines)
        }
    };
    let area = frame.area();
    let width = area.width.saturating_sub(4).min(68);
    let height = area.height.saturating_sub(6).min(lines.len() as u16 + 2);
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + 2 + (area.height.saturating_sub(6) - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));
    let inner_height = height.saturating_sub(2) as usize;
    let paragraph = match modal {
        Modal::Settings { selected, .. } => {
            let scroll = (selected * 3 + 2).saturating_sub(inner_height);
            // Keep the focused field visible even in a short terminal.
            Paragraph::new(lines).scroll((scroll as u16, 0))
        }
        Modal::Add { selected } => {
            let scroll = (selected + 3).saturating_sub(inner_height);
            Paragraph::new(lines).scroll((scroll as u16, 0))
        }
        Modal::Help { .. } => Paragraph::new(lines).wrap(Wrap { trim: false }),
    };
    frame.render_widget(paragraph.block(block), rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        metrics::{DiskUsage, Metrics},
        tiles::Registry,
    };
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn render_survives_tiny_wide_tall_and_edit_sizes() {
        let mut app = App::new(Config::default(), "unused.toml".into(), Registry::builtin());
        app.update_metrics(Metrics {
            cpu: Some(42.0),
            cores: vec![42.0; 4],
            disks: vec![DiskUsage {
                mount: "/".into(),
                total: 1000,
                available: 400,
            }],
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
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|c| c.symbol())
                    .collect();
                assert!(text.contains("TILEBOARD"));
                app.handle_key(crossterm::event::KeyCode::Char('e').into());
                terminal.draw(|frame| draw(frame, &mut app)).unwrap();
                app.handle_key(crossterm::event::KeyCode::Char('?').into());
                terminal.draw(|frame| draw(frame, &mut app)).unwrap();
                app.handle_key(crossterm::event::KeyCode::Esc.into());
                app.handle_key(crossterm::event::KeyCode::Esc.into());
            }
        }
    }
}
