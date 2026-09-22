use super::{OptionField, Tile, TileDefinition, accent, option};
use crate::{
    config::TileConfig,
    metrics::{Metrics, Source},
    theme,
};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Style,
    widgets::{Paragraph, Row, Table},
};

struct Processes;
pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "processes",
        name: "Top processes",
        default_refresh_ms: 2000,
        sources: &[Source::Processes],
        create: || Box::new(Processes),
        fields: &[
            OptionField {
                key: "sort",
                label: "Sort (cpu / memory)",
                default: "cpu",
            },
            OptionField {
                key: "filter",
                label: "Process name contains",
                default: "",
            },
        ],
        validate_options: |options| {
            anyhow::ensure!(
                matches!(
                    crate::integrations::setting(options, "sort", "cpu"),
                    "cpu" | "memory"
                ),
                "Sort must be cpu or memory"
            );
            Ok(())
        },
    }
}
impl Tile for Processes {
    fn minimum_size(&self) -> (u16, u16) {
        (28, 5)
    }
    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics) {
        if !metrics.ready {
            frame.render_widget(Paragraph::new("Reading processes…"), area);
            return;
        }
        let filter = option(config, "filter", "").to_lowercase();
        let mut processes: Vec<_> = metrics
            .processes
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&filter))
            .collect();
        if option(config, "sort", "cpu") == "memory" {
            processes.sort_by(|a, b| b.memory.cmp(&a.memory).then(a.pid.cmp(&b.pid)));
        } else {
            processes.sort_by(|a, b| {
                b.cpu
                    .unwrap_or(0.0)
                    .total_cmp(&a.cpu.unwrap_or(0.0))
                    .then(a.pid.cmp(&b.pid))
            });
        }
        if processes.is_empty() {
            frame.render_widget(Paragraph::new("No matching processes"), area);
            return;
        }
        let rows = processes
            .into_iter()
            .take(area.height.saturating_sub(1) as usize)
            .map(|p| {
                Row::new(vec![
                    super::plain_text(&p.name),
                    p.cpu
                        .map(|v| format!("{v:.0}%"))
                        .unwrap_or_else(|| "…".into()),
                    theme::bytes(p.memory),
                ])
            });
        let table = Table::new(
            rows,
            [
                Constraint::Min(8),
                Constraint::Length(6),
                Constraint::Length(9),
            ],
        )
        .column_spacing(1)
        .header(
            Row::new(["PROCESS", "CPU¹", "RAM"])
                .style(Style::default().fg(accent(&config.accent).unwrap_or(theme::CYAN))),
        );
        frame.render_widget(table, area);
    }
}
