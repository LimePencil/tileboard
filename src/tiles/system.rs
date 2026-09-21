use super::{Tile, TileDefinition, accent};
use crate::{config::TileConfig, metrics::Metrics, theme};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::Paragraph,
};

struct SystemTile;
pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "system",
        name: "System",
        create: || Box::new(SystemTile),
        fields: &[],
        validate_options: |_| Ok(()),
    }
}

impl Tile for SystemTile {
    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics) {
        let Some(system) = &metrics.system else {
            frame.render_widget(
                Paragraph::new(if metrics.ready {
                    "System unavailable"
                } else {
                    "Reading system…"
                }),
                area,
            );
            return;
        };
        let days = system.uptime / 86400;
        let hours = system.uptime / 3600 % 24;
        let minutes = system.uptime / 60 % 60;
        let uptime = if days > 0 {
            format!("Up {days}d {hours:02}h {minutes:02}m")
        } else {
            format!("Up {hours}h {minutes:02}m")
        };
        let lines = vec![
            Line::from(uptime).style(
                Style::default()
                    .fg(accent(&config.accent).unwrap_or(theme::GREEN))
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(system.hostname.clone()),
            Line::from(system.os.clone()).style(Style::default().fg(theme::MUTED)),
            Line::from(format!("{} logical CPUs", metrics.cores.len()))
                .style(Style::default().fg(theme::MUTED)),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }
}
