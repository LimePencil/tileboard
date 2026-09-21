use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::{grid::Placement, tiles::Registry};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub min_width: u16,
    #[serde(default)]
    pub min_height: u16,
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,
    /// Terminal columns / rows, not physical pixel aspect ratio.
    pub min_aspect: Option<f64>,
    pub max_aspect: Option<f64>,
    pub columns: u16,
    pub rows: u16,
    #[serde(default)]
    pub tiles: Vec<TileConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileConfig {
    pub id: String,
    pub kind: String,
    pub title: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    pub placement: Placement,
    #[serde(default)]
    pub options: toml::Table,
}

fn default_accent() -> String {
    "cyan".into()
}

impl Profile {
    pub fn matches(&self, width: u16, height: u16) -> bool {
        let aspect = f64::from(width) / f64::from(height.max(1));
        width >= self.min_width
            && height >= self.min_height
            && self.max_width.is_none_or(|v| width <= v)
            && self.max_height.is_none_or(|v| height <= v)
            && self.min_aspect.is_none_or(|v| aspect >= v)
            && self.max_aspect.is_none_or(|v| aspect <= v)
    }

    pub fn can_place(&self, except: Option<usize>, placement: Placement) -> bool {
        placement.fits(self.columns, self.rows)
            && self
                .tiles
                .iter()
                .enumerate()
                .all(|(i, t)| Some(i) == except || !placement.overlaps(t.placement))
    }

    pub fn free_slot(&self) -> Option<Placement> {
        for row in 0..self.rows {
            for column in 0..self.columns {
                let p = Placement {
                    column,
                    row,
                    column_span: 1,
                    row_span: 1,
                };
                if self.can_place(None, p) {
                    return Some(p);
                }
            }
        }
        None
    }
}

impl Config {
    pub fn profile_for(&self, width: u16, height: u16) -> usize {
        self.profiles
            .iter()
            .position(|p| p.matches(width, height))
            .unwrap_or(self.profiles.len() - 1)
    }

    pub fn validate(&self, registry: &Registry) -> Result<()> {
        ensure!(
            self.version == 1,
            "Unsupported config version {}",
            self.version
        );
        ensure!(
            !self.profiles.is_empty(),
            "At least one profile is required"
        );
        let mut names = HashSet::new();
        for profile in &self.profiles {
            ensure!(
                !profile.name.trim().is_empty() && names.insert(&profile.name),
                "Profile names must be nonempty and unique"
            );
            ensure!(
                (1..=64).contains(&profile.columns) && (1..=64).contains(&profile.rows),
                "Profile {}: grid dimensions must be 1..64",
                profile.name
            );
            ensure!(
                profile.max_width.is_none_or(|v| v >= profile.min_width)
                    && profile.max_height.is_none_or(|v| v >= profile.min_height),
                "Profile {}: invalid size bounds",
                profile.name
            );
            for aspect in [profile.min_aspect, profile.max_aspect]
                .into_iter()
                .flatten()
            {
                ensure!(
                    aspect.is_finite() && aspect > 0.0,
                    "Aspect ratio must be finite and positive"
                );
            }
            ensure!(
                !matches!((profile.min_aspect, profile.max_aspect), (Some(min), Some(max)) if min > max),
                "Invalid aspect ratio bounds"
            );
            let mut ids = HashSet::new();
            for (i, tile) in profile.tiles.iter().enumerate() {
                ensure!(
                    !tile.id.trim().is_empty() && ids.insert(&tile.id),
                    "Profile {}: tile IDs must be nonempty and unique",
                    profile.name
                );
                ensure!(
                    profile.can_place(Some(i), tile.placement),
                    "Profile {}: tile {} overlaps or is outside the grid",
                    profile.name,
                    tile.id
                );
                registry
                    .validate(tile)
                    .with_context(|| format!("Profile {}, tile {}", profile.name, tile.id))?;
            }
        }
        let fallback = self.profiles.last().unwrap();
        ensure!(
            fallback.min_width == 0
                && fallback.min_height == 0
                && fallback.max_width.is_none()
                && fallback.max_height.is_none()
                && fallback.min_aspect.is_none()
                && fallback.max_aspect.is_none(),
            "The last profile must be an unconditional fallback (no size or aspect limits)"
        );
        Ok(())
    }

