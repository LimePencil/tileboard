//! A restrained, shared palette for dashboard chrome and built-in tiles.
use ratatui::style::{Color, Style};

pub const BACKGROUND: Color = Color::Rgb(16, 21, 31);
pub const SURFACE: Color = Color::Rgb(22, 30, 42);
pub const TEXT: Color = Color::Rgb(214, 223, 235);
pub const MUTED: Color = Color::Rgb(140, 155, 174);
pub const BORDER: Color = Color::Rgb(51, 66, 86);
pub const CYAN: Color = Color::Rgb(125, 211, 252);
pub const GREEN: Color = Color::Rgb(141, 225, 187);
pub const YELLOW: Color = Color::Rgb(247, 206, 130);
pub const RED: Color = Color::Rgb(242, 139, 156);
pub const PURPLE: Color = Color::Rgb(196, 181, 253);

pub fn surface() -> Style {
    Style::default().fg(TEXT).bg(SURFACE)
}

pub fn bytes(value: u64) -> String {
    let mut value = value as f64;
    for unit in ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] {
        if value < 1024.0 || unit == "EiB" {
            return format!("{value:.1} {unit}");
        }
        value /= 1024.0;
    }
    unreachable!()
}
