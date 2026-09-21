use super::{OptionField, Tile, TileDefinition, accent, option};
use crate::{
    config::TileConfig,
    metrics::Metrics,
    theme::{self, bytes},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{LineGauge, Paragraph},
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
                    if metrics.ready {
                        "No mounted disks reported"
                    } else {
                        "Reading storage…"
                    }
                } else {
                    "Mount unavailable"
                }),
                area,
            );
            return;
        }
        let capacity = usize::from(area.height / 2);
        let visible = if disks.len() > capacity && capacity > 1 && area.height.is_multiple_of(2) {
            capacity - 1
        } else {
            capacity
        };
        let has_footer = visible * 2 < usize::from(area.height);
        for (i, disk) in disks.iter().take(visible).enumerate() {
            let y = area.y + (i as u16 * 2);
            let used = disk.total.saturating_sub(disk.available);
            frame.render_widget(
                Paragraph::new(format!(
                    "{}  {} free{}",
                    disk.mount,
                    bytes(disk.available),
                    if !has_footer && i + 1 == visible && disks.len() > visible {
                        format!(" · +{} mounts", disks.len() - visible)
                    } else {
                        String::new()
                    }
                )),
                Rect::new(area.x, y, area.width, 1),
            );
            let ratio = if disk.total == 0 {
                0.0
            } else {
                (used as f64 / disk.total as f64).clamp(0.0, 1.0)
            };
            frame.render_widget(
                LineGauge::default()
                    .ratio(ratio)
                    .label(format!("{:.0}%", ratio * 100.0))
                    .unfilled_style(Style::default().fg(theme::BORDER))
                    .filled_style(
                        Style::default()
                            .fg(accent(&config.accent).unwrap_or(theme::GREEN))
                            .bg(theme::SURFACE),
                    ),
                Rect::new(area.x, y + 1, area.width, 1),
            );
        }
        if has_footer && disks.len() > visible {
            frame.render_widget(
                Paragraph::new(format!(
                    "+{} mounts · enlarge tile to see more",
                    disks.len() - visible
                ))
                .style(Style::default().fg(theme::MUTED)),
                Rect::new(area.x, area.bottom() - 1, area.width, 1),
            );
        }
    }
}
