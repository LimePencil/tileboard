use super::{OptionField, Tile, TileDefinition, option};
use crate::{
    config::TileConfig,
    metrics::{Metrics, Source},
    theme,
};
use ratatui::{Frame, layout::Rect, style::Style, text::Line, widgets::Paragraph};
struct Temperatures;
pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "temperature",
        name: "Temperatures",
        default_refresh_ms: 5000,
        sources: &[Source::Temperature],
        create: || Box::new(Temperatures),
        fields: &[
            OptionField {
                key: "filter",
                label: "Sensor name contains",
                default: "",
            },
            OptionField {
                key: "unit",
                label: "Unit (celsius / fahrenheit)",
                default: "celsius",
            },
        ],
        validate_options: super::external::validate_unit,
    }
}
impl Tile for Temperatures {
    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics) {
        if metrics.temperatures.is_empty() {
            frame.render_widget(
                Paragraph::new(if metrics.ready {
                    "No temperature sensors available"
                } else {
                    "Reading sensors…"
                }),
                area,
            );
            return;
        }
        let filter = option(config, "filter", "").to_lowercase();
        let mut sensors: Vec<_> = metrics
            .temperatures
            .iter()
            .filter(|s| s.label.to_lowercase().contains(&filter))
            .collect();
        sensors.sort_by(|a, b| b.celsius.total_cmp(&a.celsius).then(a.label.cmp(&b.label)));
        let fahrenheit = option(config, "unit", "celsius") == "fahrenheit";
        let mut lines: Vec<Line> = sensors
            .into_iter()
            .take(area.height as usize)
            .map(|s| {
                let value = if fahrenheit {
                    s.celsius * 1.8 + 32.0
                } else {
                    s.celsius
                };
                Line::from(format!(
                    "{value:.1}°{}  {}",
                    if fahrenheit { "F" } else { "C" },
                    super::plain_text(&s.label)
                ))
                .style(Style::default().fg(
                    if s.critical.is_some_and(|v| s.celsius >= v) {
                        theme::RED
                    } else {
                        theme::TEXT
                    },
                ))
            })
            .collect();
        if lines.is_empty() {
            lines.push(Line::from("No matching sensors"));
        }
        frame.render_widget(Paragraph::new(lines), area);
    }
}
