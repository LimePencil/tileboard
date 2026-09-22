use std::fs;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tileboard::{
    app::{App, Modal},
    config::Config,
    tiles::Registry,
};

fn saved_app() -> (tempfile::TempDir, App) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let registry = Registry::builtin();
    let config = Config::default();
    config.save(&path, &registry).unwrap();
    let mut app = App::new(config, path, registry);
    app.resize(Rect::new(0, 0, 120, 32));
    (directory, app)
}

fn key(app: &mut App, code: KeyCode) {
    app.handle_key(code.into());
}

fn replace_field(app: &mut App, value: &str) {
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    app.paste(value);
}

fn set_settings_fields(app: &mut App, values: &[(usize, &str)]) {
    let Some(Modal::ProfileSettings {
        fields,
        selected,
        cursor,
    }) = &mut app.modal
    else {
        panic!("profile settings were not opened");
    };
    for &(index, value) in values {
        fields[index].1 = value.into();
    }
    *cursor = fields[*selected].1.len();
}

fn choose_profile(app: &mut App, index: usize) {
    key(app, KeyCode::Char('p'));
    key(app, KeyCode::Home);
    for _ in 0..=index {
        key(app, KeyCode::Down);
    }
    key(app, KeyCode::Enter);
    assert!(app.modal.is_none(), "{}", app.status);
}

#[test]
fn picker_saves_a_pin_across_resize_and_restart_then_restores_auto() {
    let (_directory, mut app) = saved_app();
    key(&mut app, KeyCode::Char('p'));
    assert!(matches!(app.modal, Some(Modal::Profiles { selected: 0 })));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.config.active_profile.as_deref(), Some("wide"));
    assert_eq!(Config::load(&app.path, &app.registry).unwrap(), app.config);

    app.resize(Rect::new(0, 0, 50, 40));
    assert_eq!(app.config.profiles[app.profile].name, "wide");
    let saved = Config::load(&app.path, &app.registry).unwrap();
    let mut restarted = App::new(saved, app.path.clone(), Registry::builtin());
    restarted.resize(Rect::new(0, 0, 50, 40));
    assert_eq!(restarted.config.profiles[restarted.profile].name, "wide");

    key(&mut restarted, KeyCode::Char('p'));
    assert!(matches!(
        restarted.modal,
        Some(Modal::Profiles { selected: 1 })
    ));
    key(&mut restarted, KeyCode::Home);
    key(&mut restarted, KeyCode::Enter);
    assert_eq!(restarted.config.active_profile, None);
    assert_eq!(restarted.config.profiles[restarted.profile].name, "tall");
    assert_eq!(
        Config::load(&restarted.path, &restarted.registry).unwrap(),
        restarted.config
    );
    restarted.resize(Rect::new(0, 0, 120, 32));
    assert_eq!(restarted.config.profiles[restarted.profile].name, "wide");
}

#[test]
fn quick_switch_cycles_both_directions_and_persists_the_selection() {
    let (_directory, mut app) = saved_app();
    for (shortcut, name) in [('[', "minimal"), (']', "wide"), (']', "compact")] {
        key(&mut app, KeyCode::Char(shortcut));
        assert_eq!(app.config.profiles[app.profile].name, name);
        assert_eq!(app.config.active_profile.as_deref(), Some(name));
        assert_eq!(Config::load(&app.path, &app.registry).unwrap(), app.config);
    }
    let saved = Config::load(&app.path, &app.registry).unwrap();
    let mut restarted = App::new(saved, app.path.clone(), Registry::builtin());
    restarted.resize(Rect::new(0, 0, 20, 10));
    assert_eq!(restarted.config.profiles[restarted.profile].name, "compact");
}

