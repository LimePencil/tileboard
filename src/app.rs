use std::{collections::HashMap, path::PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    config::{Config, TileConfig},
    grid::{Grid, Placement},
    metrics::Metrics,
    tiles::{Registry, Tile},
};

pub enum Modal {
    Add {
        selected: usize,
    },
    Settings {
        fields: Vec<(String, String)>,
        selected: usize,
        cursor: usize,
    },
    Help {
        scroll: usize,
    },
}

struct Drag {
    origin: Placement,
    start: (u16, u16),
    resize: bool,
}

fn previous_boundary(value: &str, cursor: usize) -> usize {
    value
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .take_while(|&i| i < cursor)
        .last()
        .unwrap_or(0)
}

fn next_boundary(value: &str, cursor: usize) -> usize {
    value
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .find(|&i| i > cursor)
        .unwrap_or(value.len())
}

pub struct App {
    pub config: Config,
    pub path: PathBuf,
    pub registry: Registry,
    pub metrics: Metrics,
    pub tiles: HashMap<(String, String, String), Box<dyn Tile>>,
    pub size: Rect,
    pub editing: bool,
    pub profile: usize,
    pub selected: usize,
    pub candidate: Option<Placement>,
    pub modal: Option<Modal>,
    pub status: String,
    pub quit: bool,
    original: Option<Config>,
    history: Vec<(Config, usize, usize)>,
    drag: Option<Drag>,
}

