//! Render the real UI with deterministic sample data for visual review.
//! cargo run --example preview -- docs/previews
use chrono::{Local, TimeZone};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use std::{fmt::Write, fs, path::PathBuf};
use tileboard::{
    app::App,
    config::Config,
    metrics::{DiskUsage, MemoryUsage, Metrics, NetworkUsage, SystemInfo},
    theme,
    tiles::Registry,
    ui,
};
use unicode_width::UnicodeWidthStr;

fn main() -> anyhow::Result<()> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/tileboard-previews".into()),
    );
    fs::create_dir_all(&directory)?;
    for (name, width, height, edit) in [
        ("wide", 120, 30, false),
        ("compact", 80, 24, false),
        ("small", 42, 28, false),
        ("settings", 38, 16, true),
        ("amber", 120, 30, false),
        ("mono", 120, 30, false),
        ("detail", 120, 44, false),
        ("editor", 120, 30, true),
    ] {
        let mut app = App::new(
            Config::default(),
            "preview.toml".into(),
            Registry::builtin(),
        );
        app.config.theme = match name {
            "amber" => theme::Theme::Amber,
            "mono" => theme::Theme::Mono,
            _ => theme::Theme::Slate,
        };
        let metrics = Metrics {
            ready: true,
            cpu: Some(24.5),
            cores: vec![24.5; 8],
            now: Local.with_ymd_and_hms(2026, 9, 22, 14, 32, 9).unwrap(),
            memory: Some(MemoryUsage {
                total: 16 << 30,
                available: 10 << 30,
                swap_total: 2 << 30,
                swap_used: 0,
            }),
            networks: vec![NetworkUsage {
                received: 0,
                transmitted: 0,
                name: "en0".into(),
                rates: Some((1258291.0, 86016.0)),
            }],
            disks: vec![DiskUsage {
                mount: "/".into(),
                total: 512 << 30,
                available: 182 << 30,
            }],
            system: Some(SystemInfo {
                logical_cpus: 8,
                hostname: "workstation".into(),
                os: "Linux".into(),
                uptime: 187320,
            }),
            ..Metrics::default()
        };
        for i in 0..100 {
            let mut sample = metrics.clone();
            sample.cpu = Some(18.0 + (i as f32 * 0.5).sin().abs() * 30.0);
            sample.memory.as_mut().unwrap().available =
                (9 << 30) + ((i as f64 * 0.15).sin().abs() * (2_u64 << 30) as f64) as u64;
            sample.networks[0].rates = Some((
                500000.0 + (i as f64 * 0.3).sin().abs() * 2000000.0,
                30000.0 + (i as f64 * 0.17).cos().abs() * 700000.0,
            ));
            app.update_metrics(sample);
        }
        app.update_metrics(metrics);
        app.resize(ratatui::layout::Rect::new(0, 0, width, height));
        if edit {
            app.handle_key(KeyCode::Char('e').into());
        }
        if name == "editor" {
            app.handle_key(KeyCode::Char('h').into());
        } else if edit {
            app.handle_key(KeyCode::Char('t').into());
            app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
            app.paste("A long title with a visible ending");
        }
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        let cursor = (name == "settings")
            .then(|| terminal.get_cursor_position())
            .transpose()?;
        let buffer = terminal.backend().buffer();
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
            width * 9 + 24,
            height * 18 + 24,
            width * 9 + 24,
            height * 18 + 24
        );
        writeln!(
            svg,
            "<rect width=\"100%\" height=\"100%\" rx=\"10\" fill=\"{}\"/>",
            color(buffer[(0, 0)].bg)
        )?;
        for y in 0..height {
            let mut x = 0;
            while x < width {
                let start = x;
                let cell = &buffer[(x, y)];
                let mut text = String::new();
                while x < width && buffer[(x, y)].style() == cell.style() {
                    let symbol = buffer[(x, y)].symbol();
                    text.push_str(symbol);
                    x += symbol.width().max(1) as u16;
                }
                let px = 12 + start * 9;
                let py = 12 + y * 18;
                let length = (x - start) * 9;
                writeln!(
                    svg,
                    "<rect x=\"{px}\" y=\"{py}\" width=\"{length}\" height=\"18\" fill=\"{}\"/>",
                    color(cell.bg)
                )?;
                if !text.trim().is_empty() {
                    writeln!(
                        svg,
                        "<text x=\"{px}\" y=\"{}\" fill=\"{}\" font-family=\"DejaVu Sans Mono, monospace\" font-size=\"15\" font-weight=\"{}\" xml:space=\"preserve\" textLength=\"{length}\" lengthAdjust=\"spacingAndGlyphs\">{}</text>",
                        py + 14,
                        color(cell.fg),
                        if cell.modifier.contains(Modifier::BOLD) {
                            "bold"
                        } else {
                            "normal"
                        },
                        escape(&text)
                    )?;
                }
            }
        }
        if let Some(cursor) = cursor {
            writeln!(
                svg,
                "<rect x=\"{}\" y=\"{}\" width=\"2\" height=\"16\" fill=\"{}\"/>",
                12 + cursor.x * 9,
                12 + cursor.y * 18,
                color(theme::TEXT)
            )?;
        }
        svg.push_str("</svg>\n");
        fs::write(directory.join(format!("{name}.svg")), svg)?;
    }
    println!("Previews written to {}", directory.display());
    Ok(())
}

fn color(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Reset => "#10151f".into(),
        _ => "#d6dfeb".into(),
    }
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
