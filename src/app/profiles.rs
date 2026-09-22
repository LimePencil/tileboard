use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent};

use super::{App, Modal, edit_field_key};
use crate::config::Config;

impl App {
    pub(super) fn open_profiles(&mut self) {
        self.modal = Some(Modal::Profiles {
            selected: if self.config.active_profile.is_some() {
                self.profile + 1
            } else {
                0
            },
        });
        self.status = "Choose a saved profile or Auto for responsive layouts".into();
    }

    fn select_saved_profile(&mut self, selected: Option<usize>) -> Result<()> {
        let mut config = self.config.clone();
        config.active_profile = selected.map(|i| config.profiles[i].name.clone());
        if config != self.config {
            // Switching is also a save. Preserve external edits until the user reloads them.
            match Config::load(&self.path, &self.registry) {
                Ok(saved) => ensure!(
                    saved == self.config,
                    "Configuration changed on disk; press Esc then r to reload before switching"
                ),
                Err(error) if self.path.exists() => return Err(error),
                Err(_) => {}
            }
            config.save(&self.path, &self.registry)?;
        }
        self.config = config;
        self.profile = self
            .config
            .selected_profile(self.size.width, self.size.height);
        self.selected = 0;
        self.status = match &self.config.active_profile {
            Some(name) => format!("Saved profile selection: {name} · p to switch"),
            None => "Saved profile selection: Auto · layouts follow terminal size".into(),
        };
        self.status_seen.clear();
        Ok(())
    }

    pub(super) fn cycle_saved_profile(&mut self, forward: bool) {
        let count = self.config.profiles.len();
        let selected = if forward {
            (self.profile + 1) % count
        } else {
            (self.profile + count - 1) % count
        };
        if let Err(error) = self.select_saved_profile(Some(selected)) {
            self.status = format!("Cannot switch profile: {error:#}");
        }
    }

    fn profile_edit_ready(&mut self) -> bool {
        if self.candidate.is_some() {
            self.status = "Apply the preview with Enter or discard it with Esc first".into();
            false
        } else {
            true
        }
    }

    pub(super) fn open_profile_settings(&mut self) {
        if !self.profile_edit_ready() {
            return;
        }
        let profile = &self.config.profiles[self.profile];
        let fields = vec![
            ("Profile name".into(), profile.name.clone()),
            ("Grid columns (1–64)".into(), profile.columns.to_string()),
            ("Grid rows (1–64)".into(), profile.rows.to_string()),
            (
                "Automatic selection (true / false)".into(),
                profile.automatic.to_string(),
            ),
            (
                "Minimum terminal width".into(),
                profile.min_width.to_string(),
            ),
            (
                "Minimum terminal height".into(),
                profile.min_height.to_string(),
            ),
            (
                "Maximum width (blank = unlimited)".into(),
                profile.max_width.map(|v| v.to_string()).unwrap_or_default(),
            ),
            (
                "Maximum height (blank = unlimited)".into(),
                profile
                    .max_height
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            ),
            (
                "Minimum aspect (blank = unlimited)".into(),
                profile
                    .min_aspect
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            ),
            (
                "Maximum aspect (blank = unlimited)".into(),
                profile
                    .max_aspect
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            ),
        ];
        let cursor = fields[0].1.len();
        self.modal = Some(Modal::ProfileSettings {
            fields,
            selected: 0,
            cursor,
        });
        self.status =
            "Edit grid and responsive rules · changes must keep every tile inside the grid".into();
    }

    pub(super) fn open_save_profile(&mut self) {
        if !self.profile_edit_ready() {
            return;
        }
        let base = &self.config.profiles[self.profile].name;
        let mut number = 1;
        let name = loop {
            let name = if number == 1 {
                format!("{base}-copy")
            } else {
                format!("{base}-copy-{number}")
            };
            if self.config.profiles.iter().all(|p| p.name != name) {
                break name;
            }
            number += 1;
        };
        let cursor = name.len();
        self.modal = Some(Modal::SaveProfile {
            fields: vec![("New profile name".into(), name)],
            selected: 0,
            cursor,
        });
        self.status =
            "Copy this layout under a new name · Enter applies · s saves all edits".into();
    }

