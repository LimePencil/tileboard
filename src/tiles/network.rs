use super::{OptionField, Tile, TileDefinition, accent, option, visuals::big_value};
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
    widgets::{Paragraph, Sparkline},
};

#[derive(Default)]
struct Network {
    receive: std::collections::VecDeque<u64>,
    send: std::collections::VecDeque<u64>,
    interface: String,
}
pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "network",
        default_refresh_ms: 1000,
        sources: &[crate::metrics::Source::Network],
        name: "Network",
        create: || Box::<Network>::default(),
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
    fn update(&mut self, config: &TileConfig, metrics: &Metrics) {
        let Some(network) = selected_interface(metrics, option(config, "interface", "")) else {
            self.receive.clear();
            self.send.clear();
            self.interface.clear();
            return;
        };
        if self.interface != network.name {
            self.receive.clear();
            self.send.clear();
            self.interface = network.name.clone();
        }
        if let Some((rx, tx)) = network.rates {
            self.receive.push_back(rx.max(0.0) as u64);
            self.send.push_back(tx.max(0.0) as u64);
            if self.receive.len() > 120 {
                self.receive.pop_front();
                self.send.pop_front();
            }
        }
    }
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
        let mut area = area;
        let received = bytes(rx.max(0.0) as u64);
        if area.height >= 13
            && area.width >= 30
            && big_value(
                frame,
                area,
                received.split_whitespace().next().unwrap_or("0"),
                color,
                false,
            )
        {
            area.y += 3;
            area.height -= 3;
        }
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
        frame.render_widget(Paragraph::new(lines), area);
        if area.height >= 8 {
            let graph_height = (area.height - 6) / 2;
            let max = self
                .receive
                .iter()
                .chain(self.send.iter())
                .copied()
                .max()
                .unwrap_or(1)
                .max(1);
            for (index, (history, label, color)) in [
                (&self.receive, "↓ receive", color),
                (&self.send, "↑ send", theme::PURPLE),
            ]
            .into_iter()
            .enumerate()
            {
                let y = area.y + 3 + index as u16 * (graph_height + 1);
                frame.render_widget(
                    Paragraph::new(label).style(Style::default().fg(theme::MUTED)),
                    Rect::new(area.x, y, area.width, 1),
                );
                let data: Vec<_> = history
                    .iter()
                    .rev()
                    .take(usize::from(area.width))
                    .rev()
                    .copied()
                    .collect();
                frame.render_widget(
                    Sparkline::default()
                        .data(&data)
                        .max(max)
                        .style(Style::default().fg(color)),
                    Rect::new(area.x, y + 1, area.width, graph_height),
                );
            }
            frame.render_widget(
                Paragraph::new("Throughput history").style(Style::default().fg(theme::MUTED)),
                Rect::new(area.x, area.bottom() - 1, area.width, 1),
            );
        }
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
                    received: 0,
                    transmitted: 0,
                    name: "lo".into(),
                    rates: Some((9999.0, 9999.0)),
                },
                NetworkUsage {
                    received: 0,
                    transmitted: 0,
                    name: "eth0".into(),
                    rates: Some((10.0, 20.0)),
                },
                NetworkUsage {
                    received: 0,
                    transmitted: 0,
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
