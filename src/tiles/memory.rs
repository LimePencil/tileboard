use super::{Tile, TileDefinition, accent};
use crate::{
    config::TileConfig,
    metrics::Metrics,
    theme::{self, bytes},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{LineGauge, Paragraph},
};

struct Memory;
pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "memory",
        name: "Memory",
        create: || Box::new(Memory),
        fields: &[],
        validate_options: |_| Ok(()),
    }
}

impl Tile for Memory {
    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics) {
        let Some(memory) = &metrics.memory else {
            frame.render_widget(
                Paragraph::new(if metrics.ready {
                    "Memory unavailable"
                } else {
                    "Sampling memory…"
                }),
                area,
            );
            return;
        };
        let used = memory.total.saturating_sub(memory.available);
        let ratio = if memory.total == 0 {
            0.0
        } else {
            (used as f64 / memory.total as f64).clamp(0.0, 1.0)
        };
        let color = accent(&config.accent).unwrap_or(theme::PURPLE);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{:.0}%", ratio * 100.0),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {} / {}", bytes(used), bytes(memory.total)),
                    Style::default().fg(theme::MUTED),
                ),
            ])),
            Rect::new(area.x, area.y, area.width, 1),
        );
        if area.height >= 2 {
            frame.render_widget(
                LineGauge::default()
                    .ratio(ratio)
                    .label("")
                    .filled_style(Style::default().fg(color))
                    .unfilled_style(Style::default().fg(theme::BORDER)),
                Rect::new(area.x, area.y + 1, area.width, 1),
            );
        }
        if area.height >= 3 {
            frame.render_widget(
                Paragraph::new(format!("{} available", bytes(memory.available)))
                    .style(Style::default().fg(theme::MUTED)),
                Rect::new(area.x, area.y + 2, area.width, 1),
            );
        }
        if area.height >= 4 {
            let swap = if memory.swap_total == 0 {
                "Swap disabled".into()
            } else {
                format!(
                    "Swap  {} / {}",
                    bytes(memory.swap_used),
                    bytes(memory.swap_total)
                )
            };
            frame.render_widget(
                Paragraph::new(swap).style(Style::default().fg(theme::MUTED)),
                Rect::new(area.x, area.y + 3, area.width, 1),
            );
        }
    }
}
