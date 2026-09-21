use super::{OptionField, Tile, TileDefinition, accent, option};
use crate::{config::TileConfig, metrics::Metrics};
use anyhow::ensure;
use chrono::format::{Item, StrftimeItems};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::Paragraph,
};

struct Clock;

pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "clock",
        name: "Local time",
        create: || Box::new(Clock),
        fields: &[OptionField {
            key: "format",
            label: "Time format",
            default: "%H:%M:%S",
        }],
        validate_options: |options| {
            let format = options
                .get("format")
                .and_then(toml::Value::as_str)
                .unwrap_or("%H:%M:%S");
            ensure!(
                !StrftimeItems::new(format).any(|item| matches!(item, Item::Error)),
                "Invalid clock format; try %H:%M:%S or %I:%M:%S %p"
            );
            Ok(())
        },
    }
}

impl Tile for Clock {
    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics) {
        let lines = vec![
            Line::from(
                metrics
                    .now
                    .format(option(config, "format", "%H:%M:%S"))
                    .to_string(),
            )
            .style(
                Style::default()
                    .fg(accent(&config.accent).unwrap_or(Color::Magenta))
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(metrics.now.format("%a, %d %b %Y").to_string()),
            Line::from(metrics.now.format("Local · UTC%:z").to_string())
                .style(Style::default().fg(Color::DarkGray)),
        ];
        let y = area.y + area.height.saturating_sub(3) / 2;
        frame.render_widget(
            Paragraph::new(lines).alignment(Alignment::Center),
            Rect::new(area.x, y, area.width, area.bottom() - y),
        );
    }
}
