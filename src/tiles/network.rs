use super::{OptionField, Tile, TileDefinition, accent, option};
use crate::{
    config::TileConfig,
    metrics::{Metrics, NetworkUsage},
    theme::{self, bytes},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

struct Network;
pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "network",
        name: "Network",
        create: || Box::new(Network),
        fields: &[OptionField {
            key: "interface",
            label: "Interface (empty = busiest)",
            default: "",
        }],
        validate_options: |_| Ok(()),
    }
}

pub(super) fn selected_interface<'a>(metrics: &'a Metrics, name: &str) -> Option<&'a NetworkUsage> {
    if !name.is_empty() {
        return metrics.networks.iter().find(|n| n.name == name);
    }
    let active: Vec<_> = metrics
        .networks
        .iter()
        .filter(|n| n.name != "lo" && n.name != "lo0")
        .collect();
    active.into_iter().max_by(|a, b| {
        let activity = |n: &NetworkUsage| n.rates.map(|(rx, tx)| rx + tx).unwrap_or(0.0);
        activity(a)
            .total_cmp(&activity(b))
            .then_with(|| b.name.cmp(&a.name))
    })
}

impl Tile for Network {
    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics) {
        let requested = option(config, "interface", "");
        let Some(network) = selected_interface(metrics, requested) else {
            frame.render_widget(
                Paragraph::new(if !metrics.ready {
                    "Sampling network…"
                } else if requested.is_empty() {
                    "No network interfaces"
                } else {
                    "Interface unavailable"
                }),
                area,
            );
            return;
        };
        let Some((rx, tx)) = network.rates else {
            frame.render_widget(
                Paragraph::new(format!("{}\nMeasuring throughput…", network.name)),
                area,
            );
            return;
        };
        let color = accent(&config.accent).unwrap_or(theme::CYAN);
        let rate = |symbol, value: f64, label| {
            Line::from(vec![
                Span::styled(
                    format!("{symbol} {}/s", bytes(value.max(0.0) as u64)),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {label}"), Style::default().fg(theme::MUTED)),
            ])
        };
        let mut lines = vec![rate("↓", rx, "receive"), rate("↑", tx, "send")];
        if area.height >= 3 {
            lines.push(Line::from(network.name.clone()).style(Style::default().fg(theme::MUTED)));
        }
        if area.height >= 5 {
            lines.push(
                Line::from(if requested.is_empty() {
                    "Auto · busiest interface"
                } else {
                    "Selected interface"
                })
                .style(Style::default().fg(theme::MUTED)),
            );
        }
        frame.render_widget(Paragraph::new(lines), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_avoids_loopback_and_explicit_missing_interface_is_not_substituted() {
        let metrics = Metrics {
            networks: vec![
                NetworkUsage {
                    name: "lo".into(),
                    rates: Some((9999.0, 9999.0)),
                },
                NetworkUsage {
                    name: "eth0".into(),
                    rates: Some((10.0, 20.0)),
                },
                NetworkUsage {
                    name: "wlan0".into(),
                    rates: Some((100.0, 200.0)),
                },
            ],
            ..Metrics::default()
        };
        assert_eq!(selected_interface(&metrics, "").unwrap().name, "wlan0");
        assert_eq!(selected_interface(&metrics, "eth0").unwrap().name, "eth0");
        assert!(selected_interface(&metrics, "missing").is_none());
    }
}
