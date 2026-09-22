use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::{Duration, Instant};
use tileboard::{
    app::App,
    config::Config,
    metrics::{Collector, Metrics, NetworkUsage, SampleRequest, SampleResult, Source, TileKey},
    theme::Theme,
    tiles::Registry,
    ui,
};

fn app(kind: &str) -> App {
    let mut config = Config::default();
    let mut first = config.profiles[0].tiles[0].clone();
    first.kind = kind.into();
    first.id = "fast".into();
    first.refresh_ms = Some(250);
    let mut second = first.clone();
    second.id = "slow".into();
    second.placement.column = 2;
    second.refresh_ms = Some(1000);
    config.profiles[0].tiles = vec![first, second];
    let mut app = App::new(config, "unused.toml".into(), Registry::builtin());
    app.resize(Rect::new(0, 0, 120, 32));
    app
}
fn key(id: &str, kind: &str) -> TileKey {
    ("wide".into(), id.into(), kind.into())
}
fn deliver(app: &mut App, request: SampleRequest, time: Instant, value: f32) {
    app.apply_sample(
        SampleResult {
            request,
            metrics: Metrics {
                ready: true,
                sampled_at: time,
                cpu: Some(value),
                ..Metrics::default()
            },
        },
        time,
    );
}

#[test]
fn each_instance_updates_on_its_own_deadline_and_only_once_while_pending() {
    let mut app = app("cpu");
    let start = Instant::now();
    let initial = app.refresh_due(start);
    assert_eq!(initial.len(), 2);
    assert!(app.refresh_due(start + Duration::from_secs(20)).is_empty());
    for request in initial {
        deliver(&mut app, request, start, 10.0);
    }
    assert!(
        app.refresh_due(start + Duration::from_millis(249))
            .is_empty()
    );
    let fast = app.refresh_due(start + Duration::from_millis(250));
    assert_eq!(fast.len(), 1);
    assert_eq!(fast[0].key, key("fast", "cpu"));
    deliver(
        &mut app,
        fast[0].clone(),
        start + Duration::from_millis(250),
        90.0,
    );
    assert_eq!(app.tiles[&key("fast", "cpu")].metrics.cpu, Some(90.0));
    assert_eq!(app.tiles[&key("slow", "cpu")].metrics.cpu, Some(10.0));
    let due = app.refresh_due(start + Duration::from_secs(1));
    assert_eq!(due.len(), 2);
    // Merely drawing the UI cannot advance a slow tile's sample.
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    for _ in 0..4 {
        terminal.draw(|frame| ui::draw(frame, &mut app)).unwrap();
    }
    assert_eq!(app.tiles[&key("slow", "cpu")].metrics.cpu, Some(10.0));
}

#[test]
fn changed_intervals_and_recreated_tiles_reject_stale_responses() {
    let mut app = app("cpu");
    let now = Instant::now();
    let old = app.refresh_due(now).remove(0);
    app.config.profiles[0].tiles[0].refresh_ms = Some(5000);
    app.sync_tiles();
    deliver(&mut app, old.clone(), now, 99.0);
    assert!(app.tiles[&old.key].metrics.cpu.is_none());
    let fresh = app.refresh_due(Instant::now());
    let request = fresh
        .into_iter()
        .find(|request| request.key == old.key)
        .unwrap();
    assert_ne!(request.generation, old.generation);
    deliver(&mut app, request, now, 25.0);
    assert_eq!(app.tiles[&old.key].interval, Duration::from_secs(5));
    let removed = app.config.profiles[0].tiles.remove(0);
    app.sync_tiles();
    app.config.profiles[0].tiles.insert(0, removed);
    app.sync_tiles();
    deliver(&mut app, old.clone(), now, 99.0);
    assert!(app.tiles[&old.key].metrics.cpu.is_none());
}

#[test]
fn hidden_profiles_pause_and_resume_without_catchup_bursts() {
    let mut app = app("cpu");
    let start = Instant::now();
    for request in app.refresh_due(start) {
        deliver(&mut app, request, start, 10.0);
    }
    app.resize(Rect::new(0, 0, 80, 24));
    let requests = app.refresh_due(start + Duration::from_secs(60));
    assert!(requests.iter().all(|r| r.key.0 == "compact"));
    assert_eq!(app.tiles[&key("fast", "cpu")].metrics.cpu, Some(10.0));
    app.resize(Rect::new(0, 0, 120, 32));
    assert_eq!(app.refresh_due(start + Duration::from_secs(120)).len(), 2);
    assert!(app.refresh_due(start + Duration::from_secs(120)).is_empty());
}

