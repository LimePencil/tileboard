//! Statically compiled tile API. Implement `Tile`, then register its factory.
mod battery;
mod calendar;
mod clock;
mod cpu;
mod external;
mod memory;
mod network;
mod processes;
mod storage;
mod system;
mod temperature;
mod visuals;

use std::collections::BTreeMap;

use anyhow::{Result, bail, ensure};
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect, style::Color};

use crate::{config::TileConfig, metrics::Metrics};

pub struct OptionField {
    pub key: &'static str,
    pub label: &'static str,
    pub default: &'static str,
}

pub trait Tile {
    /// Called on this tile instance's refresh schedule; keep this nonblocking.
    fn update(&mut self, _config: &TileConfig, _metrics: &Metrics) {}
    fn render(&self, frame: &mut Frame, area: Rect, config: &TileConfig, metrics: &Metrics);
    fn minimum_size(&self) -> (u16, u16) {
        (12, 4)
    }
    /// Reserved for future focus/interaction support; the dashboard currently never calls it.
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        false
    }
}

pub struct TileDefinition {
    pub kind: &'static str,
    pub default_refresh_ms: u64,
    pub sources: &'static [crate::metrics::Source],
    pub name: &'static str,
    pub create: fn() -> Box<dyn Tile>,
    pub fields: &'static [OptionField],
    pub validate_options: fn(&toml::Table) -> Result<()>,
}

pub struct Registry {
    definitions: BTreeMap<&'static str, TileDefinition>,
}

impl Registry {
    pub fn builtin() -> Self {
        let mut registry = Self {
            definitions: BTreeMap::new(),
        };
        registry.register(battery::definition());
        registry.register(calendar::definition());
        registry.register(processes::definition());
        registry.register(temperature::definition());
        for definition in external::definitions() {
            registry.register(definition);
        }
        registry.register(cpu::definition());
        registry.register(clock::definition());
        registry.register(storage::definition());
        registry.register(memory::definition());
        registry.register(network::definition());
        registry.register(system::definition());
        registry
    }

    pub fn register(&mut self, definition: TileDefinition) {
        use crate::metrics::Source;
        assert!(
            definition.sources.len() <= 1
                || !definition.sources.iter().any(|s| matches!(
                    s,
                    Source::Git
                        | Source::Service
                        | Source::Weather
                        | Source::Usage
                        | Source::Battery
                )),
            "Dedicated worker sources must be the only source in a tile definition"
        );
        assert!(
            !self.definitions.contains_key(definition.kind),
            "Duplicate tile kind"
        );
        self.definitions.insert(definition.kind, definition);
    }

    pub fn get(&self, kind: &str) -> Option<&TileDefinition> {
        self.definitions.get(kind)
    }
    pub fn list(&self) -> Vec<&TileDefinition> {
        self.definitions.values().collect()
    }

    pub fn validate(&self, config: &TileConfig) -> Result<()> {
        let Some(definition) = self.get(&config.kind) else {
            bail!("Unknown tile kind: {}", config.kind);
        };
        ensure!(!config.title.trim().is_empty(), "Title cannot be empty");
        ensure!(
            (250..=86_400_000)
                .contains(&config.refresh_ms.unwrap_or(definition.default_refresh_ms)),
            "Refresh must be 250..86400000 milliseconds"
        );
        ensure!(
            accent(&config.accent).is_some(),
            "Accent must be cyan, magenta, green, yellow, blue, red, or white"
        );
        for (key, value) in &config.options {
            ensure!(
                definition.fields.iter().any(|f| f.key == key),
                "Unknown option: {key}"
            );
            ensure!(value.is_str(), "Option {key} must be a string");
        }
        (definition.validate_options)(&config.options)
    }
}

pub fn accent(name: &str) -> Option<Color> {
    Some(match name {
        "cyan" => crate::theme::CYAN,
        "magenta" => crate::theme::PURPLE,
        "green" => crate::theme::GREEN,
        "yellow" => crate::theme::YELLOW,
        "blue" => Color::Rgb(147, 177, 255),
        "red" => crate::theme::RED,
        "white" => crate::theme::TEXT,
        _ => return None,
    })
}

pub fn option<'a>(config: &'a TileConfig, key: &str, fallback: &'a str) -> &'a str {
    config
        .options
        .get(key)
        .and_then(toml::Value::as_str)
        .unwrap_or(fallback)
}

/// External strings cannot add rows or terminal controls to a tile.
pub(crate) fn plain_text(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
