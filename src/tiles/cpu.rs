use super::{Tile, TileDefinition, accent};
use crate::{config::TileConfig, metrics::Metrics};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Gauge, Paragraph, Sparkline},
};
use std::collections::VecDeque;

#[derive(Default)]
struct Cpu {
    history: VecDeque<u64>,
}

pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "cpu",
        name: "CPU usage",
        create: || Box::<Cpu>::default(),
        fields: &[],
        validate_options: |_| Ok(()),
    }
}

impl Tile for Cpu {
    fn update(&mut self, _config: &TileConfig, metrics: &Metrics) {
        if let Some(cpu) = metrics.cpu {
            self.history.push_back(cpu.clamp(0.0, 100.0) as u64);
            if self.history.len() > 120 {
                self.history.pop_front();
            }
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics) {
        let Some(cpu) = metrics.cpu else {
            frame.render_widget(Paragraph::new("Sampling CPU…"), area);
            return;
        };
        let color = accent(&config.accent).unwrap_or(Color::Cyan);
        frame.render_widget(
            Gauge::default()
                .ratio(f64::from(cpu.clamp(0.0, 100.0)) / 100.0)
                .label(format!("{cpu:.1}% · {} cores", metrics.cores.len()))
                .gauge_style(Style::default().fg(color).bg(Color::DarkGray)),
            Rect::new(area.x, area.y, area.width, 1),
        );
        if area.height >= 3 {
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
                Rect::new(
                    area.x,
                    area.y + 2,
                    area.width,
                    area.height.saturating_sub(3).max(1),
                ),
            );
        }
        if area.height >= 5 {
            frame.render_widget(
                Paragraph::new(
                    Line::from("Recent CPU load · 1s samples")
                        .style(Style::default().fg(Color::DarkGray)),
                ),
                Rect::new(area.x, area.bottom() - 1, area.width, 1),
            );
        }
    }
}
