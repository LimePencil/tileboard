use super::{OptionField, Tile, TileDefinition};
use crate::{
    config::TileConfig,
    integrations::{ExternalData, setting},
    metrics::{Metrics, Source},
    theme,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Paragraph, Wrap},
};
struct ExternalTile;
const fn field(key: &'static str, label: &'static str, default: &'static str) -> OptionField {
    OptionField {
        key,
        label,
        default,
    }
}
pub fn definitions() -> [TileDefinition; 4] {
    [
        TileDefinition {
            kind: "git",
            name: "Git status",
            default_refresh_ms: 5000,
            sources: &[Source::Git],
            create: || Box::new(ExternalTile),
            fields: const { &[field("path", "Repository path", "")] },
            validate_options: |_| Ok(()),
        },
        TileDefinition {
            kind: "service",
            name: "Service health",
            default_refresh_ms: 30000,
            sources: &[Source::Service],
            create: || Box::new(ExternalTile),
            fields: const { &[field("url", "Service URL (HTTP HEAD)", "")] },
            validate_options: |options| {
                let url = setting(options, "url", "");
                if !url.is_empty() {
                    crate::integrations::http_url(url)?;
                }
                Ok(())
            },
        },
        TileDefinition {
            kind: "weather",
            name: "Weather",
            default_refresh_ms: 600000,
            sources: &[Source::Weather],
            create: || Box::new(ExternalTile),
            fields: const {
                &[
                    field("latitude", "Latitude (-90 to 90)", ""),
                    field("longitude", "Longitude (-180 to 180)", ""),
                    field("unit", "Unit (celsius / fahrenheit)", "celsius"),
                ]
            },
            validate_options: |options| {
                validate_unit(options)?;
                for (key, max) in [("latitude", 90.0), ("longitude", 180.0)] {
                    let value = setting(options, key, "");
                    if !value.is_empty() {
                        let value = value.parse::<f64>()?;
                        anyhow::ensure!(value.is_finite() && value.abs() <= max, "Invalid {key}");
                    }
                }
                Ok(())
            },
        },
        TileDefinition {
            kind: "usage",
            name: "Usage / quota",
            default_refresh_ms: 60000,
            sources: &[Source::Usage],
            create: || Box::new(ExternalTile),
            fields: const {
                &[
                    field("source", "Usage JSON path or URL", ""),
                    field("token_env", "Bearer token environment variable", ""),
                ]
            },
            validate_options: |options| {
                let source = setting(options, "source", "");
                if source.contains("://") {
                    crate::integrations::http_url(source)?;
                }
                let env = setting(options, "token_env", "");
                anyhow::ensure!(
                    env.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                    "Invalid environment variable name"
                );
                Ok(())
            },
        },
    ]
}
pub fn validate_unit(options: &toml::Table) -> anyhow::Result<()> {
    anyhow::ensure!(
        matches!(
            setting(options, "unit", "celsius"),
            "celsius" | "fahrenheit"
        ),
        "Unit must be celsius or fahrenheit"
    );
    Ok(())
}
fn weather_description(code: u16) -> &'static str {
    match code {
        0 => "Clear",
        1..=3 => "Partly cloudy / overcast",
        45 | 48 => "Fog",
        51..=57 => "Drizzle",
        61..=67 => "Rain",
        71..=77 => "Snow",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95..=99 => "Thunderstorm",
        _ => "Conditions unavailable",
    }
}
impl Tile for ExternalTile {
    fn render(&self, frame: &mut Frame, area: Rect, _: &TileConfig, metrics: &Metrics) {
        let primary = Style::default()
            .fg(theme::CYAN)
            .add_modifier(Modifier::BOLD);
        let muted = Style::default().fg(theme::MUTED);
        let lines = match &metrics.external {
            None => vec![Line::from("Reading data…")],
            Some(Err(error)) => {
                vec![Line::from(super::plain_text(error)).style(Style::default().fg(theme::YELLOW))]
            }
            Some(Ok(ExternalData::Git {
                branch,
                changed,
                staged,
                untracked,
            })) => vec![
                Line::from(super::plain_text(branch)).style(primary),
                Line::from(if *changed == 0 && *untracked == 0 {
                    "Working tree clean".into()
                } else {
                    format!("{changed} changed · {untracked} untracked")
                }),
                Line::from(format!("{staged} staged")).style(muted),
                Line::from("Local checkout · no fetch").style(muted),
            ],
            Some(Ok(ExternalData::Service {
                status,
                milliseconds,
            })) => vec![
                Line::from(format!(
                    "{} · HTTP {status}",
                    if (200..300).contains(status) {
                        "Healthy"
                    } else {
                        "Check failed"
                    }
                ))
                .style(
                    Style::default()
                        .fg(if (200..300).contains(status) {
                            theme::GREEN
                        } else {
                            theme::RED
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                Line::from(format!("{milliseconds} ms response")),
                Line::from("HEAD · redirects not followed").style(muted),
            ],
            Some(Ok(ExternalData::Weather {
                temperature,
                feels_like,
                humidity,
                code,
                unit,
            })) => vec![
                Line::from(format!(
                    "{temperature:.1}{unit} · {}",
                    weather_description(*code)
                ))
                .style(primary),
                Line::from(format!("Feels like {feels_like:.1}{unit}")),
                Line::from(format!("Humidity {humidity:.0}%")),
                Line::from("Weather: Open-Meteo.com").style(muted),
            ],
            Some(Ok(ExternalData::Usage(report))) => vec![
                Line::from(format!("{:.1}% used", report.used / report.limit * 100.0))
                    .style(primary),
                Line::from(format!(
                    "{} / {} {}",
                    report.used,
                    report.limit,
                    super::plain_text(&report.unit)
                )),
                Line::from(format!(
                    "{} remaining",
                    (report.limit - report.used).max(0.0)
                ))
                .style(muted),
                Line::from(if report.reset_at.is_empty() {
                    "Reset not provided".into()
                } else {
                    format!("Reset: {}", super::plain_text(&report.reset_at))
                })
                .style(muted),
            ],
        };
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
    }
}
