use super::{Tile, TileDefinition};
use crate::{
    config::TileConfig,
    metrics::{Metrics, Source},
    theme,
};
use ratatui::{Frame, layout::Rect, style::Style, text::Line, widgets::Paragraph};
struct Battery;
pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "battery",
        name: "Battery",
        default_refresh_ms: 30000,
        sources: &[Source::Battery],
        create: || Box::new(Battery),
        fields: &[],
        validate_options: |_| Ok(()),
    }
}
impl Tile for Battery {
    fn render(&self, frame: &mut Frame, area: Rect, _: &TileConfig, metrics: &Metrics) {
        let mut lines = vec![];
        match &metrics.batteries {
            None => lines.push(Line::from("Reading battery…")),
            Some(Err(error)) => {
                lines.push(Line::from(error.clone()).style(Style::default().fg(theme::RED)))
            }
            Some(Ok(batteries)) if batteries.is_empty() => {
                lines.push(Line::from("No battery detected"))
            }
            Some(Ok(batteries)) => {
                for (index, battery) in batteries.iter().enumerate() {
                    lines.push(
                        Line::from(format!(
                            "Battery {}  {:.0}% · {}",
                            index + 1,
                            battery.percent,
                            battery.state
                        ))
                        .style(Style::default().fg(
                            if battery.percent < 20.0 {
                                theme::YELLOW
                            } else {
                                theme::GREEN
                            },
                        )),
                    );
                    lines.push(
                        Line::from(format!(
                            "Health {:.0}%{}",
                            battery.health,
                            battery
                                .remaining_minutes
                                .map(|v| format!(" · {}h {:02}m remaining", v / 60, v % 60))
                                .unwrap_or_default()
                        ))
                        .style(Style::default().fg(theme::MUTED)),
                    );
                }
            }
        }
        frame.render_widget(Paragraph::new(lines), area);
    }
}