#[test]
fn profile_settings_support_keyboard_editing_and_atomic_undo() {
    let (_directory, mut app) = saved_app();
    let original = app.config.clone();
    let disk = fs::read(&app.path).unwrap();
    key(&mut app, KeyCode::Char('e'));
    key(&mut app, KeyCode::Char('g'));
    for value in [
        "Work layout",
        "8",
        "6",
        "true",
        "130",
        "24",
        "200",
        "60",
        "2",
        "6",
    ] {
        replace_field(&mut app, value);
        key(&mut app, KeyCode::Tab);
    }
    key(&mut app, KeyCode::Enter);
    assert!(app.modal.is_none(), "{}", app.status);
    let profile = &app.config.profiles[0];
    assert_eq!(profile.name, "Work layout");
    assert_eq!((profile.columns, profile.rows), (8, 6));
    assert!(profile.automatic);
    assert_eq!((profile.min_width, profile.min_height), (130, 24));
    assert_eq!(
        (profile.max_width, profile.max_height),
        (Some(200), Some(60))
    );
    assert_eq!(
        (profile.min_aspect, profile.max_aspect),
        (Some(2.0), Some(6.0))
    );
    assert_eq!(profile.tiles, original.profiles[0].tiles);
    assert!(app.is_dirty());
    assert_eq!(fs::read(&app.path).unwrap(), disk);

    key(&mut app, KeyCode::Char('u'));
    assert_eq!(app.config, original);
    assert!(!app.is_dirty());
    assert_eq!(app.profile, 0);
}

#[test]
fn invalid_profile_settings_preserve_layout_and_saved_configuration() {
    let (_directory, mut app) = saved_app();
    let original = app.config.clone();
    let disk = fs::read(&app.path).unwrap();
    key(&mut app, KeyCode::Char('e'));
    let invalid: &[&[(usize, &str)]] = &[
        &[(0, "   ")],
        &[(0, "compact")],
        &[(1, "5")], // The clock and system tiles extend into column six.
        &[(2, "3")], // The lower tiles extend into row four.
        &[(1, "0")],
        &[(2, "65")],
        &[(3, "yes")],
        &[(4, "150"), (6, "100")],
        &[(5, "40"), (7, "20")],
        &[(8, "NaN")],
        &[(9, "inf")],
        &[(8, "0")],
        &[(8, "4"), (9, "2")],
    ];
    for changes in invalid {
        key(&mut app, KeyCode::Char('g'));
        set_settings_fields(&mut app, changes);
        key(&mut app, KeyCode::Enter);
        assert!(
            matches!(app.modal, Some(Modal::ProfileSettings { .. })),
            "accepted {changes:?}"
        );
        assert!(app.status.starts_with("Invalid profile:"), "{}", app.status);
        assert_eq!(
            app.config, original,
            "mutated configuration for {changes:?}"
        );
        assert_eq!(fs::read(&app.path).unwrap(), disk);
        assert!(!app.is_dirty());
        key(&mut app, KeyCode::Esc);
    }
}

#[test]
fn fallback_cannot_be_disabled_or_given_responsive_limits() {
    let (_directory, mut app) = saved_app();
    let fallback = app.config.profiles.len() - 1;
    choose_profile(&mut app, fallback);
    key(&mut app, KeyCode::Char('e'));
    let original = app.config.clone();
    for change in [
        (3, "false"),
        (4, "1"),
        (5, "1"),
        (6, "100"),
        (7, "40"),
        (8, "1"),
        (9, "5"),
    ] {
        key(&mut app, KeyCode::Char('g'));
        set_settings_fields(&mut app, &[change]);
        key(&mut app, KeyCode::Enter);
        assert!(
            app.status.contains("unconditional fallback"),
            "{}",
            app.status
        );
        assert_eq!(app.config, original);
        key(&mut app, KeyCode::Esc);
    }
    assert_eq!(Config::load(&app.path, &app.registry).unwrap(), original);
}

#[test]
fn renaming_a_pinned_profile_updates_its_reference_and_runtime_tiles() {
    let (_directory, mut app) = saved_app();
    choose_profile(&mut app, 0);
    key(&mut app, KeyCode::Char('e'));
    key(&mut app, KeyCode::Char('g'));
    replace_field(&mut app, "Work 한글");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.config.active_profile.as_deref(), Some("Work 한글"));
    assert_eq!(app.config.profiles[app.profile].name, "Work 한글");
    assert!(!app.tiles.keys().any(|(profile, _, _)| profile == "wide"));
    assert_eq!(
        app.tiles
            .keys()
            .filter(|(profile, _, _)| profile == "Work 한글")
            .count(),
        6
    );
    key(&mut app, KeyCode::Char('s'));
    assert!(!app.editing, "{}", app.status);
    let saved = Config::load(&app.path, &app.registry).unwrap();
    assert_eq!(saved, app.config);
    let mut restarted = App::new(saved, app.path.clone(), Registry::builtin());
    restarted.resize(Rect::new(0, 0, 30, 10));
    assert_eq!(
        restarted.config.profiles[restarted.profile].name,
        "Work 한글"
    );
}