#[test]
fn clock_is_cached_until_due_and_never_requires_worker_io() {
    let mut app = app("clock");
    let start = Instant::now();
    assert!(app.refresh_due(start).is_empty());
    let slow_time = app.tiles[&key("slow", "clock")].metrics.sampled_at;
    app.refresh_due(start + Duration::from_millis(250));
    assert_eq!(
        app.tiles[&key("slow", "clock")].metrics.sampled_at,
        slow_time
    );
    assert_eq!(
        app.tiles[&key("slow", "clock")].next_due,
        start + Duration::from_secs(1)
    );
}

#[test]
fn network_tiles_average_over_their_own_sampling_windows() {
    let mut app = app("network");
    let start = Instant::now();
    for (milliseconds, counter) in [(0, 1000), (250, 3000), (500, 4000), (1000, 5000)] {
        let time = start + Duration::from_millis(milliseconds);
        for request in app.refresh_due(time) {
            app.apply_sample(
                SampleResult {
                    request,
                    metrics: Metrics {
                        ready: true,
                        sampled_at: time,
                        networks: vec![NetworkUsage {
                            name: "eth0".into(),
                            received: counter,
                            transmitted: counter / 2,
                            rates: None,
                        }],
                        ..Metrics::default()
                    },
                },
                time,
            );
        }
    }
    assert_eq!(
        app.tiles[&key("fast", "network")].metrics.networks[0].rates,
        Some((2000.0, 1000.0))
    );
    assert_eq!(
        app.tiles[&key("slow", "network")].metrics.networks[0].rates,
        Some((4000.0, 2000.0))
    );
}

#[test]
fn settings_validate_persist_and_cancel_interval_changes() {
    let mut app = app("cpu");
    let original = app.config.clone();
    app.handle_key(KeyCode::Char('e').into());
    app.handle_key(KeyCode::Char('t').into());
    app.handle_key(KeyCode::BackTab.into()); // Last field is always the refresh interval.
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    app.paste("0");
    app.handle_key(KeyCode::Enter.into());
    assert!(app.status.contains("250..86400000"));
    assert_eq!(app.config, original);
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    app.paste("5000");
    app.handle_key(KeyCode::Enter.into());
    assert_eq!(app.config.profiles[0].tiles[0].refresh_ms, Some(5000));
    assert_eq!(
        app.tiles[&key("fast", "cpu")].interval,
        Duration::from_secs(5)
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    app.config.save(&path, &app.registry).unwrap();
    assert_eq!(Config::load(&path, &app.registry).unwrap(), app.config);
    app.handle_key(KeyCode::Esc.into());
    assert_eq!(app.config, original);
    assert_eq!(
        app.tiles[&key("fast", "cpu")].interval,
        Duration::from_millis(250)
    );
}

#[test]
fn theme_changes_are_visible_undoable_and_saved_as_configuration() {
    let mut app = app("cpu");
    app.handle_key(KeyCode::Char('e').into());
    app.handle_key(KeyCode::Char('c').into());
    assert_eq!(app.config.theme, Theme::Amber);
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal.draw(|frame| ui::draw(frame, &mut app)).unwrap();
    assert_eq!(
        terminal.backend().buffer()[(0, 0)].bg,
        ratatui::style::Color::Rgb(24, 22, 20)
    );
    app.handle_key(KeyCode::Char('c').into());
    assert_eq!(app.config.theme, Theme::Mono);
    app.handle_key(KeyCode::Char('u').into());
    assert_eq!(app.config.theme, Theme::Amber);
    app.handle_key(KeyCode::Esc.into());
    assert_eq!(app.config.theme, Theme::Slate);
}

#[test]
fn success_notices_expire_but_errors_remain_visible() {
    let mut app = app("cpu");
    let start = Instant::now();
    app.status = "Saved config.toml".into();
    assert!(app.visible_status(start).starts_with("Saved"));
    assert_eq!(app.visible_status(start + Duration::from_secs(4)), "");
    app.status = "Save failed: read-only directory".into();
    app.visible_status(start);
    assert!(
        app.visible_status(start + Duration::from_secs(60))
            .starts_with("Save failed")
    );
}

#[test]
fn worker_returns_only_requested_sources() {
    let collector = Collector::start();
    collector
        .request(SampleRequest {
            options: toml::Table::new(),
            key: key("memory", "memory"),
            generation: 1,
            sources: &[Source::Memory],
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = collector.drain().next() {
            assert!(result.metrics.ready);
            assert!(result.metrics.cpu.is_none());
            assert!(result.metrics.disks.is_empty());
            assert!(result.metrics.networks.is_empty());
            assert!(result.metrics.system.is_none());
            break;
        }
        assert!(Instant::now() < deadline, "Collector failed to answer");
        std::thread::sleep(Duration::from_millis(10));
    }
}
