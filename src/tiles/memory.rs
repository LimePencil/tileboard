use super::{Tile, TileDefinition, accent, visuals::big_value};
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
    widgets::{LineGauge, Paragraph, Sparkline},
};
use std::collections::VecDeque;

#[derive(Default)]
struct Memory {
    history: VecDeque<u64>,
}
pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "memory",
        default_refresh_ms: 2000,
        sources: &[crate::metrics::Source::Memory],
        name: "Memory",
        create: || Box::<Memory>::default(),
        fields: &[],
        validate_options: |_| Ok(()),
    }
}
impl Tile for Memory {
    fn update(&mut self, _config: &TileConfig, metrics: &Metrics) {
        if let Some(memory) = &metrics.memory {
            let ratio = if memory.total == 0 {
                0.0
            } else {
                memory.total.saturating_sub(memory.available) as f64 / memory.total as f64
            };
            self.history.push_back((ratio * 100.0) as u64);
            if self.history.len() > 120 {
                self.history.pop_front();
            }
        }
    }
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
        let large = area.height >= 9
            && area.width >= 24
            && big_value(frame, area, &format!("{:.0}%", ratio * 100.0), color, false);
        let offset = if large { 3 } else { 0 };
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
            Rect::new(area.x, area.y + offset, area.width, 1),
        );
        let y = area.y + offset + 1;
        if y < area.bottom() {
            frame.render_widget(
                LineGauge::default()
                    .ratio(ratio)
                    .label("")
                    .filled_style(Style::default().fg(color))
                    .unfilled_style(Style::default().fg(theme::BORDER)),
                Rect::new(area.x, y, area.width, 1),
            );
        }
        if y + 1 < area.bottom() {
            frame.render_widget(
                Paragraph::new(format!("{} available", bytes(memory.available)))
                    .style(Style::default().fg(theme::MUTED)),
                Rect::new(area.x, y + 1, area.width, 1),
            );
        }
        if y + 2 < area.bottom() {
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
                Rect::new(area.x, y + 2, area.width, 1),
            );
        }
        let remaining = area.bottom().saturating_sub(y + 3);
        if remaining >= 2 {
            let data: Vec<_> = self
                .history
                .iter()
                .rev()
                .take(usize::from(area.width))
                .rev()
                .copied()
                .collect();
            frame.render_widget(
                Sparkline::default()
                    .data(&data)
                    .max(100)
                    .style(Style::default().fg(color)),
                Rect::new(area.x, y + 3, area.width, remaining - 1),
            );
            frame.render_widget(
                Paragraph::new("RAM history").style(Style::default().fg(theme::MUTED)),
                Rect::new(area.x, area.bottom() - 1, area.width, 1),
            );
        }
    }
}
