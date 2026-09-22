use super::{OptionField, Tile, TileDefinition, option};
use crate::{config::TileConfig, metrics::Metrics, theme};
use chrono::{Datelike, Duration};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
struct Calendar;
pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "calendar",
        name: "Calendar",
        default_refresh_ms: 60000,
        sources: &[],
        create: || Box::new(Calendar),
        fields: &[OptionField {
            key: "week_start",
            label: "Week starts (monday / sunday)",
            default: "monday",
        }],
        validate_options: |options| {
            anyhow::ensure!(
                matches!(
                    crate::integrations::setting(options, "week_start", "monday"),
                    "monday" | "sunday"
                ),
                "Week start must be monday or sunday"
            );
            Ok(())
        },
    }
}
impl Tile for Calendar {
    fn minimum_size(&self) -> (u16, u16) {
        (25, 10)
    }
    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics) {
        let today = metrics.now.date_naive();
        let first = today.with_day(1).unwrap();
        let sunday = option(config, "week_start", "monday") == "sunday";
        let offset = if sunday {
            first.weekday().num_days_from_sunday()
        } else {
            first.weekday().num_days_from_monday()
        };
        let mut lines = vec![
            Line::from(today.format("%B %Y").to_string()).style(
                Style::default()
                    .fg(theme::CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(if sunday {
                "Su Mo Tu We Th Fr Sa"
            } else {
                "Mo Tu We Th Fr Sa Su"
            })
            .style(Style::default().fg(theme::MUTED)),
        ];
        for week in 0..6 {
            let mut spans = vec![];
            for day in 0..7 {
                let date = first + Duration::days(i64::from(week * 7 + day) - i64::from(offset));
                let label = if date.month() == today.month() {
                    format!("{:2}", date.day())
                } else {
                    "  ".into()
                };
                spans.push(Span::styled(
                    label,
                    if date == today {
                        Style::default()
                            .fg(theme::BACKGROUND)
                            .bg(theme::CYAN)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme::TEXT)
                    },
                ));
                if day != 6 {
                    spans.push(Span::raw(" "));
                }
            }
            lines.push(Line::from(spans));
        }
        frame.render_widget(Paragraph::new(lines), area);
    }
}