#[test]
fn save_as_creates_an_independent_manual_layout_and_reloads_it_selected() {
    let (_directory, mut app) = saved_app();
    let original = app.config.clone();
    key(&mut app, KeyCode::Char('e'));
    key(&mut app, KeyCode::Char('n'));
    replace_field(&mut app, "My dashboard");
    key(&mut app, KeyCode::Enter);
    assert!(app.modal.is_none(), "{}", app.status);
    let copy = app.profile;
    assert_eq!(copy, original.profiles.len() - 1);
    assert_eq!(app.config.profiles.len(), original.profiles.len() + 1);
    assert_eq!(app.config.profiles[copy].tiles, original.profiles[0].tiles);
    assert!(!app.config.profiles[copy].automatic);
    assert_eq!(app.config.profiles.last(), original.profiles.last());
    assert_eq!(app.config.active_profile.as_deref(), Some("My dashboard"));
    assert_eq!(Config::load(&app.path, &app.registry).unwrap(), original);

    key(&mut app, KeyCode::Char('t'));
    replace_field(&mut app, "Work CPU");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.config.profiles[copy].tiles[0].title, "Work CPU");
    assert_eq!(app.config.profiles[0], original.profiles[0]);
    key(&mut app, KeyCode::Char('s'));
    assert!(!app.editing, "{}", app.status);
    let saved = Config::load(&app.path, &app.registry).unwrap();
    assert_eq!(saved, app.config);
    let mut restarted = App::new(saved, app.path.clone(), Registry::builtin());
    restarted.resize(Rect::new(0, 0, 30, 10));
    assert_eq!(
        restarted.config.profiles[restarted.profile].name,
        "My dashboard"
    );
    assert_eq!(
        restarted.config.profiles[restarted.profile].tiles[0].title,
        "Work CPU"
    );
    assert_eq!(restarted.config.profiles[0], original.profiles[0]);
}

#[test]
fn copied_fallback_is_undoable_and_cancelling_restores_the_entire_session() {
    let (_directory, mut app) = saved_app();
    let fallback = app.config.profiles.len() - 1;
    choose_profile(&mut app, fallback);
    let original = app.config.clone();
    key(&mut app, KeyCode::Char('e'));
    key(&mut app, KeyCode::Char('n'));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.profile, fallback);
    assert_eq!(app.config.profiles[app.profile].name, "minimal-copy");
    assert!(app.is_dirty());
    key(&mut app, KeyCode::Char('u'));
    assert_eq!(app.config, original);
    assert_eq!(app.profile, fallback);
    assert!(!app.tiles.keys().any(|(name, _, _)| name == "minimal-copy"));
    assert!(!app.is_dirty());

    key(&mut app, KeyCode::Char('d'));
    key(&mut app, KeyCode::Char('n'));
    key(&mut app, KeyCode::Enter);
    assert!(app.config.profiles[app.profile].tiles.is_empty());
    key(&mut app, KeyCode::Esc);
    assert!(!app.editing);
    assert_eq!(app.config, original);
    assert_eq!(app.profile, fallback);
    assert_eq!(Config::load(&app.path, &app.registry).unwrap(), original);
}

#[test]
fn duplicate_or_empty_copy_names_do_not_change_the_existing_profiles() {
    let (_directory, mut app) = saved_app();
    let original = app.config.clone();
    key(&mut app, KeyCode::Char('e'));
    for name in ["wide", "minimal", "  "] {
        key(&mut app, KeyCode::Char('n'));
        replace_field(&mut app, name);
        key(&mut app, KeyCode::Enter);
        assert!(matches!(app.modal, Some(Modal::SaveProfile { .. })));
        assert!(app.status.contains("nonempty and unique"));
        assert_eq!(app.config, original);
        assert!(!app.is_dirty());
        key(&mut app, KeyCode::Esc);
    }
    assert_eq!(Config::load(&app.path, &app.registry).unwrap(), original);
}