    fn apply_profile_settings(&mut self, fields: &[(String, String)]) -> Result<()> {
        let mut config = self.config.clone();
        let profile = &mut config.profiles[self.profile];
        let old_name = profile.name.clone();
        profile.name = fields[0].1.trim().to_owned();
        profile.columns = parse_field(fields, 1)?;
        profile.rows = parse_field(fields, 2)?;
        profile.automatic = parse_field(fields, 3)?;
        profile.min_width = optional_field(fields, 4)?.unwrap_or(0);
        profile.min_height = optional_field(fields, 5)?.unwrap_or(0);
        profile.max_width = optional_field(fields, 6)?;
        profile.max_height = optional_field(fields, 7)?;
        profile.min_aspect = optional_field(fields, 8)?;
        profile.max_aspect = optional_field(fields, 9)?;
        if config.active_profile.as_ref() == Some(&old_name) {
            config.active_profile = Some(profile.name.clone());
        }
        config.validate(&self.registry)?;
        self.remember();
        self.config = config;
        self.sync_tiles();
        self.status = "Profile settings applied · u undo · s save".into();
        Ok(())
    }

    fn copy_profile(&mut self, name: &str) -> Result<()> {
        let mut config = self.config.clone();
        let mut profile = config.profiles[self.profile].clone();
        profile.name = name.trim().to_owned();
        profile.automatic = false;
        config.active_profile = Some(profile.name.clone());
        // Keep the unconditional fallback last and leave automatic matching unchanged.
        let index = config.profiles.len() - 1;
        config.profiles.insert(index, profile);
        config.validate(&self.registry)?;
        self.remember();
        self.config = config;
        self.profile = index;
        self.selected = 0;
        self.sync_tiles();
        self.status = "Profile copied · u undo · s save and use this profile".into();
        Ok(())
    }

    pub(super) fn profile_modal_key(&mut self, key: KeyEvent) {
        let mut modal = self.modal.take().unwrap();
        if key.code == KeyCode::Esc {
            self.status = "Dialog closed · ? for help".into();
            return;
        }
        let copying = matches!(modal, Modal::SaveProfile { .. });
        match &mut modal {
            Modal::Profiles { selected } => {
                let count = self.config.profiles.len() + 1;
                match key.code {
                    KeyCode::Down | KeyCode::Tab | KeyCode::Char('j') => {
                        *selected = (*selected + 1) % count
                    }
                    KeyCode::Up | KeyCode::BackTab | KeyCode::Char('k') => {
                        *selected = (*selected + count - 1) % count
                    }
                    KeyCode::Home => *selected = 0,
                    KeyCode::End => *selected = count - 1,
                    KeyCode::Enter => match self.select_saved_profile(selected.checked_sub(1)) {
                        Ok(()) => return,
                        Err(error) => self.status = format!("Cannot switch profile: {error:#}"),
                    },
                    _ => {}
                }
            }
            Modal::ProfileSettings {
                fields,
                selected,
                cursor,
            }
            | Modal::SaveProfile {
                fields,
                selected,
                cursor,
            } => {
                if key.code == KeyCode::Enter {
                    let result = if copying {
                        self.copy_profile(&fields[0].1)
                    } else {
                        self.apply_profile_settings(fields)
                    };
                    match result {
                        Ok(()) => return,
                        Err(error) => self.status = format!("Invalid profile: {error:#}"),
                    }
                } else {
                    edit_field_key(fields, selected, cursor, key, false);
                }
            }
            _ => unreachable!(),
        }
        self.modal = Some(modal);
    }
}

fn parse_field<T: std::str::FromStr>(fields: &[(String, String)], index: usize) -> Result<T> {
    fields[index]
        .1
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid {}", fields[index].0))
}

fn optional_field<T: std::str::FromStr>(
    fields: &[(String, String)],
    index: usize,
) -> Result<Option<T>> {
    if fields[index].1.trim().is_empty() {
        Ok(None)
    } else {
        parse_field(fields, index)
            .map(Some)
            .with_context(|| fields[index].0.clone())
    }
}
