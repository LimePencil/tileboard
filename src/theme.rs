//! A restrained, shared palette for dashboard chrome and built-in tiles.
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Slate,
    Amber,
    Mono,
}

impl Theme {
    pub fn next(self) -> Self {
        match self {
            Self::Slate => Self::Amber,
            Self::Amber => Self::Mono,
            Self::Mono => Self::Slate,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Slate => "slate",
            Self::Amber => "amber",
            Self::Mono => "mono",
        }
    }
    pub fn apply(self, buffer: &mut ratatui::buffer::Buffer) {
        for cell in &mut buffer.content {
            cell.fg = self.color(cell.fg);
            cell.bg = self.color(cell.bg);
        }
    }
    fn color(self, color: Color) -> Color {
        use Color::Rgb;
        if self == Self::Slate {
            return color;
        }
        if self == Self::Mono {
            return match color {
                BACKGROUND => Rgb(16, 16, 16),
                SURFACE => Rgb(25, 25, 25),
                BORDER => Rgb(65, 65, 65),
                PREVIEW_OK | PREVIEW_BAD => Rgb(50, 50, 50),
                TEXT => Rgb(228, 228, 228),
                MUTED => Rgb(156, 156, 156),
                YELLOW | RED => Rgb(255, 255, 255),
                Rgb(_, _, _) => Rgb(208, 208, 208),
                _ => color,
            };
        }
        match color {
            BACKGROUND => Rgb(24, 22, 20),
            SURFACE => Rgb(35, 31, 26),
            BORDER => Rgb(79, 67, 52),
            PREVIEW_OK => Rgb(47, 45, 31),
            PREVIEW_BAD => Rgb(56, 34, 28),
            TEXT => Rgb(237, 226, 209),
            MUTED => Rgb(176, 157, 132),
            CYAN => Rgb(232, 186, 117),
            GREEN => Rgb(184, 201, 148),
            PURPLE => Rgb(211, 178, 164),
            YELLOW => Rgb(255, 211, 139),
            Color::Rgb(147, 177, 255) => Rgb(225, 204, 166),
            _ => color,
        }
    }
}

pub fn interval(milliseconds: u64) -> String {
    if milliseconds < 1000 {
        format!("{milliseconds}ms")
    } else if milliseconds.is_multiple_of(60_000) {
        format!("{}m", milliseconds / 60_000)
    } else {
        format!("{}s", milliseconds as f64 / 1000.0)
    }
}

pub const BACKGROUND: Color = Color::Rgb(16, 21, 31);
pub const PREVIEW_OK: Color = Color::Rgb(24, 43, 42);
pub const PREVIEW_BAD: Color = Color::Rgb(54, 29, 37);
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