#[test]
fn automatic_selection_skips_manual_profiles_even_when_their_rules_match() {
    let (_directory, mut app) = saved_app();
    let mut manual = app.config.profiles.last().unwrap().clone();
    manual.name = "Manual first".into();
    manual.automatic = false;
    app.config.profiles.insert(0, manual);
    app.config.validate(&app.registry).unwrap();
    assert_eq!(
        app.config.profiles[app.config.profile_for(120, 32)].name,
        "wide"
    );
    assert_eq!(
        app.config.profiles[app.config.profile_for(10, 5)].name,
        "minimal"
    );
    app.config.active_profile = Some("Manual first".into());
    assert_eq!(app.config.selected_profile(120, 32), 0);
    app.config.save(&app.path, &app.registry).unwrap();
    let reloaded = Config::load(&app.path, &app.registry).unwrap();
    assert_eq!(reloaded, app.config);
    assert!(!reloaded.profiles[0].automatic);
}

#[test]
fn legacy_profiles_default_to_automatic_and_round_trip_without_migration() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.toml");
    fs::write(&path, include_str!("fixtures/legacy.toml")).unwrap();
    let registry = Registry::builtin();
    let config = Config::load(&path, &registry).unwrap();
    assert_eq!(config.active_profile, None);
    assert!(config.profiles.iter().all(|profile| profile.automatic));
    assert_eq!(
        config.profiles[config.selected_profile(120, 40)].name,
        "wide"
    );
    assert_eq!(
        config.profiles[config.selected_profile(80, 40)].name,
        "compact"
    );
    assert_eq!(
        config.profiles[config.selected_profile(40, 40)].name,
        "narrow"
    );
    config.save(&path, &registry).unwrap();
    assert_eq!(Config::load(&path, &registry).unwrap(), config);
}

#[test]
fn switching_refuses_to_overwrite_external_edits_or_corrupt_saved_data() {
    let (_directory, mut app) = saved_app();
    let original = app.config.clone();
    let original_profile = app.profile;
    let mut external = original.clone();
    external.profiles[0].tiles[0].title = "Edited externally".into();
    external.save(&app.path, &app.registry).unwrap();
    let disk = fs::read(&app.path).unwrap();
    key(&mut app, KeyCode::Char(']'));
    assert!(app.status.contains("changed on disk"), "{}", app.status);
    assert_eq!(app.config, original);
    assert_eq!(app.profile, original_profile);
    assert_eq!(fs::read(&app.path).unwrap(), disk);

    fs::write(&app.path, "broken = [").unwrap();
    key(&mut app, KeyCode::Char('p'));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert!(matches!(app.modal, Some(Modal::Profiles { .. })));
    assert!(app.status.starts_with("Cannot switch profile:"));
    assert_eq!(app.config, original);
    assert_eq!(app.profile, original_profile);
    assert_eq!(fs::read_to_string(&app.path).unwrap(), "broken = [");
}

#[test]
fn switching_and_saving_failures_preserve_selection_and_staged_edits() {
    let (_directory, mut app) = saved_app();
    let original = app.config.clone();
    let original_profile = app.profile;
    let saved_path = app.path.clone();
    let disk = fs::read(&saved_path).unwrap();
    // A regular file cannot become the parent directory of a new saved config.
    // This produces a deterministic write failure even when tests run as root.
    app.path = saved_path.join("blocked.toml");
    key(&mut app, KeyCode::Char(']'));
    assert!(
        app.status.starts_with("Cannot switch profile:"),
        "{}",
        app.status
    );
    assert_eq!(app.config, original);
    assert_eq!(app.profile, original_profile);
    assert_eq!(fs::read(&saved_path).unwrap(), disk);

    key(&mut app, KeyCode::Char('e'));
    key(&mut app, KeyCode::Char('n'));
    key(&mut app, KeyCode::Enter);
    let staged = app.config.clone();
    let staged_profile = app.profile;
    key(&mut app, KeyCode::Char('s'));
    assert!(app.status.starts_with("Save failed:"), "{}", app.status);
    assert!(app.editing);
    assert!(app.is_dirty());
    assert_eq!(app.config, staged);
    assert_eq!(app.profile, staged_profile);
    assert_eq!(fs::read(&saved_path).unwrap(), disk);
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.config, original);
    assert_eq!(app.profile, original_profile);
}
