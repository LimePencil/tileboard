use super::{OptionField, Tile, TileDefinition, accent, option};
use crate::{config::TileConfig, metrics::Metrics};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Gauge, Paragraph},
};

struct Storage;

pub fn definition() -> TileDefinition {
    TileDefinition {
        kind: "storage",
        name: "Storage",
        create: || Box::new(Storage),
        fields: &[OptionField {
            key: "mount",
            label: "Mount (empty = all)",
            default: "",
        }],
        validate_options: |_| Ok(()),
    }
}

impl Tile for Storage {
    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics) {
        let mount = option(config, "mount", "");
        let disks: Vec<_> = metrics
            .disks
            .iter()
            .filter(|d| mount.is_empty() || d.mount == mount)
            .collect();
        if disks.is_empty() {
            frame.render_widget(
                Paragraph::new(if mount.is_empty() {
                    "Waiting for storage data…"
                } else {
                    "Mount unavailable"
                }),
                area,
            );
            return;
        }
        let capacity = usize::from(area.height / 2);
        let visible = if disks.len() > capacity {
            usize::from(area.height.saturating_sub(1) / 2)
        } else {
            capacity
        };
        for (i, disk) in disks.iter().take(visible).enumerate() {
            let y = area.y + (i as u16 * 2);
            let used = disk.total.saturating_sub(disk.available);
            frame.render_widget(
                Paragraph::new(format!(
                    "{}   {} free / {}",
                    disk.mount,
                    bytes(disk.available),
                    bytes(disk.total)
                )),
                Rect::new(area.x, y, area.width, 1),
            );
            let ratio = if disk.total == 0 {
                0.0
            } else {
                (used as f64 / disk.total as f64).clamp(0.0, 1.0)
            };
            frame.render_widget(
                Gauge::default()
                    .ratio(ratio)
                    .label(format!("{:.0}% used", ratio * 100.0))
                    .gauge_style(
                        Style::default()
                            .fg(accent(&config.accent).unwrap_or(Color::Green))
                            .bg(Color::DarkGray),
                    ),
                Rect::new(area.x, y + 1, area.width, 1),
            );
        }
        if disks.len() > visible {
            frame.render_widget(
                Paragraph::new(format!(
                    "+{} mounts · enlarge tile to see more",
                    disks.len() - visible
                ))
                .style(Style::default().fg(Color::DarkGray)),
                Rect::new(area.x, area.bottom() - 1, area.width, 1),
            );
        }
    }
}

fn bytes(value: u64) -> String {
    let mut value = value as f64;
    for unit in ["B", "KiB", "MiB", "GiB", "TiB", "PiB"] {
        if value < 1024.0 || unit == "PiB" {
            return format!("{value:.1} {unit}");
        }
        value /= 1024.0;
    }
    unreachable!()
}
