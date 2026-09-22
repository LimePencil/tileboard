use super::{Tile, TileDefinition, accent, visuals::big_value};
use crate::{config::TileConfig, metrics::Metrics, theme};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{LineGauge, Paragraph, Sparkline},
};
use std::collections::VecDeque;

#[derive(Default)]
struct Cpu {
    history: VecDeque<u64>,
}

pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "cpu",
        default_refresh_ms: 1000,
        sources: &[crate::metrics::Source::Cpu],
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
            frame.render_widget(
                Paragraph::new(if metrics.ready {
                    "CPU unavailable"
                } else {
                    "Sampling CPU…"
                }),
                area,
            );
            return;
        };
        let color = accent(&config.accent).unwrap_or(theme::CYAN);
        let large = area.height >= 8
            && area.width >= 24
            && big_value(frame, area, &format!("{cpu:.1}%"), color, false);
        let offset = if large { 3 } else { 0 };
        let line = Line::from(vec![
            Span::styled(
                format!("{cpu:.1}%"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {} logical CPUs", metrics.cores.len()),
                Style::default().fg(theme::MUTED),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(line),
            Rect::new(area.x, area.y + offset, area.width, 1),
        );
        let y = area.y + offset + 1;
        if y < area.bottom() {
            frame.render_widget(
                LineGauge::default()
                    .ratio(f64::from(cpu.clamp(0.0, 100.0)) / 100.0)
                    .label("")
                    .filled_style(Style::default().fg(color))
                    .unfilled_style(Style::default().fg(theme::BORDER)),
                Rect::new(area.x, y, area.width, 1),
            );
        }
        let remaining = area.bottom().saturating_sub(y + 1);
        if remaining == 0 {
            return;
        }
        let footer = u16::from(remaining >= 3);
        let core_rows = if remaining >= 6 && area.width >= 30 {
            (metrics.cores.len().div_ceil(2) as u16).min(3)
        } else {
            0
        };
        let graph_height = remaining.saturating_sub(footer + core_rows);
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
            Rect::new(area.x, y + 1, area.width, graph_height),
        );
        for row in 0..core_rows {
            for column in 0..2 {
                let index = (row * 2 + column) as usize;
                if let Some(load) = metrics.cores.get(index) {
                    let width = area.width / 2;
                    frame.render_widget(
                        LineGauge::default()
                            .ratio(f64::from(load.clamp(0.0, 100.0)) / 100.0)
                            .label(format!("C{} {:>3.0}%", index + 1, load))
                            .filled_style(Style::default().fg(color))
                            .unfilled_style(Style::default().fg(theme::BORDER)),
                        Rect::new(
                            area.x + column * width,
                            y + 1 + graph_height + row,
                            width.saturating_sub(1),
                            1,
                        ),
                    );
                }
            }
        }
        if footer > 0 {
            frame.render_widget(
                Paragraph::new("CPU history").style(Style::default().fg(theme::MUTED)),
                Rect::new(area.x, area.bottom() - 1, area.width, 1),
            );
        }
    }
}
