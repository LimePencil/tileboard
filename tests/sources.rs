use chrono::{Local, TimeZone};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tileboard::{
    config::{Config, TileConfig},
    integrations::{self, ExternalData},
    metrics::{Collector, Metrics, ProcessUsage, SampleRequest, SampleResult, Source, Temperature},
    tiles::Registry,
};

fn config(kind: &str) -> TileConfig {
    let mut tile = Config::default().profiles[0].tiles[0].clone();
    tile.kind = kind.into();
    tile.options.clear();
    tile.refresh_ms = None;
    tile
}
fn render(config: &TileConfig, metrics: &Metrics, width: u16, height: u16) -> String {
    let registry = Registry::builtin();
    let mut tile = (registry.get(&config.kind).unwrap().create)();
    tile.update(config, metrics);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| tile.render(f, Rect::new(0, 0, width, height), config, metrics))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}
#[test]
fn all_fourteen_tiles_validate_defaults_and_render_small_and_large() {
    let registry = Registry::builtin();
    assert_eq!(registry.list().len(), 14);
    for definition in registry.list() {
        let tile = config(definition.kind);
        registry.validate(&tile).unwrap();
        for (w, h) in [(1, 1), (12, 4), (25, 10), (60, 20)] {
            render(&tile, &Metrics::default(), w, h);
            render(
                &tile,
                &Metrics {
                    ready: true,
                    ..Metrics::default()
                },
                w,
                h,
            );
        }
    }
}
#[test]
fn processes_sort_filter_and_first_sample_are_explicit() {
    let mut tile = config("processes");
    let metrics = Metrics {
        ready: true,
        processes: vec![
            ProcessUsage {
                pid: 1,
                name: "compiler".into(),
                cpu: Some(120.0),
                memory: 1 << 20,
            },
            ProcessUsage {
                pid: 2,
                name: "browser".into(),
                cpu: None,
                memory: 1 << 30,
            },
        ],
        ..Metrics::default()
    };
    let cpu = render(&tile, &metrics, 50, 8);
    assert!(cpu.find("compiler").unwrap() < cpu.find("browser").unwrap());
    assert!(cpu.contains("120%") && cpu.contains('…'));
    tile.options.insert("sort".into(), "memory".into());
    let memory = render(&tile, &metrics, 50, 8);
    assert!(memory.find("browser").unwrap() < memory.find("compiler").unwrap());
    tile.options.insert("filter".into(), "COMP".into());
    assert!(!render(&tile, &metrics, 50, 8).contains("browser"));
}
#[test]
fn temperatures_units_and_missing_hardware_are_honest() {
    let mut tile = config("temperature");
    tile.options.insert("unit".into(), "fahrenheit".into());
    let mut metrics = Metrics {
        ready: true,
        temperatures: vec![Temperature {
            label: "CPU".into(),
            celsius: 50.0,
            critical: Some(90.0),
        }],
        ..Metrics::default()
    };
    assert!(render(&tile, &metrics, 50, 8).contains("122.0°F"));
    metrics.temperatures.clear();
    assert!(render(&tile, &metrics, 50, 8).contains("No temperature sensors"));
    metrics.batteries = Some(Ok(vec![]));
    assert!(render(&config("battery"), &metrics, 50, 8).contains("No battery detected"));
}
#[test]
fn calendar_handles_leap_years_and_sunday_first() {
    let mut tile = config("calendar");
    let metrics = Metrics {
        now: Local.with_ymd_and_hms(2024, 2, 29, 12, 0, 0).unwrap(),
        ..Metrics::default()
    };
    let text = render(&tile, &metrics, 30, 10);
    assert!(text.contains("February 2024") && text.contains("29"));
    assert!(!text.contains("30") && !text.contains("31"));
    tile.options.insert("week_start".into(), "sunday".into());
    assert!(render(&tile, &metrics, 30, 10).contains("Su Mo Tu We Th Fr Sa"));
}
#[test]
fn weather_and_usage_parsing_validate_external_reports() {
    let body = br#"{"current":{"temperature_2m":12.5,"apparent_temperature":10.0,"relative_humidity_2m":68,"weather_code":61}}"#;
    assert!(matches!(
        integrations::parse_weather(&body[..], "celsius").unwrap(),
        ExternalData::Weather { code: 61, .. }
    ));
    assert!(integrations::parse_weather(&b"{}"[..], "celsius").is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage.json");
    let mut options = toml::Table::new();
    options.insert("source".into(), path.to_string_lossy().into_owned().into());
    let client = integrations::client().unwrap();
    std::fs::write(&path, r#"{"used":25,"limit":100,"unit":"requests"}"#).unwrap();
    let data = integrations::collect(Source::Usage, &options, &client).unwrap();
    assert!(
        render(
            &config("usage"),
            &Metrics {
                external: Some(Ok(data)),
                ..Metrics::default()
            },
            50,
            8
        )
        .contains("25.0% used")
    );
    std::fs::write(&path, r#"{"used":25,"limit":0}"#).unwrap();
    assert!(integrations::collect(Source::Usage, &options, &client).is_err());
    let mut weather = config("weather");
    weather.options.insert("latitude".into(), "NaN".into());
    assert!(Registry::builtin().validate(&weather).is_err());
    assert!(integrations::http_url("file:///etc/passwd").is_err());
}
#[test]
fn git_reads_local_changes_without_changing_the_repository() {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .output()
            .unwrap()
    };
    assert!(git(&["init", "-q"]).status.success());
    std::fs::write(dir.path().join("tracked.txt"), "hello").unwrap();
    assert!(git(&["add", "tracked.txt"]).status.success());
    std::fs::write(dir.path().join("new.txt"), "new").unwrap();
    std::fs::create_dir(dir.path().join("new-dir")).unwrap();
    std::fs::write(dir.path().join("new-dir/one.txt"), "one").unwrap();
    std::fs::write(dir.path().join("new-dir/two.txt"), "two").unwrap();
    let before = std::fs::read(dir.path().join(".git/index")).unwrap();
    let data = integrations::git_status(dir.path().to_str().unwrap()).unwrap();
    assert!(matches!(
        data,
        ExternalData::Git {
            staged: 1,
            untracked: 3,
            changed: 1,
            ..
        }
    ));
    assert_eq!(
        std::fs::read(dir.path().join(".git/index")).unwrap(),
        before
    );
}
#[test]
fn slow_service_does_not_block_system_collection_and_options_reach_worker() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (arrived, arrival) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = [0; 2048];
        let n = socket.read(&mut bytes).unwrap();
        assert!(
            std::str::from_utf8(&bytes[..n])
                .unwrap()
                .starts_with("HEAD /")
        );
        arrived.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(4)).unwrap();
        socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let collector = Collector::start();
    collector
        .request(SampleRequest {
            key: ("test".into(), "service".into(), "service".into()),
            generation: 1,
            sources: &[Source::Service],
            options: toml::Table::from_iter([("url".into(), url.into())]),
        })
        .unwrap();
    arrival.recv_timeout(Duration::from_secs(3)).unwrap();
    collector
        .request(SampleRequest {
            key: ("test".into(), "memory".into(), "memory".into()),
            generation: 1,
            sources: &[Source::Memory],
            options: toml::Table::new(),
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(result) = collector.drain().next() {
            assert_eq!(result.request.key.1, "memory");
            assert!(result.metrics.external.is_none());
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    release.send(()).unwrap();
    server.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(result) = collector.drain().next() {
            assert!(matches!(
                result.metrics.external,
                Some(Ok(ExternalData::Service { status: 503, .. }))
            ));
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
}

fn service_listener() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

fn request_service(collector: &Collector, listener: &TcpListener, id: &str, generation: u64) {
    collector
        .request(SampleRequest {
            key: ("test".into(), id.into(), "service".into()),
            generation,
            sources: &[Source::Service],
            options: toml::Table::from_iter([(
                "url".into(),
                format!("http://{}/{id}", listener.local_addr().unwrap()).into(),
            )]),
        })
        .unwrap();
}

fn accept_service(listener: &TcpListener, timeout: Duration) -> Option<(String, TcpStream)> {
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((mut socket, _)) => {
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = String::new();
                let mut reader = BufReader::new(&mut socket);
                reader.read_line(&mut request).unwrap();
                assert!(request.starts_with("HEAD /"));
                loop {
                    let mut header = String::new();
                    assert!(reader.read_line(&mut header).unwrap() > 0);
                    if header == "\r\n" {
                        break;
                    }
                }
                return Some((request, socket));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return None;
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("Cannot accept service check: {error}"),
        }
    }
}

fn respond_service(mut socket: TcpStream, status: u16) {
    write!(
        socket,
        "HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
}

fn next_sample(collector: &Collector) -> SampleResult {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(result) = collector.drain().next() {
            return result;
        }
        assert!(Instant::now() < deadline, "Collector failed to answer");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn fast_service_completes_while_another_service_is_stalled() {
    let listener = service_listener();
    let collector = Collector::start();
    request_service(&collector, &listener, "slow", 1);
    let (request, slow) = accept_service(&listener, Duration::from_secs(2)).unwrap();
    assert!(request.starts_with("HEAD /slow "));

    request_service(&collector, &listener, "fast", 42);
    let (request, fast) = accept_service(&listener, Duration::from_secs(2))
        .expect("A slow service must leave capacity for another service tile");
    assert!(request.starts_with("HEAD /fast "));
    respond_service(fast, 204);
    let result = next_sample(&collector);
    assert_eq!(result.request.key.1, "fast");
    assert_eq!(result.request.generation, 42);
    assert!(matches!(
        result.metrics.external,
        Some(Ok(ExternalData::Service { status: 204, .. }))
    ));
    assert!(collector.drain().next().is_none());

    respond_service(slow, 503);
    let result = next_sample(&collector);
    assert_eq!(result.request.key.1, "slow");
    assert_eq!(result.request.generation, 1);
    assert!(matches!(
        result.metrics.external,
        Some(Ok(ExternalData::Service { status: 503, .. }))
    ));
}

#[test]
fn service_concurrency_is_bounded_and_queued_work_resumes() {
    let listener = service_listener();
    let collector = Collector::start();
    request_service(&collector, &listener, "first", 1);
    let (_, first) = accept_service(&listener, Duration::from_secs(2)).unwrap();
    request_service(&collector, &listener, "second", 2);
    let (_, second) = accept_service(&listener, Duration::from_secs(2)).unwrap();
    request_service(&collector, &listener, "queued", 3);
    assert!(accept_service(&listener, Duration::from_millis(150)).is_none());

    // Filling the service pool must leave other external sources available.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("usage.json");
    std::fs::write(&path, r#"{"used":25,"limit":100}"#).unwrap();
    collector
        .request(SampleRequest {
            key: ("test".into(), "usage".into(), "usage".into()),
            generation: 4,
            sources: &[Source::Usage],
            options: toml::Table::from_iter([(
                "source".into(),
                path.to_string_lossy().into_owned().into(),
            )]),
        })
        .unwrap();
    let result = next_sample(&collector);
    assert_eq!(result.request.key.1, "usage");
    assert!(matches!(
        result.metrics.external,
        Some(Ok(ExternalData::Usage(_)))
    ));

    respond_service(first, 200);
    let (request, queued) = accept_service(&listener, Duration::from_secs(2))
        .expect("Queued work must resume when a worker becomes available");
    assert!(request.starts_with("HEAD /queued "));
    assert_eq!(next_sample(&collector).request.key.1, "first");
    respond_service(queued, 200);
    assert_eq!(next_sample(&collector).request.key.1, "queued");
    respond_service(second, 200);
    assert_eq!(next_sample(&collector).request.key.1, "second");
}

#[test]
fn dropping_collector_discards_queued_service_checks() {
    let listener = service_listener();
    let collector = Collector::start();
    request_service(&collector, &listener, "first", 1);
    let (_, first) = accept_service(&listener, Duration::from_secs(2)).unwrap();
    request_service(&collector, &listener, "second", 2);
    let (_, second) = accept_service(&listener, Duration::from_secs(2)).unwrap();
    request_service(&collector, &listener, "queued", 3);
    drop(collector);

    respond_service(first, 200);
    respond_service(second, 200);
    assert!(accept_service(&listener, Duration::from_millis(200)).is_none());
}