impl App {
    fn new_tile_placement(&self, kind: &str) -> Option<Placement> {
        let minimum = (self.registry.get(kind)?.create)().minimum_size();
        let profile = &self.config.profiles[self.profile];
        let area = self.board_area();
        if area.is_empty() {
            return None;
        }
        let columns = (u32::from(minimum.0) * u32::from(profile.columns))
            .div_ceil(u32::from(area.width)) as u16;
        let rows = (u32::from(minimum.1) * u32::from(profile.rows)).div_ceil(u32::from(area.height))
            as u16;
        let sizes = [
            (columns.max(2), rows.max(2)),
            (columns, rows),
            (columns + 1, rows),
            (columns, rows + 1),
            (columns + 1, rows + 1),
        ];
        for (column_span, row_span) in sizes {
            for row in 0..profile.rows {
                for column in 0..profile.columns {
                    let placement = Placement {
                        column,
                        row,
                        column_span,
                        row_span,
                    };
                    if profile.can_place(None, placement) {
                        let rect = self.tile_rect(placement);
                        if rect.width >= minimum.0 && rect.height >= minimum.1 {
                            return Some(placement);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn is_dirty(&self) -> bool {
        self.original
            .as_ref()
            .is_some_and(|original| original != &self.config)
    }

    pub fn tile_rect(&self, placement: Placement) -> Rect {
        let mut rect = self.grid().rect(placement);
        if rect.width >= 20 {
            rect.width -= 1;
        }
        if rect.height >= 7 {
            rect.height -= 1;
        }
        rect
    }

    pub fn paste(&mut self, text: &str) {
        if let Some(Modal::Settings {
            fields,
            selected,
            cursor,
        }) = &mut self.modal
        {
            for c in text.chars().filter(|c| !c.is_control()) {
                if fields[*selected].1.len() + c.len_utf8() > 512 {
                    break;
                }
                fields[*selected].1.insert(*cursor, c);
                *cursor += c.len_utf8();
            }
        }
    }
    pub fn new(config: Config, path: PathBuf, registry: Registry) -> Self {
        let mut app = Self {
            config,
            path,
            registry,
            metrics: Metrics::default(),
            tiles: HashMap::new(),
            size: Rect::default(),
            editing: false,
            profile: 0,
            selected: 0,
            candidate: None,
            modal: None,
            status: "Press e to arrange your dashboard · ? for help".into(),
            quit: false,
            original: None,
            history: vec![],
            drag: None,
        };
        app.sync_tiles();
        app
    }

    pub fn sync_tiles(&mut self) {
        let mut keys = std::collections::HashSet::new();
        for profile in &self.config.profiles {
            for config in &profile.tiles {
                let key = (profile.name.clone(), config.id.clone(), config.kind.clone());
                keys.insert(key.clone());
                self.tiles
                    .entry(key)
                    .or_insert_with(|| (self.registry.get(&config.kind).unwrap().create)());
            }
        }
        self.tiles.retain(|key, _| keys.contains(key));
    }

    pub fn update_metrics(&mut self, metrics: Metrics) {
        for profile in &self.config.profiles {
            for config in &profile.tiles {
                let key = (profile.name.clone(), config.id.clone(), config.kind.clone());
                if let Some(tile) = self.tiles.get_mut(&key) {
                    tile.update(config, &metrics);
                }
            }
        }
        self.metrics = metrics;
    }

    pub fn resize(&mut self, size: Rect) {
        if self.size != size {
            self.drag = None;
            self.candidate = None;
        }
        self.size = size;
        if !self.editing {
            self.profile = self.config.profile_for(size.width, size.height);
        }
    }

    pub fn board_area(&self) -> Rect {
        Rect::new(
            self.size.x + self.size.width.min(1),
            self.size.y + self.size.height.min(3),
            self.size.width.saturating_sub(2),
            self.size.height.saturating_sub(6),
        )
    }

    pub fn grid(&self) -> Grid {
        let profile = &self.config.profiles[self.profile];
        Grid {
            area: self.board_area(),
            columns: profile.columns,
            rows: profile.rows,
        }
    }

    pub fn candidate_valid(&self) -> bool {
        self.candidate
            .is_some_and(|p| self.config.profiles[self.profile].can_place(Some(self.selected), p))
    }

    fn remember(&mut self) {
        if self.history.len() >= 100 {
            self.history.remove(0);
        }
        self.history
            .push((self.config.clone(), self.profile, self.selected));
    }

    fn begin_edit(&mut self) {
        self.original = Some(self.config.clone());
        self.editing = true;
        self.selected = 0;
        self.history.clear();
        self.status = "Arrange tiles · ? for all controls".into();
    }

    fn end_edit(&mut self) {
        self.editing = false;
        self.original = None;
        self.candidate = None;
        self.drag = None;
        self.history.clear();
        self.profile = self.config.profile_for(self.size.width, self.size.height);
        self.sync_tiles();
    }

    fn commit_candidate(&mut self) {
        if self.candidate_valid() {
            let candidate = self.candidate.take().unwrap();
            if self.config.profiles[self.profile].tiles[self.selected].placement != candidate {
                self.remember();
                self.config.profiles[self.profile].tiles[self.selected].placement = candidate;
            }
            self.status = "Placement applied · u to undo · s to save".into();
        } else if self.candidate.is_some() {
            self.status = "Blocked: tiles cannot overlap or leave the grid · adjust or Esc".into();
        }
    }

    fn preview(&mut self, dx: i32, dy: i32, resize: bool) {
        let Some(tile) = self.config.profiles[self.profile].tiles.get(self.selected) else {
            return;
        };
        let mut p = self.candidate.unwrap_or(tile.placement);
        let add = |value: u16, delta: i32, minimum: i32| {
            (i32::from(value) + delta).clamp(minimum, i32::from(u16::MAX)) as u16
        };
        if resize {
            p.column_span = add(p.column_span, dx, 1);
            p.row_span = add(p.row_span, dy, 1);
        } else {
            p.column = add(p.column, dx, 0);
            p.row = add(p.row, dy, 0);
        }
        self.candidate = Some(p);
        self.preview_status();
    }

    fn preview_status(&mut self) {
        self.status = if self.candidate_valid() {
            "Preview · Enter applies · Esc discards"
        } else {
            "Blocked: occupied cells or outside grid · adjust or Esc"
        }
        .into();
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        self.drag = None;
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        if self.modal.is_some() {
            self.modal_key(key);
            return;
        }
        if key.code == KeyCode::Char('?') {
            self.modal = Some(Modal::Help { scroll: 0 });
            return;
        }
        if !self.editing {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
                KeyCode::Char('e') => self.begin_edit(),
                KeyCode::Char('r') => match Config::load(&self.path, &self.registry) {
                    Ok(config) => {
                        self.config = config;
                        self.profile = self.config.profile_for(self.size.width, self.size.height);
                        self.sync_tiles();
                        self.status = "Configuration reloaded".into();
                    }
                    Err(error) => self.status = format!("Reload failed: {error:#}"),
                },
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.drag = None;
                if self.candidate.take().is_some() {
                    self.status = "Preview discarded".into();
                } else {
                    self.config = self.original.take().unwrap();
                    self.end_edit();
                    self.status = "Edit session cancelled".into();
                }
            }
            KeyCode::Char('s') => {
                if self.candidate.is_some() {
                    self.status =
                        "Apply the preview with Enter or discard it with Esc before saving".into();
                } else {
                    match self.config.save(&self.path, &self.registry) {
                        Ok(()) => {
                            self.end_edit();
                            self.status = format!("Saved {}", self.path.display());
                        }
                        Err(error) => self.status = format!("Save failed: {error:#}"),
                    }
                }
            }
            KeyCode::Char('q') => {
                self.status = "Use s to save or Esc to cancel this edit session".into()
            }
            KeyCode::Char('p') => {
                self.profile = (self.profile + 1) % self.config.profiles.len();
                self.selected = 0;
                self.candidate = None;
                self.drag = None;
                self.status = "Profile preview pinned while editing · p cycles · changes affect this profile only".into();
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let count = self.config.profiles[self.profile].tiles.len();
                if count > 0 {
                    self.selected = if key.code == KeyCode::BackTab {
                        (self.selected + count - 1) % count
                    } else {
                        (self.selected + 1) % count
                    };
                }
                self.candidate = None;
                self.drag = None;
                self.status = "Selected tile · arrows move · t settings · ? help".into();
            }
            KeyCode::Enter => self.commit_candidate(),
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                let (dx, dy) = match key.code {
                    KeyCode::Left => (-1, 0),
                    KeyCode::Right => (1, 0),
                    KeyCode::Up => (0, -1),
                    _ => (0, 1),
                };
                self.preview(dx, dy, key.modifiers.contains(KeyModifiers::SHIFT));
            }
            // Explicit span controls also work in terminals that swallow Shift+arrow.
            KeyCode::Char('h') => self.preview(-1, 0, true),
            KeyCode::Char('l') => self.preview(1, 0, true),
            KeyCode::Char('k') => self.preview(0, -1, true),
            KeyCode::Char('j') => self.preview(0, 1, true),
            KeyCode::Char('u') => {
                self.candidate = None;
                self.drag = None;
                if let Some((config, profile, selected)) = self.history.pop() {
                    self.config = config;
                    self.profile = profile;
                    self.selected = selected.min(
                        self.config.profiles[self.profile]
                            .tiles
                            .len()
                            .saturating_sub(1),
                    );
                    self.sync_tiles();
                    self.status = "Last edit undone".into();
                }
            }
            KeyCode::Delete | KeyCode::Char('d') => {
                if self.selected < self.config.profiles[self.profile].tiles.len() {
                    self.remember();
                    self.config.profiles[self.profile]
                        .tiles
                        .remove(self.selected);
                    self.selected = self.selected.min(
                        self.config.profiles[self.profile]
                            .tiles
                            .len()
                            .saturating_sub(1),
                    );
                    self.candidate = None;
                    self.sync_tiles();
                    self.status = "Tile removed from this profile · u to undo".into();
                }
            }
            KeyCode::Char('a') => {
                self.candidate = None;
                self.modal = Some(Modal::Add { selected: 0 });
            }
            KeyCode::Char('t') => self.open_settings(),
            _ => {}
        }
    }

    fn open_settings(&mut self) {
        let Some(tile) = self.config.profiles[self.profile].tiles.get(self.selected) else {
            return;
        };
        let mut fields = vec![
            ("Title".into(), tile.title.clone()),
            ("Accent · ←/→ choose".into(), tile.accent.clone()),
        ];
        for field in self.registry.get(&tile.kind).unwrap().fields {
            fields.push((
                field.label.into(),
                tile.options
                    .get(field.key)
                    .and_then(toml::Value::as_str)
                    .unwrap_or(field.default)
                    .into(),
            ));
        }
        self.candidate = None;
        let cursor = fields[0].1.len();
        self.modal = Some(Modal::Settings {
            fields,
            selected: 0,
            cursor,
        });
        self.status = "Edit tile settings".into();
    }

    fn modal_key(&mut self, key: KeyEvent) {
        let mut modal = self.modal.take().unwrap();
        if key.code == KeyCode::Esc {
            self.status = "Dialog closed · ? for help".into();
            return;
        }
        match &mut modal {
            Modal::Help { scroll } => match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    *scroll = (*scroll + 1).min(crate::ui::HELP_LINES.len() - 1)
                }
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::PageDown => *scroll = (*scroll + 5).min(crate::ui::HELP_LINES.len() - 1),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(5),
                _ => return,
            },
            Modal::Add { selected } => {
                let count = self.registry.list().len();
                match key.code {
                    KeyCode::Down | KeyCode::Tab => *selected = (*selected + 1) % count,
                    KeyCode::Up | KeyCode::BackTab => *selected = (*selected + count - 1) % count,
                    KeyCode::Enter => {
                        let kind = self.registry.list()[*selected].kind;
                        if let Some(placement) = self.new_tile_placement(kind) {
                            let def = self.registry.list()[*selected];
                            let kind = def.kind.to_string();
                            let title = def.name.to_string();
                            let mut number = 1;
                            let id = loop {
                                let id = format!("{kind}-{number}");
                                if !self.config.profiles[self.profile]
                                    .tiles
                                    .iter()
                                    .any(|t| t.id == id)
                                {
                                    break id;
                                }
                                number += 1;
                            };
                            self.remember();
                            let tiles = &mut self.config.profiles[self.profile].tiles;
                            tiles.push(TileConfig {
                                id,
                                kind,
                                title,
                                accent: "cyan".into(),
                                placement,
                                options: toml::Table::new(),
                            });
                            self.selected = tiles.len() - 1;
                            self.sync_tiles();
                            self.status = "Tile added · resize with Shift+arrows or h/j/k/l".into();
                            return;
                        }
                        self.status = "No readable space · free cells or enlarge terminal".into();
                    }
                    _ => {}
                }
            }
            Modal::Settings {
                fields,
                selected,
                cursor,
            } => match key.code {
                KeyCode::Tab | KeyCode::Down => {
                    *selected = (*selected + 1) % fields.len();
                    *cursor = fields[*selected].1.len();
                }
                KeyCode::BackTab | KeyCode::Up => {
                    *selected = (*selected + fields.len() - 1) % fields.len();
                    *cursor = fields[*selected].1.len();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    fields[*selected].1.clear();
                    *cursor = 0;
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = fields[*selected].1.len(),
                KeyCode::Left | KeyCode::Right if *selected == 1 => {
                    let colors = ["cyan", "magenta", "green", "yellow", "blue", "red", "white"];
                    let current = colors
                        .iter()
                        .position(|&color| color == fields[1].1)
                        .unwrap_or(0);
                    let next = if key.code == KeyCode::Right {
                        (current + 1) % colors.len()
                    } else {
                        (current + colors.len() - 1) % colors.len()
                    };
                    fields[1].1 = colors[next].into();
                    *cursor = fields[1].1.len();
                }
                KeyCode::Left => *cursor = previous_boundary(&fields[*selected].1, *cursor),
                KeyCode::Right => *cursor = next_boundary(&fields[*selected].1, *cursor),
                KeyCode::Backspace => {
                    let previous = previous_boundary(&fields[*selected].1, *cursor);
                    fields[*selected].1.replace_range(previous..*cursor, "");
                    *cursor = previous;
                }
                KeyCode::Delete => {
                    let next = next_boundary(&fields[*selected].1, *cursor);
                    fields[*selected].1.replace_range(*cursor..next, "");
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    if !c.is_control() && fields[*selected].1.len() + c.len_utf8() <= 512 {
                        fields[*selected].1.insert(*cursor, c);
                        *cursor += c.len_utf8();
                    }
                }
                KeyCode::Enter => {
                    let mut tile = self.config.profiles[self.profile].tiles[self.selected].clone();
                    tile.title = fields[0].1.clone();
                    tile.accent = fields[1].1.clone();
                    for (field, (_, value)) in self
                        .registry
                        .get(&tile.kind)
                        .unwrap()
                        .fields
                        .iter()
                        .zip(&fields[2..])
                    {
                        tile.options
                            .insert(field.key.into(), toml::Value::String(value.clone()));
                    }
                    match self.registry.validate(&tile) {
                        Ok(()) => {
                            self.remember();
                            self.config.profiles[self.profile].tiles[self.selected] = tile;
                            self.status = "Tile settings applied · s to save".into();
                            return;
                        }
                        Err(error) => self.status = format!("Invalid settings: {error:#}"),
                    }
                }
                _ => {}
            },
        }
        self.modal = Some(modal);
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) {
        if !self.editing || self.modal.is_some() || self.size.width < 26 || self.size.height < 10 {
            return;
        }
        let grid = self.grid();
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.drag = None;
                let Some(cell) = grid.cell(event.column, event.row) else {
                    return;
                };
                let selected = self.config.profiles[self.profile]
                    .tiles
                    .iter()
                    .position(|tile| {
                        self.tile_rect(tile.placement)
                            .contains((event.column, event.row).into())
                    });
                if let Some(index) = selected {
                    self.selected = index;
                    self.candidate = None;
                    let origin = self.config.profiles[self.profile].tiles[index].placement;
                    let rect = self.tile_rect(origin);
                    let resize = event.column >= rect.right().saturating_sub(2)
                        && event.row == rect.bottom().saturating_sub(1);
                    self.drag = Some(Drag {
                        origin,
                        start: cell,
                        resize,
                    });
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some(drag) = &self.drag else {
                    return;
                };
                let x = event
                    .column
                    .clamp(grid.area.x, grid.area.right().saturating_sub(1));
                let y = event
                    .row
                    .clamp(grid.area.y, grid.area.bottom().saturating_sub(1));
                let Some(cell) = grid.cell(x, y) else {
                    return;
                };
                let (dx, dy) = (
                    i32::from(cell.0) - i32::from(drag.start.0),
                    i32::from(cell.1) - i32::from(drag.start.1),
                );
                let resize = drag.resize;
                self.candidate = Some(drag.origin);
                self.preview(dx, dy, resize);
            }
            MouseEventKind::Up(MouseButton::Left) if self.drag.take().is_some() => {
                self.commit_candidate();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::new(
            toml::from_str(include_str!("../tests/fixtures/legacy.toml")).unwrap(),
            PathBuf::from("unused.toml"),
            Registry::builtin(),
        );
        app.resize(Rect::new(0, 0, 120, 40));
        app.handle_key(KeyCode::Char('e').into());
        app
    }

    #[test]
    fn collision_does_not_change_layout_and_cancel_restores_session() {
        let mut app = app();
        let original = app.config.clone();
        app.handle_key(KeyCode::Right.into());
        assert!(!app.candidate_valid());
        app.handle_key(KeyCode::Enter.into());
        assert_eq!(app.config, original);
        app.handle_key(KeyCode::Esc.into());
        app.handle_key(KeyCode::Char('h').into());
        assert!(app.candidate_valid());
        app.handle_key(KeyCode::Enter.into());
        assert_eq!(app.config.profiles[0].tiles[0].placement.column_span, 3);
        app.handle_key(KeyCode::Char('u').into());
        assert_eq!(app.config, original);
        app.handle_key(KeyCode::Char('d').into());
        app.handle_key(KeyCode::Esc.into());
        assert!(!app.editing);
        assert_eq!(app.config, original);
    }

    #[test]
    fn editing_pins_profile_until_saved_then_resumes_responsiveness() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app();
        app.path = dir.path().join("config.toml");
        app.resize(Rect::new(0, 0, 50, 30));
        assert_eq!(app.profile, 0);
        app.handle_key(KeyCode::Char('h').into());
        app.handle_key(KeyCode::Enter.into());
        app.handle_key(KeyCode::Char('s').into());
        assert!(!app.editing);
        assert_eq!(app.profile, 2);
        let saved = Config::load(&app.path, &app.registry).unwrap();
        assert_eq!(saved.profiles[0].tiles[0].placement.column_span, 3);
        assert_eq!(saved.profiles[2].tiles[0].placement.column_span, 2);
    }

    #[test]
    fn add_settings_and_delete_are_undoable() {
        let mut app = app();
        app.handle_key(KeyCode::Char('d').into());
        app.handle_key(KeyCode::Char('a').into());
        app.handle_key(KeyCode::Enter.into()); // Clock is first in registry.
        assert_eq!(app.config.profiles[0].tiles.len(), 3);
        app.handle_key(KeyCode::Char('t').into());
        app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        for c in "My clock".chars() {
            app.handle_key(KeyCode::Char(c).into());
        }
        app.handle_key(KeyCode::Enter.into());
        assert_eq!(app.config.profiles[0].tiles[app.selected].title, "My clock");
        app.handle_key(KeyCode::Char('u').into());
        assert_eq!(
            app.config.profiles[0].tiles[app.selected].title,
            "Local time"
        );
    }

    #[test]
    fn mouse_resize_and_blocked_move_keep_layout_valid() {
        let mut app = app();
        let rect = app.tile_rect(app.config.profiles[0].tiles[0].placement);
        let mouse = |kind, column, row| MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            rect.right() - 1,
            rect.bottom() - 1,
        ));
        app.handle_mouse(mouse(
            MouseEventKind::Drag(MouseButton::Left),
            rect.right() - 22,
            rect.bottom() - 1,
        ));
        app.handle_mouse(mouse(
            MouseEventKind::Up(MouseButton::Left),
            rect.right() - 22,
            rect.bottom() - 1,
        ));
        assert_eq!(app.config.profiles[0].tiles[0].placement.column_span, 3);
        app.config.validate(&app.registry).unwrap();
        let before = app.config.clone();
        app.handle_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            rect.x + 1,
            rect.y + 1,
        ));
        app.handle_mouse(mouse(
            MouseEventKind::Drag(MouseButton::Left),
            rect.x + 60,
            rect.y + 1,
        ));
        app.handle_mouse(mouse(
            MouseEventKind::Up(MouseButton::Left),
            rect.x + 60,
            rect.y + 1,
        ));
        assert!(!app.candidate_valid());
        assert_eq!(app.config, before);
    }
}
