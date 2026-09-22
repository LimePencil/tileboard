use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use tileboard::{
    app::{App, Candidate},
    config::Config,
    tiles::Registry,
    ui,
};

fn app() -> App {
    let mut app = App::new(Config::default(), "unused.toml".into(), Registry::builtin());
    app.resize(Rect::new(0, 0, 120, 32));
    app.handle_key(KeyCode::Char('e').into());
    app
}
#[test]
fn packed_grid_swaps_preview_both_slots_and_undo_as_one_edit() {
    let mut app = app();
    let original = app.config.clone();
    assert!(app.config.profiles[0].free_slot().is_none());
    app.handle_key(KeyCode::Right.into());
    assert!(matches!(app.candidate, Some(Candidate::Swap(2))));
    assert_eq!(app.placement_preview().unwrap().len(), 2);
    assert_eq!(app.config, original);
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert_eq!(text.matches("swap preview").count(), 2);
    app.handle_key(KeyCode::Enter.into());
    assert_eq!(
        app.config.profiles[0].tiles[0].placement,
        original.profiles[0].tiles[2].placement
    );
    assert_eq!(
        app.config.profiles[0].tiles[2].placement,
        original.profiles[0].tiles[0].placement
    );
    app.config.validate(&app.registry).unwrap();
    app.handle_key(KeyCode::Char('u').into());
    assert_eq!(app.config, original);
}
#[test]
fn unequal_spans_exchange_slots_preserving_settings_and_save_cancel() {
    let config: Config = toml::from_str(include_str!("fixtures/legacy.toml")).unwrap();
    let mut app = App::new(config.clone(), "unused.toml".into(), Registry::builtin());
    app.resize(Rect::new(0, 0, 120, 40));
    app.handle_key(KeyCode::Char('e').into());
    app.handle_key(KeyCode::Right.into());
    app.handle_key(KeyCode::Enter.into());
    assert_eq!(
        app.config.profiles[0].tiles[0].placement,
        config.profiles[0].tiles[1].placement
    );
    assert_eq!(
        app.config.profiles[0].tiles[1].placement,
        config.profiles[0].tiles[0].placement
    );
    assert_eq!(app.config.profiles[0].tiles[0].kind, "cpu");
    app.config.validate(&app.registry).unwrap();
    app.handle_key(KeyCode::Esc.into());
    assert_eq!(app.config, config);
    let dir = tempfile::tempdir().unwrap();
    app.path = dir.path().join("layout.toml");
    app.handle_key(KeyCode::Char('e').into());
    app.handle_key(KeyCode::Right.into());
    app.handle_key(KeyCode::Enter.into());
    app.handle_key(KeyCode::Char('s').into());
    assert_eq!(Config::load(&app.path, &app.registry).unwrap(), app.config);
}
#[test]
fn arrow_preview_can_traverse_a_packed_row_and_return_to_origin() {
    let mut app = app();
    app.handle_key(KeyCode::Right.into());
    app.handle_key(KeyCode::Left.into());
    assert_eq!(
        app.placement_preview().unwrap()[0].1,
        app.config.profiles[0].tiles[0].placement
    );
    app.handle_key(KeyCode::Right.into());
    app.handle_key(KeyCode::Right.into());
    assert!(matches!(app.candidate, Some(Candidate::Swap(1))));
    app.handle_key(KeyCode::Right.into());
    assert!(!app.candidate_valid());
    app.handle_key(KeyCode::Esc.into());
    assert!(!app.is_dirty());
}
#[test]
fn dragging_across_multiple_occupied_tiles_targets_the_drop_slot() {
    let mut app = app();
    let original = app.config.clone();
    let start = app.tile_rect(original.profiles[0].tiles[0].placement);
    let end = app.tile_rect(original.profiles[0].tiles[5].placement);
    for (kind, rect) in [
        (MouseEventKind::Down(MouseButton::Left), start),
        (MouseEventKind::Drag(MouseButton::Left), end),
        (MouseEventKind::Up(MouseButton::Left), end),
    ] {
        app.handle_mouse(MouseEvent {
            kind,
            column: rect.x + 2,
            row: rect.y + 1,
            modifiers: KeyModifiers::NONE,
        });
    }
    assert_eq!(
        app.config.profiles[0].tiles[0].placement,
        original.profiles[0].tiles[5].placement
    );
    assert_eq!(
        app.config.profiles[0].tiles[5].placement,
        original.profiles[0].tiles[0].placement
    );
    app.config.validate(&app.registry).unwrap();
}
#[test]
fn replace_in_a_full_grid_preserves_slot_and_is_undoable() {
    let mut app = app();
    let original = app.config.clone();
    app.handle_key(KeyCode::Char('r').into());
    let index = app
        .registry
        .list()
        .iter()
        .position(|d| d.kind == "weather")
        .unwrap();
    for _ in 0..index {
        app.handle_key(KeyCode::Down.into());
    }
    app.handle_key(KeyCode::Enter.into());
    let tile = &app.config.profiles[0].tiles[0];
    assert_eq!(tile.kind, "weather");
    assert_eq!(tile.placement, original.profiles[0].tiles[0].placement);
    assert_eq!(tile.refresh_ms, None);
    assert_eq!(app.config.profiles[0].tiles.len(), 6);
    app.config.validate(&app.registry).unwrap();
    app.handle_key(KeyCode::Char('u').into());
    assert_eq!(app.config, original);
}
