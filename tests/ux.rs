use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use tileboard::{
    app::{App, Modal},
    config::Config,
    metrics::{DiskUsage, MemoryUsage, Metrics, NetworkUsage, SystemInfo},
    tiles::Registry,
    ui,
};

fn app() -> App {
    let mut app = App::new(Config::default(), "unused.toml".into(), Registry::builtin());
    app.resize(Rect::new(0, 0, 120, 32));
    app.update_metrics(Metrics {
        ready: true,
        cpu: Some(24.5),
        cores: vec![24.5; 8],
        memory: Some(MemoryUsage {
            total: 16 << 30,
            available: 10 << 30,
            swap_total: 2 << 30,
            swap_used: 0,
        }),
        networks: vec![NetworkUsage {
            received: 0,
            transmitted: 0,
            name: "eth0".into(),
            rates: Some((1048576.0, 8192.0)),
        }],
        disks: vec![DiskUsage {
            mount: "/".into(),
            total: 100 << 30,
            available: 60 << 30,
        }],
        system: Some(SystemInfo {
            logical_cpus: 8,
            hostname: "workstation".into(),
            os: "Example OS".into(),
            uptime: 90061,
        }),
        ..Metrics::default()
    });
    app
}

fn screen(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn default_profiles_keep_readable_tiles_across_supported_terminal_shapes() {
    let mut app = app();
    for (width, height, profile) in [
        (120, 32, "wide"),
        (80, 24, "compact"),
        (58, 28, "small"),
        (38, 40, "tall"),
        (120, 12, "short"),
        (38, 16, "minimal"),
        (26, 10, "minimal"),
    ] {
        let output = screen(&mut app, width, height);
        assert_eq!(app.config.profiles[app.profile].name, profile);
        assert!(
            !output.contains("Enlarge tile"),
            "Unreadable tile at {width}x{height}"
        );
        assert!(output.contains("24.5%"));
    }
}

#[test]
fn new_tiles_show_actual_values_and_distinguish_missing_data() {
    let mut app = app();
    let output = screen(&mut app, 120, 32);
    for expected in [
        "38%",
        "6.0 GiB / 16.0 GiB",
        "Swap  0.0 B",
        "1.0 MiB/s",
        "8.0 KiB/s",
        "eth0",
        "Up 1d 01h 01m",
        "workstation",
    ] {
        assert!(output.contains(expected), "Missing {expected}");
    }
    let network = app.config.profiles[0]
        .tiles
        .iter_mut()
        .find(|tile| tile.kind == "network")
        .unwrap();
    network.options.insert("interface".into(), "missing".into());
    assert!(screen(&mut app, 120, 32).contains("Interface unavailable"));
    app.metrics.memory = None;
    app.update_metrics(app.metrics.clone());
    assert!(screen(&mut app, 120, 32).contains("Memory unavailable"));
    app.metrics.ready = false;
    app.update_metrics(app.metrics.clone());
    assert!(screen(&mut app, 120, 32).contains("Sampling memory"));
}

#[test]
fn small_editor_keeps_save_cancel_and_settings_caret_visible() {
    let mut app = app();
    app.resize(Rect::new(0, 0, 38, 16));
    app.handle_key(KeyCode::Char('e').into());
    let output = screen(&mut app, 38, 16);
    assert!(output.contains("s save  Esc cancel  ? help"));
    app.handle_key(KeyCode::Char('t').into());
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    app.paste("a very long title with 한글 ending");
    let output = screen(&mut app, 38, 16);
    assert!(output.contains("ending") && output.contains("한") && output.contains("글"));
    assert!(output.contains("Esc close"));
    app.handle_key(KeyCode::Home.into());
    app.paste("NEW ");
    app.handle_key(KeyCode::End.into());
    app.handle_key(KeyCode::Backspace.into());
    app.handle_key(KeyCode::Enter.into());
    assert!(
        app.config.profiles[app.profile].tiles[0]
            .title
            .starts_with("NEW a very long")
    );
    assert!(
        app.config.profiles[app.profile].tiles[0]
            .title
            .ends_with("endin")
    );
}

#[test]
fn undo_returns_to_the_profile_and_tile_that_changed() {
    let mut app = app();
    app.handle_key(KeyCode::Char('e').into());
    app.handle_key(KeyCode::Char('h').into());
    app.handle_key(KeyCode::Enter.into());
    assert!(app.is_dirty());
    app.handle_key(KeyCode::Char('p').into());
    assert_eq!(app.profile, 1);
    app.handle_key(KeyCode::Char('u').into());
    assert_eq!(app.profile, 0);
    assert_eq!(app.selected, 0);
    assert!(!app.is_dirty());
}

#[test]
fn new_tile_fits_readably_and_bad_settings_leave_original_intact() {
    let mut app = app();
    app.handle_key(KeyCode::Char('e').into());
    app.handle_key(KeyCode::Char('d').into());
    app.handle_key(KeyCode::Char('a').into());
    let index = app
        .registry
        .list()
        .iter()
        .position(|d| d.kind == "network")
        .unwrap();
    for _ in 0..index {
        app.handle_key(KeyCode::Down.into());
    }
    app.handle_key(KeyCode::Enter.into());
    let tile = &app.config.profiles[0].tiles[app.selected];
    assert_eq!(tile.kind, "network");
    let area = app.tile_rect(tile.placement);
    assert!(area.width >= 12 && area.height >= 4);
    let original = tile.clone();
    app.handle_key(KeyCode::Char('t').into());
    app.handle_key(KeyCode::Tab.into());
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    app.paste("invalid-color");
    app.handle_key(KeyCode::Enter.into());
    assert!(matches!(app.modal, Some(Modal::Settings { .. })));
    assert!(app.status.contains("Invalid settings"));
    assert_eq!(app.config.profiles[0].tiles[app.selected], original);
}

#[test]
fn legacy_configs_load_without_migrating_user_layouts() {
    let config: Config = toml::from_str(include_str!("fixtures/legacy.toml")).unwrap();
    config.validate(&Registry::builtin()).unwrap();
    assert_eq!(config.profiles.len(), 3);
    assert_eq!(config.profiles[0].tiles[0].placement.column_span, 4);
}