    pub fn load(path: &Path, registry: &Registry) -> Result<Self> {
        let content =
            fs::read_to_string(path).with_context(|| format!("Reading {}", path.display()))?;
        let config: Self = toml::from_str(&content).context("Invalid TOML configuration")?;
        config.validate(registry)?;
        Ok(config)
    }

    /// Write in the same directory, flush, then replace; failed writes preserve the original.
    pub fn save(&self, path: &Path, registry: &Registry) -> Result<()> {
        self.validate(registry)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(toml::to_string_pretty(self)?.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(path)
            .map_err(|e| e.error)
            .with_context(|| format!("Saving {}", path.display()))?;
        Ok(())
    }
}

pub fn default_path() -> Result<PathBuf> {
    ProjectDirs::from("", "", "tileboard")
        .map(|dirs| dirs.config_dir().join("config.toml"))
        .context("Cannot find the configuration directory; pass --config PATH")
}

pub fn load_or_create(path: &Path, registry: &Registry) -> Result<Config> {
    match fs::metadata(path) {
        Ok(_) => Config::load(path, registry),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let config = Config::default();
            config.save(path, registry)?;
            Ok(config)
        }
        Err(e) => bail!("Cannot access {}: {e}", path.display()),
    }
}

impl Default for Config {
    fn default() -> Self {
        let tile = |kind: &str, column, row, column_span, row_span| TileConfig {
            id: kind.into(),
            kind: kind.into(),
            title: match kind {
                "cpu" => "CPU usage",
                "clock" => "Local time",
                _ => "Storage",
            }
            .into(),
            accent: match kind {
                "cpu" => "cyan",
                "clock" => "magenta",
                _ => "green",
            }
            .into(),
            placement: Placement {
                column,
                row,
                column_span,
                row_span,
            },
            options: toml::Table::new(),
        };
        let profile = |name: &str, min_width, columns, rows, tiles| Profile {
            name: name.into(),
            min_width,
            min_height: 0,
            max_width: None,
            max_height: None,
            min_aspect: None,
            max_aspect: None,
            columns,
            rows,
            tiles,
        };
        Self {
            version: 1,
            profiles: vec![
                profile(
                    "wide",
                    110,
                    6,
                    4,
                    vec![
                        tile("cpu", 0, 0, 4, 2),
                        tile("clock", 4, 0, 2, 2),
                        tile("storage", 0, 2, 6, 2),
                    ],
                ),
                profile(
                    "compact",
                    70,
                    4,
                    4,
                    vec![
                        tile("cpu", 0, 0, 2, 2),
                        tile("clock", 2, 0, 2, 2),
                        tile("storage", 0, 2, 4, 2),
                    ],
                ),
                profile(
                    "narrow",
                    0,
                    2,
                    6,
                    vec![
                        tile("cpu", 0, 0, 2, 2),
                        tile("clock", 0, 2, 2, 2),
                        tile("storage", 0, 4, 2, 2),
                    ],
                ),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_obey_width_height_aspect_and_order() {
        let mut config = Config::default();
        assert_eq!(config.profile_for(110, 30), 0);
        assert_eq!(config.profile_for(109, 30), 1);
        assert_eq!(config.profile_for(69, 30), 2);
        config.profiles[0].min_height = 40;
        assert_eq!(config.profile_for(120, 30), 1);
        config.profiles[0].min_aspect = Some(4.0);
        assert_eq!(config.profile_for(120, 40), 1);
        assert_eq!(config.profile_for(160, 40), 0);
    }

    #[test]
    fn invalid_configs_do_not_overwrite_saved_file() {
        let registry = Registry::builtin();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = Config::default();
        config.save(&path, &registry).unwrap();
        assert_eq!(Config::load(&path, &registry).unwrap(), config);
        config.profiles[0].tiles[1].placement.column = 0;
        assert!(config.save(&path, &registry).is_err());
        assert_eq!(Config::load(&path, &registry).unwrap(), Config::default());
    }

    #[test]
    fn malformed_existing_file_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        fs::write(&path, "broken = [").unwrap();
        assert!(load_or_create(&path, &Registry::builtin()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken = [");
    }
}
