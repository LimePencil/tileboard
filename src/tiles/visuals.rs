use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
};

/// Three-row numerals, rendered only when the whole value fits. No special font required.
pub fn big_value(frame: &mut Frame, area: Rect, text: &str, color: Color, centered: bool) -> bool {
    let mut lines = [String::new(), String::new(), String::new()];
    for (i, c) in text.chars().enumerate() {
        let glyph = match c {
            '0' => ["█▀█", "█ █", "▀▀▀"],
            '1' => [" ▀█", "  █", "  ▀"],
            '2' => ["▀▀█", "█▀▀", "▀▀▀"],
            '3' => ["▀▀█", " ▀█", "▀▀▀"],
            '4' => ["█ █", "▀▀█", "  ▀"],
            '5' => ["█▀▀", "▀▀█", "▀▀▀"],
            '6' => ["█▀▀", "█▀█", "▀▀▀"],
            '7' => ["▀▀█", "  █", "  ▀"],
            '8' => ["█▀█", "█▀█", "▀▀▀"],
            '9' => ["█▀█", "▀▀█", "▀▀▀"],
            '.' => [" ", " ", "▄"],
            ':' => [" ", "▪", "▪"],
            '%' => [" ", "%", " "],
            _ => return false,
        };
        for row in 0..3 {
            if i > 0 {
                lines[row].push(' ');
            }
            lines[row].push_str(glyph[row]);
        }
    }
    let width = lines[0].chars().count() as u16;
    if width > area.width || area.height < 3 {
        return false;
    }
    let x = area.x
        + if centered {
            (area.width - width) / 2
        } else {
            0
        };
    frame.render_widget(
        Paragraph::new(lines.join("\n")).style(Style::default().fg(color)),
        Rect::new(x, area.y, width, 3),
    );
    true
}
