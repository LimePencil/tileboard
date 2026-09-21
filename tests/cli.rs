use std::{fs, process::Command};

use tileboard::{config::Config, tiles::Registry};

#[test]
fn printed_defaults_and_shipped_example_are_valid_and_equivalent() {
    let output = Command::new(env!("CARGO_BIN_EXE_tileboard"))
        .arg("--print-default-config")
        .output()
        .unwrap();
    assert!(output.status.success());
    let config: Config = toml::from_str(std::str::from_utf8(&output.stdout).unwrap()).unwrap();
    config.validate(&Registry::builtin()).unwrap();
    let example: Config = toml::from_str(include_str!("../examples/dashboard.toml")).unwrap();
    assert_eq!(example, config);
}

#[test]
fn check_rejects_invalid_input_without_modifying_or_creating_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_tileboard"))
            .arg("--config")
            .arg(&path)
            .arg("--check")
            .output()
            .unwrap()
    };
    assert!(!run().status.success());
    assert!(!path.exists());
    let mut config = Config::default();
    config.profiles[0].tiles[0].kind = "missing-tile".into();
    let content = toml::to_string(&config).unwrap();
    fs::write(&path, &content).unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unknown tile kind"));
    assert_eq!(fs::read_to_string(&path).unwrap(), content);
}

#[test]
fn clock_format_errors_are_reported_before_rendering() {
    let mut config = Config::default();
    config.profiles[0].tiles[1]
        .options
        .insert("format".into(), "%J".into());
    assert!(
        config
            .validate(&Registry::builtin())
            .unwrap_err()
            .chain()
            .any(|e| e.to_string().contains("Invalid clock format"))
    );
}
